use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
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
            state: RaceState {
                current_race: 1,
                current_points: None,
            },
            last_detected_time: None,
        }
    }

    /// Update the current race if it has been more than 2 minutes since the last detection.
    pub fn try_update_race(&mut self, new_race: u32) -> bool {
        if new_race < self.state.current_race && self.state.current_race != 12 {
            println!("Got race {} but current is {}, ignoring update to prevent rollback.", new_race, self.state.current_race);
            return false;
        }

        let now = Instant::now();
        if let Some(last_time) = self.last_detected_time {
            // Cool off for 10s
            if now.duration_since(last_time) < Duration::from_secs(10) {
                return false;
            }
        }

        println!(
            "Match state updated: {}th race (was {})",
            new_race, self.state.current_race
        );
        self.state.current_race = new_race;
        self.last_detected_time = Some(now);
        true
    }

    /// Update the current points score. Unlike races, there is no strict cooldown
    /// to support potential mid-race re-read patches, but typically this is queried post-race.
    pub fn try_update_points(&mut self, points: u32) -> bool {
        if self.state.current_points == Some(points) {
            return false;
        }

        println!(
            "Running points updated: {} (was {:?})",
            points, self.state.current_points
        );
        self.state.current_points = Some(points);
        true
    }

    pub fn manual_update_race(&mut self, new_race: u32) {
        println!(
            "Manual override: {}th race (was {})",
            new_race, self.state.current_race
        );
        self.state.current_race = new_race;
        self.last_detected_time = Some(Instant::now());
    }

    pub fn manual_update_points(&mut self, points: u32) {
        println!(
            "Manual override running points: {} (was {:?})",
            points, self.state.current_points
        );
        self.state.current_points = Some(points);
    }
}

pub type SharedState = Arc<Mutex<AppState>>;
