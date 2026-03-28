use std::time::{Duration, Instant};
use std::sync::Arc;
use image::RgbaImage;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use xcap::Window;

pub async fn start_publisher(
    frame_tx: watch::Sender<Option<Arc<RgbaImage>>>,
    token: CancellationToken,
) {
    let mut last_scan = Instant::now();
    let mut printed_wait_message = false;

    println!("[Publisher] Telemetry Engine initialized. Looking for 'Projector' window...");

    loop {
        // Shutdown check
        if token.is_cancelled() {
            println!("[Publisher] Shutdown signal received. Halting telemetry capture stream.");
            break;
        }

        // Limit capture rate to 1 FPS uniformly
        let now = Instant::now();
        if now.duration_since(last_scan) < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        }

        // Window polling
        let windows = match Window::all() {
            Ok(w) => w,
            Err(_) => {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
        };

        let target_window = windows.into_iter().find(|w| w.title().unwrap_or_default().to_lowercase().contains("projector"));

        if let Some(window) = target_window {
            if !printed_wait_message {
                println!("[Publisher] Successfully hooked window: '{}'", window.title().unwrap_or_default());
                printed_wait_message = true;
            }

            if let Ok(frame) = window.capture_image() {
                // Instantly blast the image out across the async cross-thread watch channel to all subscribers!
                // We wrap it in Arc so OCR threads don't clone the entire 12 megabyte uncompressed payload, saving CPU.
                let _ = frame_tx.send(Some(Arc::new(frame)));
            } else {
                if printed_wait_message {
                    println!("[Publisher] Lost stream handle to '{}', is it minimized?", window.title().unwrap_or_default());
                    printed_wait_message = false;
                }
            }
        } else {
            if printed_wait_message {
                println!("[Publisher] Lost track of 'Projector' window! Pausing telemetry publisher...");
                printed_wait_message = false;
            }
        }

        last_scan = Instant::now();
    }
}
