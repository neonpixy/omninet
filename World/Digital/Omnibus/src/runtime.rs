use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crown::{CrownKeypair, Keyring, Soul};
use globe::client::pool::{PoolEvent, RelayPool};
use globe::discovery::local::{LocalAdvertiser, LocalBrowser, LocalPeer};
#[cfg(feature = "upnp")]
use globe::discovery::upnp::PortMapper;
use globe::event::OmniEvent;
use globe::filter::OmniFilter;
use globe::server::listener::RelayServer;
use globe::EventBuilder;
use globe::GlobeConfig;
use globe::StoreStats;
use globe::UnsignedEvent;
use tokio::sync::broadcast;
use url::Url;

use crate::config::OmnibusConfig;
use crate::error::OmnibusError;
use crate::event::OmnibusEvent;
use crate::health_snapshot::RelayHealthSnapshot;
use crate::log_capture::{LogCapture, LogEntry};
use crate::status::OmnibusStatus;

/// The shared node runtime. Every Throne app embeds one.
pub struct Omnibus {
    runtime: Arc<tokio::runtime::Runtime>,
    server: RelayServer,
    server_addr: SocketAddr,
    pool: Mutex<RelayPool>,
    keyring: Mutex<Keyring>,
    soul: Mutex<Option<Soul>>,
    _advertiser: Option<LocalAdvertiser>,
    #[cfg(feature = "upnp")]
    _port_mapper: Option<PortMapper>,
    browser: LocalBrowser,
    home_node: Mutex<Option<Url>>,
    log_capture: Arc<Mutex<LogCapture>>,
    config: OmnibusConfig,
    events_tx: broadcast::Sender<OmnibusEvent>,
    stopped: AtomicBool,
}

impl Omnibus {
    /// Start an Omnibus instance.
    ///
    /// This boots the full node: relay server, mDNS discovery, relay pool.
    /// Identity is NOT loaded yet — call `create_identity` or `load_identity` next.
    pub fn start(config: OmnibusConfig) -> Result<Self, OmnibusError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| OmnibusError::ServerFailed(format!("tokio runtime: {e}")))?;

        let runtime = Arc::new(runtime);

        // Start the relay server.
        let host = if config.bind_all { "0.0.0.0" } else { "127.0.0.1" };
        let addr: SocketAddr = format!("{host}:{}", config.port)
            .parse()
            .map_err(|e| OmnibusError::ServerFailed(format!("bad address: {e}")))?;

        let server_config = config.server_config.clone().unwrap_or_default();
        let (server, server_addr) = runtime
            .block_on(RelayServer::start_at(addr, server_config))
            .map_err(|e| OmnibusError::ServerFailed(e.to_string()))?;

        log::info!("omnibus: relay server on {server_addr}");

        // Attempt UPnP port mapping (non-fatal).
        // Gated behind `enable_upnp` — opening a port on the user's router
        // requires explicit consent (Polity ConsentScope::NetworkExposure).
        #[cfg(feature = "upnp")]
        let port_mapper = if config.bind_all && config.enable_upnp {
            runtime.block_on(async {
                PortMapper::map(server_addr.port(), Arc::clone(&runtime)).await
            })
        } else {
            if config.bind_all && !config.enable_upnp {
                log::info!("omnibus: UPnP disabled (enable_upnp=false) — client-only mode");
            }
            None
        };

        #[cfg(feature = "upnp")]
        if let Some(ref mapper) = port_mapper {
            if let Some(ref mapping) = mapper.mapping() {
                log::info!("omnibus: Tower publicly reachable at {}", mapping.public_url);
            }
        } else if config.bind_all && config.enable_upnp {
            log::info!("omnibus: UPnP unavailable — Tower is local only");
        }

        // Start mDNS browser (always).
        let browser = LocalBrowser::start()
            .map_err(|e| OmnibusError::DiscoveryFailed(e.to_string()))?;

        // Start mDNS advertiser.
        let advertiser =
            match LocalAdvertiser::start(&config.device_name, server_addr.port(), None) {
                Ok(adv) => Some(adv),
                Err(e) => {
                    log::warn!("omnibus: mDNS advertiser failed (non-fatal): {e}");
                    None
                }
            };

        // Create relay pool and connect to own server.
        let _guard = runtime.enter();
        let mut pool = RelayPool::new(GlobeConfig::default());

        let own_url: Url = format!("ws://127.0.0.1:{}", server_addr.port())
            .parse()
            .expect("ws://127.0.0.1:<port> is always a valid URL");
        if let Err(e) = pool.add_relay(own_url) {
            log::warn!("omnibus: failed to connect pool to own relay: {e}");
        }

        // Connect to home node if configured.
        let home_node = if let Some(ref home_url) = config.home_node {
            if let Err(e) = pool.add_relay(home_url.clone()) {
                log::warn!("omnibus: failed to connect to home node: {e}");
            }
            Some(home_url.clone())
        } else {
            None
        };

        let log_capture = Arc::new(Mutex::new(LogCapture::new(config.log_capture_capacity)));

        let (events_tx, _) = broadcast::channel::<OmnibusEvent>(256);

        let omnibus = Self {
            runtime,
            server,
            server_addr,
            pool: Mutex::new(pool),
            keyring: Mutex::new(Keyring::new()),
            soul: Mutex::new(None),
            _advertiser: advertiser,
            #[cfg(feature = "upnp")]
            _port_mapper: port_mapper,
            browser,
            home_node: Mutex::new(home_node),
            log_capture,
            config,
            events_tx: events_tx.clone(),
            stopped: AtomicBool::new(false),
        };

        let _ = events_tx.send(OmnibusEvent::Started);

        Ok(omnibus)
    }

    // =================================================================
    // Identity
    // =================================================================

    /// Create a new identity with a display name.
    ///
    /// Generates a keypair, creates a Soul with the given name,
    /// and publishes a profile event to connected relays.
    pub fn create_identity(&self, display_name: &str) -> Result<String, OmnibusError> {
        let mut keyring = self.keyring.lock().unwrap_or_else(|e| e.into_inner());
        keyring.generate_primary()?;

        let crown_id = keyring
            .public_key()
            .map(String::from)
            .map_err(|e| OmnibusError::IdentityFailed(e.to_string()))?;

        // Keyring persistence is the caller's responsibility.
        // The daemon saves to its own data_dir (not the Tower's).
        drop(keyring);

        // Create Soul in memory. Disk persistence is the caller's responsibility.
        let mut profile = crown::Profile::empty();
        profile.display_name = Some(display_name.into());
        let mut soul = Soul::new();
        soul.update_profile(profile);

        *self.soul.lock().unwrap_or_else(|e| e.into_inner()) = Some(soul);

        // Publish profile event.
        if let Err(e) = self.publish_profile() {
            log::warn!("omnibus: failed to publish profile: {e}");
        }

        Ok(crown_id)
    }

    /// Load an existing identity from disk.
    pub fn load_identity(&self, path: &str) -> Result<String, OmnibusError> {
        let soul_dir = std::path::Path::new(path).join("soul");
        let soul = Soul::load(&soul_dir, None)?;
        *self.soul.lock().unwrap_or_else(|e| e.into_inner()) = Some(soul);

        // Try loading keyring from saved data.
        let keyring_path = std::path::Path::new(path).join("keyring.dat");
        if keyring_path.exists() {
            let data = std::fs::read(&keyring_path)
                .map_err(|e| OmnibusError::IdentityFailed(format!("read keyring: {e}")))?;
            let mut keyring = self.keyring.lock().unwrap_or_else(|e| e.into_inner());
            keyring.load(&data)?;
        }

        let keyring = self.keyring.lock().unwrap_or_else(|e| e.into_inner());
        let crown_id = keyring
            .public_key()
            .map(String::from)
            .map_err(|e| OmnibusError::IdentityFailed(e.to_string()))?;

        Ok(crown_id)
    }

    /// Clear the in-memory identity (keyring + soul).
    ///
    /// Does NOT delete files from disk — the caller handles that.
    /// After this, `pubkey()` returns `None` and `profile_json()` returns `None`.
    pub fn clear_identity(&self) {
        self.keyring.lock().unwrap_or_else(|e| e.into_inner()).lock();
        *self.soul.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// Get the current profile as JSON, or None if no identity.
    pub fn profile_json(&self) -> Option<String> {
        let soul = self.soul.lock().unwrap_or_else(|e| e.into_inner());
        let soul = soul.as_ref()?;
        serde_json::to_string(soul.profile()).ok()
    }

    /// Update the display name and re-publish profile.
    pub fn update_display_name(&self, name: &str) -> Result<(), OmnibusError> {
        {
            let mut soul_guard = self.soul.lock().unwrap_or_else(|e| e.into_inner());
            let soul = soul_guard.as_mut().ok_or(OmnibusError::NoIdentity)?;
            let mut profile = soul.profile().clone();
            profile.display_name = Some(name.into());
            soul.update_profile(profile);
            if let Err(e) = soul.save() {
                log::warn!("omnibus: soul save failed: {e}");
            }
        }
        self.publish_profile()
    }

    /// Get the public key (crown_id bech32), or None if no identity.
    pub fn pubkey(&self) -> Option<String> {
        self.keyring
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .public_key()
            .ok()
            .map(String::from)
    }

    /// Get the public key as hex, or None if no identity.
    pub fn pubkey_hex(&self) -> Option<String> {
        self.keyring
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .public_key_hex()
            .ok()
    }

    /// Export the keyring as JSON bytes (for syncing to a standalone keyring).
    pub fn export_keyring(&self) -> Result<Vec<u8>, OmnibusError> {
        let keyring = self.keyring.lock().unwrap_or_else(|e| e.into_inner());
        keyring.export().map_err(OmnibusError::Crown)
    }

    /// Import a keyring from exported bytes (for syncing from another device).
    pub fn import_keyring(&self, data: &[u8]) -> Result<(), OmnibusError> {
        let mut keyring = self.keyring.lock().unwrap_or_else(|e| e.into_inner());
        keyring.load(data).map_err(OmnibusError::Crown)
    }

    // =================================================================
    // Network
    // =================================================================

    /// Set a home node for persistent sync.
    pub fn set_home_node(&self, url: &str) -> Result<(), OmnibusError> {
        let parsed: Url = url
            .parse()
            .map_err(|e| OmnibusError::NetworkFailed(format!("bad URL: {e}")))?;

        let _guard = self.runtime.enter();
        let mut pool = self.pool.lock().unwrap_or_else(|e| e.into_inner());
        pool.add_relay(parsed.clone())?;

        *self.home_node.lock().unwrap_or_else(|e| e.into_inner()) = Some(parsed);
        Ok(())
    }

    /// Connect to a specific relay (e.g., a discovered peer).
    pub fn connect_relay(&self, url: &str) -> Result<(), OmnibusError> {
        let parsed: Url = url
            .parse()
            .map_err(|e| OmnibusError::NetworkFailed(format!("bad URL: {e}")))?;

        let _guard = self.runtime.enter();
        let mut pool = self.pool.lock().unwrap_or_else(|e| e.into_inner());
        pool.add_relay(parsed)?;
        Ok(())
    }

    /// Publish an event to all connected relays.
    pub fn publish(&self, event: OmniEvent) -> Result<(), OmnibusError> {
        let pool = self.pool.lock().unwrap_or_else(|e| e.into_inner());
        self.runtime.block_on(pool.publish(event))?;
        Ok(())
    }

    /// Publish a text note. Signs it with the loaded identity.
    pub fn post(&self, content: &str) -> Result<OmniEvent, OmnibusError> {
        let keypair = self.extract_keypair()?;
        let event = EventBuilder::text_note(content, &keypair)?;

        // Also inject into own server for local queries.
        self.server.store().insert(event.clone());

        self.publish(event.clone())?;
        Ok(event)
    }

    /// Sign an unsigned event with the loaded Crown identity.
    ///
    /// Creates a complete, signed OmniEvent ready for publishing.
    /// This is the canonical way for daemon modules to create signed events
    /// without direct access to the Crown keypair.
    pub fn sign_event(&self, unsigned: &UnsignedEvent) -> Result<OmniEvent, OmnibusError> {
        let keypair = self.extract_keypair()?;
        EventBuilder::sign(unsigned, &keypair).map_err(OmnibusError::Globe)
    }

    /// Sign an unsigned event, insert it into the local relay store,
    /// broadcast it to connected relay sessions, and publish it to the pool.
    pub fn sign_and_publish(&self, unsigned: &UnsignedEvent) -> Result<OmniEvent, OmnibusError> {
        let event = self.sign_event(unsigned)?;
        self.server.store().insert(event.clone());
        // Broadcast directly to connected relay sessions (remote peers).
        // Without this, events published by the relay's own Omnibus never
        // reach remote peers — the pool's dedup cache marks them as "seen"
        // before they can loop back through the session broadcast path.
        self.server.broadcast_live(event.clone());
        self.publish(event.clone())?;
        Ok(event)
    }

    /// Subscribe to events matching filters.
    /// Returns (subscription_id, receiver).
    pub fn subscribe(
        &self,
        filters: Vec<OmniFilter>,
    ) -> (String, broadcast::Receiver<PoolEvent>) {
        let _guard = self.runtime.enter();
        let mut pool = self.pool.lock().unwrap_or_else(|e| e.into_inner());
        pool.subscribe(filters)
    }

    /// Get a broadcast receiver for ALL events (no filter).
    pub fn event_stream(&self) -> broadcast::Receiver<PoolEvent> {
        let _guard = self.runtime.enter();
        let pool = self.pool.lock().unwrap_or_else(|e| e.into_inner());
        pool.subscribe_events()
    }

    /// Seed an event directly into the local relay's store.
    pub fn seed_event(&self, event: OmniEvent) {
        self.server.store().insert(event);
    }

    // =================================================================
    // Discovery
    // =================================================================

    /// Get all currently discovered peers on the local network.
    pub fn peers(&self) -> Vec<LocalPeer> {
        self.browser.peers()
    }

    /// Connect to all currently discovered peers.
    pub fn connect_discovered_peers(&self) -> u32 {
        let peers = self.browser.peers();
        let mut connected = 0u32;
        for peer in &peers {
            if let Some(ws_url) = peer.ws_url() {
                if self.connect_relay(&ws_url).is_ok() {
                    connected += 1;
                }
            }
        }
        connected
    }

    // =================================================================
    // Status
    // =================================================================

    /// Get the current status of this Omnibus instance.
    pub fn status(&self) -> OmnibusStatus {
        let keyring = self.keyring.lock().unwrap_or_else(|e| e.into_inner());
        let soul = self.soul.lock().unwrap_or_else(|e| e.into_inner());
        let home = self.home_node.lock().unwrap_or_else(|e| e.into_inner());

        let display_name = soul
            .as_ref()
            .and_then(|s| s.profile().display_name.clone());

        OmnibusStatus {
            has_identity: keyring.is_unlocked(),
            pubkey: keyring.public_key().ok().map(String::from),
            display_name,
            relay_port: self.server_addr.port(),
            relay_connections: self.server.active_connections() as u32,
            relay_url: format!("ws://{}", self.server_addr),
            discovered_peers: self.browser.peers().len() as u32,
            pool_relays: self.pool.lock().unwrap_or_else(|e| e.into_inner()).relay_count() as u32,
            has_home_node: home.is_some(),
            public_url: self.public_url(),
        }
    }

    /// The local relay server port.
    pub fn port(&self) -> u16 {
        self.server_addr.port()
    }

    /// The local relay server WebSocket URL.
    pub fn relay_url(&self) -> String {
        format!("ws://{}", self.server_addr)
    }

    /// The public URL of this node (if UPnP mapping succeeded).
    #[cfg(feature = "upnp")]
    pub fn public_url(&self) -> Option<String> {
        self._port_mapper.as_ref()?.public_url()
    }

    /// The public URL of this node. Always returns `None` when UPnP is disabled.
    #[cfg(not(feature = "upnp"))]
    pub fn public_url(&self) -> Option<String> {
        None
    }

    // =================================================================
    // Health & Diagnostics
    // =================================================================

    /// Get health snapshots for all relays in the pool.
    pub fn relay_health(&self) -> Vec<RelayHealthSnapshot> {
        let pool = self.pool.lock().unwrap_or_else(|e| e.into_inner());
        pool.relay_health()
            .iter()
            .map(RelayHealthSnapshot::from)
            .collect()
    }

    /// Get health snapshot for a specific relay by URL.
    pub fn relay_health_for(&self, url: &str) -> Option<RelayHealthSnapshot> {
        self.relay_health()
            .into_iter()
            .find(|h| h.url == url)
    }

    /// Get statistics about the local event store.
    pub fn store_stats(&self) -> StoreStats {
        self.server.store().stats()
    }

    // =================================================================
    // Log Capture
    // =================================================================

    /// Get the most recent log entries.
    pub fn recent_logs(&self, count: usize) -> Vec<LogEntry> {
        self.log_capture.lock().unwrap_or_else(|e| e.into_inner()).recent(count)
    }

    /// Push a log entry into the capture buffer.
    ///
    /// Designed for external callers (FFI layer, app layer) to feed logs.
    pub fn push_log(&self, entry: LogEntry) {
        self.log_capture.lock().unwrap_or_else(|e| e.into_inner()).push(entry);
    }

    /// Get a reference to the log capture buffer.
    ///
    /// Useful for FFI callback wiring where the app layer needs to hold
    /// onto the buffer and push entries from a `log::Log` implementation.
    pub fn log_capture(&self) -> Arc<Mutex<LogCapture>> {
        self.log_capture.clone()
    }

    // =================================================================
    // Gospel
    // =================================================================

    /// Get the persistent gospel registry (DB-backed, survives restarts).
    pub fn gospel_registry(&self) -> Option<&globe::gospel::GospelRegistry> {
        self.server.gospel_registry()
    }

    /// Save the gospel registry to the encrypted database.
    pub fn save_gospel(&self) {
        self.server.save_gospel();
    }

    // =================================================================
    // Query
    // =================================================================

    /// Query events from the local relay's store.
    pub fn query(&self, filter: &OmniFilter) -> Vec<OmniEvent> {
        self.server.store().query(filter)
    }

    // =================================================================
    // Runtime Access
    // =================================================================

    /// Access the tokio runtime. Used by FFI layer for spawning async tasks.
    pub fn runtime(&self) -> &Arc<tokio::runtime::Runtime> {
        &self.runtime
    }

    /// Access the configuration this instance was started with.
    pub fn config(&self) -> &OmnibusConfig {
        &self.config
    }

    // =================================================================
    // Internal
    // =================================================================

    /// Extract a CrownKeypair from the keyring for signing.
    fn extract_keypair(&self) -> Result<CrownKeypair, OmnibusError> {
        let keyring = self.keyring.lock().unwrap_or_else(|e| e.into_inner());
        let export = keyring
            .export()
            .map_err(|e| OmnibusError::IdentityFailed(format!("export: {e}")))?;

        let storage: serde_json::Value = serde_json::from_slice(&export)
            .map_err(|e| OmnibusError::IdentityFailed(format!("parse: {e}")))?;

        let hex_key = storage
            .get("primary_private_key")
            .and_then(|v| v.as_str())
            .ok_or(OmnibusError::NoIdentity)?;

        let key_bytes: Vec<u8> = (0..hex_key.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex_key[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| OmnibusError::IdentityFailed(format!("hex decode: {e}")))?;

        CrownKeypair::from_private_key(&key_bytes).map_err(OmnibusError::Crown)
    }

    /// Publish the current profile as a kind-0 event.
    fn publish_profile(&self) -> Result<(), OmnibusError> {
        let keypair = self.extract_keypair()?;
        let soul = self.soul.lock().unwrap_or_else(|e| e.into_inner());
        let soul = soul.as_ref().ok_or(OmnibusError::NoIdentity)?;

        let display_name = soul
            .profile()
            .display_name
            .as_deref()
            .unwrap_or("");
        let about = soul.profile().bio.as_deref();

        let event = EventBuilder::profile(display_name, about, None, &keypair)?;
        self.server.store().insert(event.clone());

        let pool = self.pool.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = self.runtime.block_on(pool.publish(event)) {
            log::warn!("omnibus: profile publish failed: {e}");
        }
        Ok(())
    }

    // =================================================================
    // Lifecycle
    // =================================================================

    /// Gracefully stop the Omnibus runtime.
    ///
    /// Saves the gospel registry, closes relay pool connections, and stops
    /// mDNS discovery. Safe to call multiple times — subsequent calls are no-ops.
    ///
    /// Does NOT shut down the tokio runtime (other things may still need it).
    pub fn stop(&self) {
        // Guard against double-stop.
        if self.stopped.swap(true, Ordering::SeqCst) {
            return;
        }

        log::info!("omnibus: stopping...");

        // Emit Stopped event before tearing down.
        let _ = self.events_tx.send(OmnibusEvent::Stopped);

        // 1. Save gospel registry.
        self.save_gospel();
        log::info!("omnibus: gospel saved");

        // 2. Close relay pool connections (dropping handles disconnects them).
        {
            let mut pool = self.pool.lock().unwrap_or_else(|e| e.into_inner());
            // Collect URLs first to avoid borrow conflict with remove_relay.
            let urls: Vec<Url> = pool
                .relay_health()
                .iter()
                .map(|h| h.url.clone())
                .collect();
            for url in &urls {
                pool.remove_relay(url);
            }
        }
        log::info!("omnibus: relay pool closed");

        // 3. Stop mDNS advertiser.
        if let Some(ref adv) = self._advertiser {
            adv.stop();
        }
        log::info!("omnibus: mDNS advertiser stopped");

        // 4. Stop mDNS browser.
        self.browser.stop();
        log::info!("omnibus: mDNS browser stopped");

        log::info!("omnibus: stopped");
    }

    /// Subscribe to Omnibus lifecycle events.
    ///
    /// Returns a broadcast receiver that emits `OmnibusEvent` variants.
    /// PeerConnected/PeerDisconnected/EventReceived/HealthChanged are not
    /// wired yet — they will be connected when the relay pool gets event hooks.
    pub fn subscribe_events(&self) -> broadcast::Receiver<OmnibusEvent> {
        self.events_tx.subscribe()
    }
}

impl Drop for Omnibus {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn omnibus_starts() {
        let config = OmnibusConfig {
            device_name: "Test Device".into(),
            bind_all: false,
            ..Default::default()
        };
        let omni = Omnibus::start(config).expect("omnibus should start");
        assert!(omni.port() > 0);
        assert!(!omni.status().has_identity);
    }

    #[test]
    fn create_identity() {
        let config = OmnibusConfig {
            device_name: "Test Device".into(),
            bind_all: false,
            ..Default::default()
        };
        let omni = Omnibus::start(config).expect("omnibus should start");
        let crown_id = omni
            .create_identity("Alice")
            .expect("should create identity");
        assert!(crown_id.starts_with("cpub1"));
        assert!(omni.status().has_identity);
        assert_eq!(omni.status().display_name, Some("Alice".into()));
    }

    #[test]
    fn post_text_note() {
        let config = OmnibusConfig {
            device_name: "Test Device".into(),
            bind_all: false,
            ..Default::default()
        };
        let omni = Omnibus::start(config).expect("omnibus should start");
        omni.create_identity("Bob")
            .expect("should create identity");

        let event = omni.post("Hello from Omnibus!").expect("should post");
        assert_eq!(event.content, "Hello from Omnibus!");
        assert_eq!(event.kind, 1);
    }

    #[test]
    fn status_reports_correctly() {
        let config = OmnibusConfig {
            device_name: "Status Test".into(),
            port: 0,
            bind_all: false,
            ..Default::default()
        };
        let omni = Omnibus::start(config).expect("omnibus should start");
        let status = omni.status();

        assert!(!status.has_identity);
        assert!(status.pubkey.is_none());
        assert!(status.relay_port > 0);
        assert!(status.relay_url.starts_with("ws://"));
        assert!(!status.has_home_node);
    }

    #[test]
    fn seed_and_query() {
        let config = OmnibusConfig {
            device_name: "Seed Test".into(),
            bind_all: false,
            ..Default::default()
        };
        let omni = Omnibus::start(config).expect("omnibus should start");
        omni.create_identity("Charlie")
            .expect("should create identity");

        omni.post("Seeded note").expect("should post");
        assert!(!omni.server.store().is_empty());

        // Event should be in the store.
        let stored = omni.server.store().query(&OmniFilter::default());
        assert!(stored.iter().any(|e| e.content == "Seeded note"));
    }

    #[test]
    fn pool_relays_not_hardcoded() {
        let config = OmnibusConfig {
            device_name: "Pool Test".into(),
            bind_all: false,
            ..Default::default()
        };
        let omni = Omnibus::start(config).expect("omnibus should start");
        // Omnibus::start connects pool to own server, so pool_relays >= 1.
        let status = omni.status();
        assert!(
            status.pool_relays >= 1,
            "pool_relays should be >= 1 (connected to own relay), got {}",
            status.pool_relays
        );
    }

    #[test]
    fn relay_health_returns_entries() {
        let config = OmnibusConfig {
            device_name: "Health Test".into(),
            bind_all: false,
            ..Default::default()
        };
        let omni = Omnibus::start(config).expect("omnibus should start");
        let health = omni.relay_health();
        // At least 1 relay (own server).
        assert!(!health.is_empty());
    }

    #[test]
    fn store_stats_after_posts() {
        let config = OmnibusConfig {
            device_name: "Stats Test".into(),
            bind_all: false,
            ..Default::default()
        };
        let omni = Omnibus::start(config).expect("omnibus should start");
        omni.create_identity("StatsUser")
            .expect("should create identity");

        // Ignore publish errors — pool relay may not be fully connected
        // in parallel tests. We're testing store_stats, not network publishing.
        // post() always seeds to local store before publishing, so stats
        // will reflect the events even if publish fails.
        let _ = omni.post("note 1");
        let _ = omni.post("note 2");

        let stats = omni.store_stats();
        // At least 2 text notes + 1 profile event.
        assert!(
            stats.event_count >= 3,
            "expected >= 3 events, got {}",
            stats.event_count
        );
        assert!(stats.oldest_event.is_some());
        assert!(stats.newest_event.is_some());
        assert!(!stats.events_by_kind.is_empty());
    }

    #[test]
    fn log_capture_push_and_retrieve() {
        let config = OmnibusConfig {
            device_name: "Log Test".into(),
            bind_all: false,
            log_capture_capacity: 50,
            ..Default::default()
        };
        let omni = Omnibus::start(config).expect("omnibus should start");

        omni.push_log(LogEntry {
            timestamp: Utc::now(),
            level: "INFO".into(),
            module: Some("test".into()),
            message: "hello from test".into(),
        });
        omni.push_log(LogEntry {
            timestamp: Utc::now(),
            level: "WARN".into(),
            module: None,
            message: "warning message".into(),
        });

        let recent = omni.recent_logs(10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].message, "hello from test");
        assert_eq!(recent[1].message, "warning message");
    }

    #[test]
    fn log_capture_arc_access() {
        let config = OmnibusConfig {
            device_name: "Arc Test".into(),
            bind_all: false,
            ..Default::default()
        };
        let omni = Omnibus::start(config).expect("omnibus should start");
        let cap = omni.log_capture();

        cap.lock().unwrap().push(LogEntry {
            timestamp: Utc::now(),
            level: "DEBUG".into(),
            module: None,
            message: "via arc".into(),
        });

        let recent = omni.recent_logs(10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].message, "via arc");
    }

    #[test]
    fn import_keyring_with_valid_export() {
        let config = OmnibusConfig {
            device_name: "Import Test".into(),
            bind_all: false,
            ..Default::default()
        };
        let omni = Omnibus::start(config).expect("omnibus should start");
        omni.create_identity("Exporter")
            .expect("should create identity");

        let exported = omni.export_keyring().expect("should export");

        // Create a fresh instance and import.
        let config2 = OmnibusConfig {
            device_name: "Import Target".into(),
            bind_all: false,
            ..Default::default()
        };
        let omni2 = Omnibus::start(config2).expect("omnibus should start");
        assert!(!omni2.status().has_identity);

        omni2
            .import_keyring(&exported)
            .expect("should import keyring");

        // After import, the keyring is unlocked.
        assert!(omni2.pubkey().is_some());
        assert_eq!(omni.pubkey(), omni2.pubkey());
    }

    #[test]
    fn import_keyring_with_invalid_data() {
        let config = OmnibusConfig {
            device_name: "Bad Import Test".into(),
            bind_all: false,
            ..Default::default()
        };
        let omni = Omnibus::start(config).expect("omnibus should start");

        let result = omni.import_keyring(b"this is not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn stop_is_idempotent() {
        let config = OmnibusConfig {
            device_name: "Stop Test".into(),
            bind_all: false,
            ..Default::default()
        };
        let omni = Omnibus::start(config).expect("omnibus should start");

        // Calling stop multiple times should not panic.
        omni.stop();
        omni.stop();
        omni.stop();
    }

    #[test]
    fn subscribe_events_receives_started() {
        let config = OmnibusConfig {
            device_name: "Event Test".into(),
            bind_all: false,
            ..Default::default()
        };
        // Subscribe BEFORE start won't work (channel created during start).
        // Instead, verify that subscribe_events returns a receiver and
        // that stop emits a Stopped event.
        let omni = Omnibus::start(config).expect("omnibus should start");
        let mut rx = omni.subscribe_events();

        omni.stop();

        // We should receive the Stopped event.
        let event = rx.try_recv().expect("should receive Stopped event");
        assert!(
            matches!(event, OmnibusEvent::Stopped),
            "expected Stopped, got {event:?}"
        );
    }

    #[test]
    fn drop_calls_stop() {
        let config = OmnibusConfig {
            device_name: "Drop Test".into(),
            bind_all: false,
            ..Default::default()
        };
        let omni = Omnibus::start(config).expect("omnibus should start");
        let mut rx = omni.subscribe_events();

        // Drop the omnibus instance.
        drop(omni);

        // The Stopped event should have been emitted during drop.
        let event = rx.try_recv().expect("should receive Stopped event from drop");
        assert!(
            matches!(event, OmnibusEvent::Stopped),
            "expected Stopped, got {event:?}"
        );
    }
}
