use crate::ocr::OcrDetector;
use crate::state::SharedState;
use image::RgbaImage;
use regex::Regex;
use std::sync::Arc;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

pub async fn start(
    state: SharedState,
    mut frame_rx: watch::Receiver<Option<Arc<RgbaImage>>>,
    token: CancellationToken,
    detector: Arc<OcrDetector>,
    player_alias: String,
) {
    println!("[PostRaceScore Agent] Online. Tracking points for player alias: '{}'", player_alias);

    // Regex to capture trailing digits at the very end of the string (e.g. "RunnerXxX    154").
    // We make it robust to random OCR whitespace glitches before the integer.
    let re = Regex::new(r"(?i)(?:\s+|^)(\d{1,3})\s*$").unwrap();

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                println!("[PostRaceScore Agent] Shutdown signal received.");
                break;
            }
            res = frame_rx.changed() => {
                if res.is_err() { break; }
                
                let frame_opt = frame_rx.borrow().clone();
                if let Some(frame) = frame_opt {
                    tokio::task::block_in_place(|| {
                        process_scoreboard(&state, &detector, frame.as_ref(), &player_alias, &re);
                    });
                }
            }
        }
    }
}

fn process_scoreboard(
    state: &SharedState,
    detector: &OcrDetector,
    frame: &RgbaImage,
    player_alias: &str,
    re: &Regex,
) {
    let width = frame.width();
    let height = frame.height();

    // The actual "overall points" scoreboard floats in the middle-right of the screen.
    // We crop roughly the horizontal center-to-right to ignore UI backgrounds,
    // and clip top/bottom UI headers to give OCR a clean, isolated text matrix grid.
    let x_start = (width as f32 * 0.3) as usize;
    let x_end   = (width as f32 * 0.95) as usize;
    let y_start = (height as f32 * 0.15) as usize;
    let y_end   = (height as f32 * 0.85) as usize;

    let crop_w = (x_end - x_start) as u32;
    let crop_h = (y_end - y_start) as u32;

    let mut cropped_pixels = Vec::with_capacity((crop_w * crop_h * 4) as usize);

    for y in y_start..y_end {
        for x in x_start..x_end {
            let pixel = frame.get_pixel(x as u32, y as u32);
            let r = pixel[0];
            let g = pixel[1];
            let b = pixel[2];

            // Binarization threshold specifically designed to pull out the white scores/names
            // against the dark glass/transparent backgrounds of the post game scoreboard.
            if r > 180 && g > 180 && b > 180 {
                // Highlighting the text by turning backgrounds stark white and text stark black
                cropped_pixels.extend_from_slice(&[0, 0, 0, 255]);
            } else {
                cropped_pixels.extend_from_slice(&[255, 255, 255, 255]);
            }
        }
    }

    if let Some(text) = detector.extract_raw_text(crop_w, crop_h, &cropped_pixels) {
        let lines: Vec<&str> = text.split('\n').collect();
        for line in lines {
            let lower_line = line.to_lowercase();
            // Search the line for our player target
            if lower_line.contains(&player_alias.to_lowercase()) {
                // If found, isolate the trailing number at the far edge
                if let Some(caps) = re.captures(&lower_line) {
                    if let Ok(points) = caps[1].parse::<u32>() {
                        let mut st = futures::executor::block_on(state.lock());
                        st.try_update_points(points);
                    }
                }
            }
        }
    }
}
