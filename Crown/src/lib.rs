//! Crown -- Identity for Omnidea.
//!
//! The royal identity. Crown is who you are: your keypair, your profile,
//! your social graph, your preferences. Everything lives in the [`Soul`].
//!
//! Crown is **zero-dep on other Omninet crates**. It defines types and
//! BIP-340 Schnorr crypto. Encryption and persistence are injected via
//! traits ([`SoulEncryptor`], [`RecoveryEncryptor`], [`SecretSharer`])
//! so the caller can plug in Sentinal or any other backend.
//!
//! # Quick start
//!
//! ```
//! use crown::{CrownKeypair, Keyring, Signature};
//!
//! // Generate a new identity
//! let mut keyring = Keyring::new();
//! let kp = keyring.generate_primary().unwrap();
//! let my_id = kp.crown_id().to_string();
//!
//! // Sign something
//! let sig = keyring.sign(b"hello omnidea").unwrap();
//! assert!(sig.verify_crown_id(b"hello omnidea", &my_id));
//! ```

pub mod blinding;
pub mod blinding_proof;
pub mod device_sync;
pub mod error;
pub mod founding_tree;
pub mod keypair;
pub mod keyring;
pub mod preferences;
pub mod profile;
pub mod recovery;
pub mod rotation;
pub mod signature;
pub mod social;
pub mod soul;
pub mod verification;

// -- Core identity --
pub use keypair::CrownKeypair;
pub use keyring::Keyring;
pub use signature::Signature;
pub use error::CrownError;

// -- Soul (profile + preferences + social graph) --
pub use soul::{Soul, SoulEncryptor};
pub use profile::{AvatarReference, Profile};
pub use preferences::{NotificationCategory, Preferences, Theme, Visibility};
pub use social::SocialGraph;
pub use verification::VerificationLevel;

// -- Key rotation --
pub use rotation::{PreviousKey, RotationAnnouncement, RotationChain};

// -- Recovery --
pub use recovery::{
    EncryptedKeyringBackup, KeyShare, RecoveryArtifacts, RecoveryConfig, RecoveryEncryptor,
    RecoveryMethod, SecretSharer, SocialRecoveryConfig,
};

// -- Blinding (context-specific pseudonymous keys) --
pub use blinding::BlindingContext;
pub use blinding_proof::BlindingProof;

// -- Device sync --
pub use device_sync::{SyncAccept, SyncOffer, SyncPayload, SyncStatus};

// -- Founding verification tree (R2C) --
pub use founding_tree::{
    AnomalyAlert, AnomalyThresholds, FoundingTree, TreeAnomaly, VerificationLineage,
};
