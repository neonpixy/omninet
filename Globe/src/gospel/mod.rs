//! Gospel — evangelized discovery for the Omnidea relay network.
//!
//! Gospel ensures that registry records (name claims, relay hints)
//! propagate across the network through evangelization. When two peers
//! connect — whether client↔relay or relay↔relay — they exchange
//! registry records the other is missing, using timestamp-based sync
//! cursors for efficiency.
//!
//! # The Discovery Flow
//!
//! 1. Alice wants to find `bob.com`
//! 2. She checks her local [`GospelRegistry`] — if found, skip to step 4
//! 3. She queries connected relays for `bob.com`'s name claim
//! 4. She gets Bob's public key from the name claim event
//! 5. She queries for Bob's [`RelayHintRecord`] to find his relays
//! 6. She connects to Bob's relays and subscribes to his content
//!
//! Gospel ensures steps 3 and 5 work globally because registry records
//! evangelize between peered relays.
//!
//! # Components
//!
//! - [`GospelConfig`] — evangelize interval, capacity, peer URLs
//! - [`GospelRegistry`] — local cache with conflict resolution
//! - [`GospelSync`] — filter building and merge helpers
//! - [`GospelPeer`] — relay-to-relay peering connection
//! - [`HintBuilder`] — relay hint event construction
//! - [`RelayHintRecord`] — parsed relay hint data

pub mod config;
pub mod digest;
pub mod hints;
pub mod peer;
pub mod registry;
pub mod sync;
pub mod tier;

pub use config::{GospelConfig, NamePolicy};
pub use hints::{parse_hint, HintBuilder, RelayHintRecord};
pub use peer::{GospelPeer, PeerState};
pub use registry::{GospelRegistry, InsertResult, RegistrySnapshot};
pub use sync::{GospelSync, MergeStats};
pub use tier::{GospelTier, gospel_tier, kinds_for_tiers};
pub use digest::{ConceptEquivalence, SemanticDigest, SynapseEdge};
