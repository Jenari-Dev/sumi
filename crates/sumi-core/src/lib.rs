//! Shared logic for the Sumi CLI and daemon.

pub mod apps;
pub mod config;
pub mod features;
pub mod firmware;
pub mod hotkeys;
pub mod monitor;
pub mod multiframe;
pub mod profile_io;
pub mod state;
pub mod strings;
#[cfg(any(test, feature = "test-support"))]
pub mod testing;
pub mod transport;

pub use config::Config;
pub use monitor::{Monitor, ProductLine};
pub use state::{FeatureState, MonitorState};
pub use transport::Transport;
