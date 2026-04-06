//! Globe — Omnidea Relay Protocol (ORP).
//!
//! The connected world. Globe moves signed, encrypted messages between
//! people over the Omnidea relay network. It defines the wire protocol,
//! manages relay connections, and provides pub/sub event streaming.
//!
//! Globe is a transport layer — it doesn't understand the content it
//! carries. Events are signed envelopes. The ABC modules above Globe
//! (Fortune, Kingdom, etc.) interpret what's inside.
//!
//! # What Globe provides
//!
//! - **OmniEvent**: Content-addressed signed events (SHA-256 ID + Schnorr signature)
//! - **ORP wire protocol**: JSON array messages over WebSocket (text + binary frames)
//! - **Client**: Connect to relays, publish events, subscribe to streams
//! - **Server**: Accept relay connections, store events, serve subscriptions
//! - **OmniFilter**: Subscription filters with client-side matching
//! - **EventBuilder**: Unsigned → signed event pipeline via Crown
//! - **Naming**: Domain-style name registration and resolution
//! - **Auth**: Challenge-response relay authentication
//! - **Health**: Relay scoring and connection state tracking

// Shared protocol types (used by both client and server)
pub mod error;
pub mod config;
pub mod event;
pub mod event_id;
pub mod kind;
pub mod filter;
pub mod protocol;
pub mod auth;
pub mod event_builder;
pub mod name;
pub mod health;
pub mod gospel;
pub mod asset;
pub mod chunk;
pub mod signaling;
pub mod deeplink;
pub mod discovery;
pub mod commons;
pub mod collaboration;
pub mod camouflage;
pub mod idea_sync;
pub mod jurisdiction;
pub mod privacy;

// Client and server
pub mod client;
pub mod server;

// Re-exports — shared types
pub use error::GlobeError;
pub use config::GlobeConfig;
pub use event::OmniEvent;
pub use event_builder::{EventBuilder, UnsignedEvent};
pub use filter::OmniFilter;
pub use kind::Subsystem;
pub use protocol::{ClientMessage, RelayMessage};
pub use health::{ConnectionState, RelayHealth};
pub use name::{NameRecord, NameBuilder, NameParts, PaymentProof};

// Re-exports — gospel types
pub use gospel::{
    GospelConfig, GospelPeer, GospelRegistry, GospelSync,
    HintBuilder, RelayHintRecord, InsertResult, RegistrySnapshot,
    PeerState, MergeStats, GospelTier, gospel_tier, kinds_for_tiers,
    ConceptEquivalence, SemanticDigest, SynapseEdge, NamePolicy,
};

// Re-exports — deep linking types
pub use deeplink::{GlobeName, LinkBuilder, OmnideaUri, UriAction, UriHandler, UriRouter};

// Re-exports — client types
pub use client::connection::{RelayHandle, RelayEvent};
pub use client::pool::{RelayPool, PoolEvent};

// Re-exports — signaling types
pub use signaling::SignalingBuilder;

// Re-exports — discovery types
pub use discovery::network_key::{
    KeyRotation, NetworkKeyBuilder, NetworkKeyEnvelope, NetworkKeyMaterial, RotationReason,
};
pub use discovery::invitation::{Invitation, InvitationBuilder, InvitationLink};
pub use discovery::beacon::{BeaconBuilder, BeaconRecord};
pub use discovery::address::{AddressInfo, EncryptedAddresses};
pub use discovery::pairing::{DevicePair, PairingChallenge, PairingResponse, PairingStatus};
pub use discovery::profile::{
    ConnectionType, DeviceCondition, DeviceProfile, DeviceType, ServingPolicy,
};
#[cfg(feature = "upnp")]
pub use discovery::upnp::{PortMapper, PortMapping};

// Re-exports — asset types
pub use asset::{AssetBuilder, AssetRecord};

// Re-exports — collaboration types
pub use collaboration::{
    CollaborationConfig, CollaborationMessage, ControlAction,
    KIND_COLLABORATION, encode_message, decode_message,
};

// Re-exports — idea sync types
pub use idea_sync::{IdeaSyncFilter, IdeaSyncPayload, KIND_IDEA_OPS};

// Re-exports — commons types
pub use commons::{
    CommonsEvent, CommonsFilter, CommonsPolicy, CommonsPublishPolicy, CommonsTag,
    COMMONS_PUBLICATION,
};

// Re-exports — camouflage types
pub use camouflage::{
    CamouflageConfig, CamouflageMode, PaddingConfig, ShapedMessage,
    ShapingProfile, TrafficPadder, TrafficShaper,
};

// Re-exports — privacy types
pub use privacy::{BucketPaddingConfig, PaddingMode, pad_to_bucket};
pub use privacy::ShapingConfig;
pub use privacy::{
    ForwardAction, ForwardEnvelope, ForwardingConfig, RelayPath,
    build_forward_envelope, process_forward, validate_path,
};
pub use privacy::{
    AnonymousAuthResponse, AnonymousConfig, AnonymousFilter, EphemeralSession,
    create_anonymous_auth, create_ephemeral_session, strip_author_from_event,
};

// Re-exports — jurisdiction types
pub use jurisdiction::{
    DiversityRecommendation, JurisdictionAnalyzer, JurisdictionDiversity,
    LegalFramework, RelayJurisdiction,
};

// Re-exports — chunk types
pub use chunk::{ChunkBuilder, ChunkInfo, ChunkManifest};

// Re-exports — SFU types
pub use server::sfu::{
    ForwardTarget, LayerPreference, MediaLayer, SfuConfig, SfuParticipant,
    SfuRouter, SfuSession,
};

// Re-exports — server types
pub use server::database::RelayDatabase;
pub use server::storage::{EventStore, StoreConfig, StoreStats};
pub use server::asset_store::{AssetStore, AssetStoreConfig};
pub use server::listener::{EventFilter, RelayServer, SearchHandler, SearchHit, ServerConfig};

#[cfg(feature = "blocking")]
pub use client::blocking::BlockingGlobe;
