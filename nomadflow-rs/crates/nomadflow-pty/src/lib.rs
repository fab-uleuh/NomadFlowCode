pub mod actor;
pub mod error;
pub mod manager;
pub mod protocol;
pub mod snapshot;
pub mod types;

// Re-export primary types for ergonomic imports.
pub use actor::{PaneActor, PaneSpawnConfig};
pub use error::PtyError;
pub use manager::PaneManager;
pub use protocol::{ControlMsg, WsFrame};
pub use types::{
    ClientId, CreatePaneRequest, PaneEvent, PaneHandle, PaneId, PaneInfo, PaneLabel, PaneMsg,
};
