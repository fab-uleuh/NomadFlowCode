use std::time::Duration;

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};

use nomadflow_core::models::{Feature, Repository};

/// Application events combining terminal events and async results.
#[derive(Debug)]
pub enum AppEvent {
    /// Terminal key event.
    Key(KeyEvent),
    /// Terminal tick (for animations/spinners).
    Tick,
    /// Repos loaded from API.
    ReposLoaded(Result<Vec<Repository>, String>),
    /// Features loaded from API.
    FeaturesLoaded(Result<Vec<Feature>, String>),
    /// Feature created via API.
    FeatureCreated(Result<String, String>),
    /// Switch feature completed.
    SwitchDone(Result<String, String>),
    /// Health check result for a server.
    HealthResult(String, bool),
}

/// Poll for crossterm events with a timeout.
pub fn poll_event(timeout: Duration) -> Option<AppEvent> {
    if event::poll(timeout).unwrap_or(false) {
        if let Ok(evt) = event::read() {
            return match evt {
                CrosstermEvent::Key(key) => Some(AppEvent::Key(key)),
                _ => None,
            };
        }
    }
    None
}
