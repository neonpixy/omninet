//! Tower — always-on network nodes for the Omnidea network.
//!
//! Tower nodes are the infrastructure backbone. They run headless,
//! serving the network 24/7. Three modes:
//!
//! - **Pharos** — lightweight directory nodes. Gospel records only.
//!   Raspberry Pi territory. Caches names, relay hints, beacons,
//!   and lighthouse announcements. Rejects non-gospel content.
//!
//! - **Harbor** — community content nodes. Everything Pharos does,
//!   plus stores and serves content for member communities.
//!   A community's Harbor is where content lives when members sleep.
//!
//! - **Intermediary** — privacy-preserving forwarding nodes. Receives
//!   events and forwards them to an upstream relay after applying
//!   privacy transforms (timestamp jitter, metadata stripping, decoy
//!   injection). Does NOT store or index events locally.

mod announcement;
mod config;
mod error;
mod peering;
pub mod privacy_transforms;
mod runtime;

pub use announcement::TowerAnnouncement;
pub use config::{TowerConfig, TowerMode};
pub use error::TowerError;
pub use peering::PeeringLoop;
pub use privacy_transforms::PrivacyTransforms;
pub use runtime::Tower;
