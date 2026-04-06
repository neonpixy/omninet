//! Globe client — connects to relays, publishes events, subscribes.

pub mod connection;
pub mod pool;

#[cfg(feature = "blocking")]
pub mod blocking;
