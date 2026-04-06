//! Privacy — traffic analysis resistance and relay forwarding.
//!
//! Complements [`camouflage`](crate::camouflage) (random-range padding + timing)
//! with bucket-based size normalization. All messages in a bucket are the same
//! length, eliminating size-based correlation attacks.
//!
//! The [`relay_forward`] module adds multi-hop relay forwarding, enabling
//! messages to traverse a chain of relays before reaching their destination.
//! Combined with Sentinal's onion encryption, this provides unlinkability.

pub mod anonymous;
pub mod padding;
pub mod relay_forward;
pub mod shaping;

pub use anonymous::{
    AnonymousAuthResponse, AnonymousConfig, AnonymousFilter, EphemeralSession,
    create_anonymous_auth, create_ephemeral_session, strip_author_from_event,
};
pub use padding::{BucketPaddingConfig, PaddingMode, pad_to_bucket, unpad};
pub use relay_forward::{
    ForwardAction, ForwardEnvelope, ForwardingConfig, RelayPath, build_forward_envelope,
    process_forward, validate_path,
};
pub use shaping::ShapingConfig;
