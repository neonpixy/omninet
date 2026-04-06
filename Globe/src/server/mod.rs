//! Globe server — accepts relay connections, stores events, serves subscriptions.
//!
//! Every Omnidea device can run a relay server. Desktop/laptop machines can
//! run always-on relays. Mobile devices relay while the app is open.

pub mod asset_fetch;
pub mod asset_http;
pub mod asset_store;
pub mod database;
pub mod storage;
pub mod session;
pub mod listener;
pub mod network_defense;
pub mod sfu;
