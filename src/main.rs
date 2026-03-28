mod agents;
mod capture;
mod ocr;
mod state;
mod web;

use crate::state::AppState;
use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use tokio_util::sync::CancellationToken;
use tokio::io::AsyncBufReadExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Starting Match Counter Detector Event Bus...");

    // 1. Initialize Global Shutdown Token
    let cancel_token = CancellationToken::new();

    // 2. Initialize Shared State Memory (JSON Payload)
    let state = Arc::new(Mutex::new(AppState::new()));

    // 3. Initialize Heavy OCR Backend Models
    let detector = Arc::new(ocr::OcrDetector::new().await?);

    // 4. Create the Global Frame Broadcast Channel
    let (frame_tx, frame_rx) = watch::channel(None);

    println!("Bootstrapping Multi-Agent Microservices...");

    // 5. Spawn the Telemetry Publisher (Raw Image Scraper)
    let publisher_token = cancel_token.clone();
    tokio::spawn(async move {
        capture::start_publisher(frame_tx, publisher_token).await;
    });

    // 6. Spawn the Isolated Race Tracker Agent
    let state_clone = state.clone();
    let tracker_token = cancel_token.clone();
    let detector_clone = detector.clone(); // Clone ARC wrappers for secondary agents
    let frame_rx_clone = frame_rx.clone();
    tokio::spawn(async move {
        // Because the OCR takes massive CPU logic, we unblock the tokio worker loop
        agents::race_tracker::start(state_clone, frame_rx_clone, tracker_token, detector_clone)
            .await;
    });

    // 7. Spawn the New Mogi Post-Race Score Tracker
    // Currently hardcoded string, but can easily be bound to an .env or command line arg
    let target_alias = "tuan".to_string();
    let score_state_clone = state.clone();
    let score_tracker_token = cancel_token.clone();
    let score_detector_clone = detector.clone();
    tokio::spawn(async move {
        agents::post_race_score::start(
            score_state_clone,
            frame_rx,
            score_tracker_token,
            score_detector_clone,
            target_alias,
        )
        .await;
    });

    // 7.5. Spawn CLI Input Reader
    let cli_state_clone = state.clone();
    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let mut reader = tokio::io::BufReader::new(stdin).lines();
        
        while let Ok(Some(line)) = reader.next_line().await {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("race ") {
                if let Ok(race) = rest.parse::<u32>() {
                    let mut lock = cli_state_clone.lock().await;
                    lock.manual_update_race(race);
                } else {
                    println!("Invalid race count: {}", rest);
                }
            } else if let Some(rest) = line.strip_prefix("score ") {
                if let Ok(points) = rest.parse::<u32>() {
                    let mut lock = cli_state_clone.lock().await;
                    lock.manual_update_points(points);
                } else {
                    println!("Invalid points: {}", rest);
                }
            } else {
                println!("Unknown command. Use 'race <num>' or 'score <num>'");
            }
        }
    });

    // 8. Start Web Server and connect Shutdown Hook
    let app = web::create_router(state.clone());
    let addr = "127.0.0.1:3000";
    let listener = tokio::net::TcpListener::bind(addr).await?;

    println!("\n=========================================================================");
    println!("==== MKWORLD METRIC TRACKER SYSTEM ONLINE ====");
    println!("=========================================================================");
    println!("> INPUT SOURCE REQUIRED:");
    println!("  Please ensure OBS is configured with a 'Windowed Projector' stream.");
    println!("  The backend Publisher will automatically attach to any window named 'Projector'.");
    println!("  It will silently fallback to sleep-mode if OBS is closed.");
    println!("");
    println!("> BROWSER SOURCES EXPOSED (Use transparent 800x600 widget bounds):");
    println!("  - Race Counter Tracker : http://127.0.0.1:3000/");
    println!("  - Points Mogi Tracker  : http://127.0.0.1:3000/points.html");
    println!("=========================================================================\n");
    println!("Awaiting web connections and Projector telemetry... (Press Ctrl+C to exit)\n");

    // Block on webserver with graceful exit future
    let shutdown_token = cancel_token.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to bind CTRL+C handler");
            println!("\n[System Manager] User invoked SIGINT. Broadcasting cancellation tokens...");
            shutdown_token.cancel();
        })
        .await?;

    println!("[System Manager] Axum runtime halted. Subscribed agents terminating in 1 second...");
    // Give async tasks slightly enough time to flush their `println!` shutdown notifications before total exit.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    println!("Goodbye.");

    Ok(())
}
