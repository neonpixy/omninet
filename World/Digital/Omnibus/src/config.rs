use std::path::PathBuf;

use globe::server::listener::ServerConfig;

use crate::privacy::PrivacyConfig;

/// Configuration for an Omnibus instance.
pub struct OmnibusConfig {
    /// Directory for persistent storage (Soul, relay database).
    /// If None, everything is in-memory (testing).
    pub data_dir: Option<PathBuf>,

    /// Human-readable device name for mDNS (e.g., "Sam's Mac").
    pub device_name: String,

    /// Port for the local relay server. 0 = OS-assigned.
    pub port: u16,

    /// Whether to bind to all interfaces (true = LAN reachable).
    pub bind_all: bool,

    /// Optional home node URL for persistent sync.
    /// Your content stays available there when your device sleeps.
    pub home_node: Option<url::Url>,

    /// Optional custom server configuration (for event filtering, etc.).
    /// When None, Omnibus uses ServerConfig::default().
    pub server_config: Option<ServerConfig>,

    /// Maximum number of log entries to capture in the ring buffer.
    /// Defaults to 1000.
    pub log_capture_capacity: usize,

    /// Privacy routing configuration.
    /// Controls how events at different sensitivity levels are routed
    /// through intermediary relays. Defaults to direct routing for all events.
    pub privacy: PrivacyConfig,

    /// Whether to attempt UPnP port mapping to make this node internet-reachable.
    ///
    /// Default: `false` — client-only mode, no open ports.
    ///
    /// From Covenant Core Art. 2: Sovereignty includes control over one's own
    /// device and network presence. Opening a port on the user's router is a
    /// network configuration change that requires explicit consent. The app
    /// layer must obtain consent (via Polity `ConsentScope::NetworkExposure`)
    /// before setting this to `true`.
    pub enable_upnp: bool,
}

impl Default for OmnibusConfig {
    fn default() -> Self {
        Self {
            data_dir: None,
            device_name: "Omnidea Device".into(),
            port: 0,
            bind_all: true,
            home_node: None,
            server_config: None,
            log_capture_capacity: 1000,
            privacy: PrivacyConfig::default(),
            enable_upnp: false,
        }
    }
}
