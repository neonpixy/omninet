use serde::Serialize;

/// Current status of an Omnibus instance.
#[derive(Clone, Debug, Serialize)]
pub struct OmnibusStatus {
    /// Whether an identity is loaded.
    pub has_identity: bool,
    /// The user's public key (crown_id), if identity is loaded.
    pub pubkey: Option<String>,
    /// The user's display name, if set.
    pub display_name: Option<String>,
    /// Local relay server port.
    pub relay_port: u16,
    /// Number of active connections to the local relay.
    pub relay_connections: u32,
    /// WebSocket URL of the local relay.
    pub relay_url: String,
    /// Number of peers discovered via mDNS.
    pub discovered_peers: u32,
    /// Number of relays in the pool (peers we're connected to).
    pub pool_relays: u32,
    /// Whether connected to a home node.
    pub has_home_node: bool,
    /// Public URL if UPnP port mapping succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_url: Option<String>,
}
