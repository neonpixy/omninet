//! DeviceManager — multi-device pairing, fleet management, and sync coordination.
//!
//! Part of the Undercroft meta-layer. DeviceManager wraps Globe's
//! pairing data types with real cryptographic verification using
//! Crown's BIP-340 Schnorr signatures, and provides fleet tracking
//! and sync coordination for all paired devices.
//!
//! # Modules
//!
//! - [`error`] — Error types for the device management subsystem.
//! - [`pairing`] — Pairing protocol: initiate, respond, verify.
//! - [`fleet`] — Device fleet registry and health tracking.
//! - [`sync`] — Sync priority, state tracking, and conflict detection.
//!
//! # Example
//!
//! ```
//! use device_manager::pairing::PairingProtocol;
//! use crown::CrownKeypair;
//!
//! let initiator = CrownKeypair::generate();
//! let responder = CrownKeypair::generate();
//!
//! // Step 1: Initiator creates a challenge.
//! let challenge = PairingProtocol::initiate(&initiator, "MacBook", "ws://localhost:8080");
//!
//! // Step 2: Responder signs it.
//! let response = PairingProtocol::respond(&challenge, &responder, "iPhone").unwrap();
//!
//! // Step 3: Initiator verifies and gets a DevicePair.
//! let pair = PairingProtocol::verify(&challenge, &response).unwrap();
//! assert!(pair.active);
//! ```

pub mod error;
pub mod fleet;
pub mod pairing;
pub mod sync;

// Re-exports — key types.
pub use error::DeviceManagerError;
pub use fleet::{DeviceFleet, DeviceStatus, FleetEntry, FleetHealth};
pub use pairing::PairingProtocol;
pub use sync::{SyncPriority, SyncState, SyncTracker};
