//! Omnibus — the shared node runtime for every Throne app.
//!
//! Every Omnidea app embeds an Omnibus. It handles:
//! - Local relay server (your device is a node)
//! - mDNS discovery (find other devices on the network)
//! - Identity lifecycle (Keyring + Soul)
//! - Relay pool (connect to discovered peers and home node)
//! - Event routing (publish and subscribe)
//!
//! Omnibus operates standalone — a phone-only user is a full participant.
//! Optionally connects to a home node (your desktop, a friend, or a community)
//! for persistence when your device sleeps.

mod config;
pub mod daemon_config;
mod error;
mod event;
mod health_snapshot;
mod log_capture;
pub mod privacy;
mod runtime;
mod status;

pub use config::OmnibusConfig;
pub use daemon_config::{DaemonConfig, DaemonConfigError, OmnibusSection, TowerSection};
pub use error::OmnibusError;
pub use event::OmnibusEvent;
pub use health_snapshot::RelayHealthSnapshot;
pub use log_capture::{LogCapture, LogEntry};
pub use privacy::{PrivacyConfig, RouteStrategy, Sensitivity};
pub use runtime::Omnibus;
pub use status::OmnibusStatus;

// Re-export from Globe for convenience.
pub use globe::StoreStats;
pub use globe::UnsignedEvent;
