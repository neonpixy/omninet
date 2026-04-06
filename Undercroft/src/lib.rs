//! # Undercroft -- The Vaulted Chamber Beneath the Castle
//!
//! System health observatory. Aggregates deidentified metrics from Globe
//! (network), Kingdom (communities), Fortune (economics), and Bulwark
//! (collective health) into a unified health view.
//!
//! Undercroft observes, never controls. All data is aggregate: counts,
//! rates, and averages. NO pubkeys, NO individual activity, NO URLs,
//! NO location data. This is a Covenant requirement.
//!
//! HQ is the app that peers into the Undercroft.
//!
//! ## Covenant Alignment
//!
//! **Dignity** -- health metrics protect communities without surveilling individuals.
//! **Sovereignty** -- every person's data remains their own; only aggregate signals flow here.
//! **Consent** -- the observatory is transparent and auditable; opacity is breach.

pub mod community;
pub mod economic;
pub mod error;
pub mod federation_scope;
pub mod metrics;
pub mod network;
pub mod privacy_health;
pub mod quest;
pub mod snapshot;

// Re-exports for convenience.
pub use community::{CommunityHealth, GovernanceActivity};
pub use economic::EconomicHealth;
pub use error::UndercraftError;
pub use metrics::HealthMetrics;
pub use network::NetworkHealth;
pub use privacy_health::RelayPrivacyHealth;
pub use quest::QuestHealth;
pub use snapshot::{HealthHistory, HealthSnapshot};
pub use federation_scope::FederationScope;
