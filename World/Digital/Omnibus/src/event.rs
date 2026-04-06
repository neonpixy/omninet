//! Omnibus lifecycle events.
//!
//! Broadcast channel events emitted by the Omnibus runtime. External consumers
//! (daemon, tray app, bridge) subscribe via `Omnibus::subscribe_events()` to
//! react to lifecycle changes without polling.

/// A lifecycle event emitted by the Omnibus runtime.
#[derive(Clone, Debug)]
pub enum OmnibusEvent {
    /// A peer connected to the relay pool.
    PeerConnected { pubkey: String },
    /// A peer disconnected from the relay pool.
    PeerDisconnected { pubkey: String },
    /// An event was received from the relay pool.
    EventReceived { event_json: String },
    /// The health score changed.
    HealthChanged { score: f64 },
    /// The Omnibus runtime has started.
    Started,
    /// The Omnibus runtime is stopping.
    Stopped,
}
