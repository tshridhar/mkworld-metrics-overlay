use crate::ocr::OcrDetector;
use crate::state::SharedState;
use image::RgbaImage;
use std::sync::Arc;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

pub async fn start(
    state: SharedState,
    mut frame_rx: watch::Receiver<Option<Arc<RgbaImage>>>,
    token: CancellationToken,
    detector: Arc<OcrDetector>,
) {
    println!("[RaceTracker Agent] Online and waiting for valid Projector frames...");

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                println!("[RaceTracker Agent] Shutdown signal received. Terminating safely.");
                break;
            }
            res = frame_rx.changed() => {
                if res.is_err() {
                    // Channel closed
                    break;
                }

                // New frame triggered! We clone the Arc reference quickly to unblock sender
                let frame_opt = frame_rx.borrow().clone();
                if let Some(frame) = frame_opt {
                    tokio::task::block_in_place(|| {
                        process_frame(&state, &detector, frame.as_ref());
                    });
                }
            }
        }
    }
}

fn process_frame(state: &SharedState, detector: &OcrDetector, frame: &RgbaImage) {
    let width = frame.width();
    let height = frame.height();

    // Crop down to the bottom third of the image where the text is located
    let row_start = (height * 2) / 3;
    let cropped_height = height - row_start;
    
    // RgbaImage uses raw bytes, 4 bytes per pixel
    let bytes_per_pixel = 4;
    let byte_start = (row_start * width * bytes_per_pixel) as usize;
    
    // We clone just the relevant slice data since we need to mutate it for Binarization
    let mut raw_pixels = frame.to_vec();
    
    if byte_start < raw_pixels.len() {
        let cropped_pixels = &mut raw_pixels[byte_start..];
        
        // Binarization preprocessing
        for chunk in cropped_pixels.chunks_exact_mut(4) {
            let r = chunk[0];
            let g = chunk[1];
            let b = chunk[2];
            
            // Invert the white text and black graphics to generate crisp OCR contrasts
            if r > 200 && g > 200 && b > 200 {
                chunk[0] = 0; chunk[1] = 0; chunk[2] = 0;
            } else {
                chunk[0] = 255; chunk[1] = 255; chunk[2] = 255;
            }
        }

        if let Some(race_num) = detector.parse_race_number(width, cropped_height, cropped_pixels) {
            // Update centralized state tree immediately across all contexts
            let mut st = futures::executor::block_on(state.lock());
            if st.try_update_race(race_num) {
                println!("[RaceTracker Agent] Detected Race {}! (Cooldown activated for 2 minutes)", race_num);
            } else if st.state.current_race != race_num {
                println!("[RaceTracker Agent] Detected Race {} but ignored due to cooldown.", race_num);
            }
        }
    }
}
