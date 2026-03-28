use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::time::{Instant, Duration};
use tokio::sync::Mutex;

#[derive(Clone, Serialize, Deserialize)]
pub struct RaceState {
    pub current_race: u32,
    pub current_points: Option<u32>,
}

pub struct AppState {
    pub state: RaceState,
    pub last_detected_time: Option<Instant>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            state: RaceState { current_race: 1, current_points: None },
            last_detected_time: None,
        }
    }

    /// Update the current race if it has been more than 2 minutes since the last detection.
    pub fn try_update_race(&mut self, new_race: u32) -> bool {
        let now = Instant::now();
        if let Some(last_time) = self.last_detected_time {
            // 2 minutes cooldown
            if now.duration_since(last_time) < Duration::from_secs(120) {
                return false;
            }
        }
        
        println!("Match state updated: {}th race (was {})", new_race, self.state.current_race);
        self.state.current_race = new_race;
        self.last_detected_time = Some(now);
        true
    }

    /// Update the current points score. Unlike races, there is no strict 2 minute cooldown
    /// to support potential mid-race re-read patches, but typically this is queried post-race.
    pub fn try_update_points(&mut self, points: u32) -> bool {
        if self.state.current_points == Some(points) {
            return false;
        }

        println!("Running points updated: {} (was {:?})", points, self.state.current_points);
        self.state.current_points = Some(points);
        true
    }
}

pub type SharedState = Arc<Mutex<AppState>>;
