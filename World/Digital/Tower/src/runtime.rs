use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crown::CrownKeypair;
use globe::event_builder::UnsignedEvent;
use globe::filter::OmniFilter;
use globe::kind;
use globe::server::listener::{ConnectionFilter, SearchHandler, SearchHit};
use globe::server::network_defense::{ConnectionGuard, ConnectionPolicy, ConnectionVerdict, IpAllowlist, RateLimiter};
use magical_index::{KeywordIndex, SearchIndex, SearchQuery, SearchResponse};
use omnibus::{Omnibus, OmnibusConfig};
use serde::Serialize;

use crate::announcement::TowerAnnouncement;
use crate::config::{TowerConfig, TowerMode};
use crate::error::TowerError;
use crate::peering::PeeringLoop;

/// Current status of a Tower node.
#[derive(Clone, Debug, Serialize)]
pub struct TowerStatus {
    pub mode: TowerMode,
    pub name: String,
    pub relay_url: String,
    pub relay_port: u16,
    pub relay_connections: u32,
    pub has_identity: bool,
    pub pubkey: Option<String>,
    pub gospel_peers: usize,
    pub gospel_peer_urls: Vec<String>,
    pub uptime_secs: u64,
    pub event_count: usize,
    pub indexed_count: usize,
    pub communities: Vec<String>,
    /// Federated community IDs (accepted alongside own communities).
    pub federated_communities: Vec<String>,
    /// Active connection policy name.
    pub connection_policy: String,
    /// Number of IPs in the Tower allowlist.
    pub allowlist_size: usize,
    /// Total connections rejected since startup.
    pub connections_rejected: u64,
}

/// A Tower node — always-on network infrastructure.
///
/// Wraps Omnibus with Tower-specific behavior:
/// - Gospel peering loop with seed peers
/// - Lighthouse announcements (kind 7032)
/// - Content filtering (Pharos rejects non-gospel)
/// - Community serving (Harbor accepts member content)
/// - Full-text search via MagicalIndex
pub struct Tower {
    omnibus: Omnibus,
    peering: Mutex<PeeringLoop>,
    index: KeywordIndex,
    /// Timestamp of the most recently indexed event (for incremental indexing).
    last_indexed_at: Mutex<i64>,
    config: TowerConfig,
    started_at: Instant,
    /// Connection guard for Tower defense (IP allowlist + rate limiting).
    /// Shared with the ConnectionFilter closure on the relay server.
    connection_guard: Arc<Mutex<ConnectionGuard>>,
}

impl Tower {
    /// Start a Tower node.
    ///
    /// 1. Boots Omnibus (relay server, pool)
    /// 2. Creates or loads the Tower identity
    /// 3. Connects seed peers for gospel sync
    /// 4. Ready to run the main loop
    pub fn start(config: TowerConfig) -> Result<Self, TowerError> {
        // Ensure data directory exists.
        std::fs::create_dir_all(&config.data_dir).map_err(|e| {
            TowerError::ConfigError(format!("data dir: {e}"))
        })?;

        // Step 1: Load or generate storage key for relay database encryption.
        // The storage key is independent of identity — the Tower can run its
        // relay server without a user Crown. Identity is loaded later by the
        // daemon when the user creates or unlocks their Crown.
        let storage_key_path = config.data_dir.join("storage.key");
        let storage_key = if storage_key_path.exists() {
            let data = std::fs::read(&storage_key_path).map_err(|e| {
                TowerError::ConfigError(format!("read storage key: {e}"))
            })?;
            log::info!("tower: loaded storage key");
            data
        } else {
            // First run: generate a random 32-byte storage key.
            let key = sentinal::key_derivation::generate_salt(32)
                .map_err(|e| TowerError::ConfigError(format!("generate storage key: {e}")))?;
            std::fs::write(&storage_key_path, &key).map_err(|e| {
                TowerError::ConfigError(format!("save storage key: {e}"))
            })?;
            log::info!("tower: generated new storage key");
            key
        };

        // Load user identity if it exists (keyring in parent data dir).
        // The daemon saves the user's keyring to its own data_dir, which
        // is the parent of the Tower's data_dir.
        let has_user_identity = config.data_dir.parent()
            .map(|p| p.join("keyring.dat").exists())
            .unwrap_or(false);

        // Step 3: Create search index BEFORE Omnibus starts (in-memory SQLite, no deps on Omnibus).
        let index = KeywordIndex::in_memory().map_err(|e| {
            TowerError::ConfigError(format!("search index: {e}"))
        })?;

        // Build search handler closure capturing a clone of the index.
        let search_handler = Self::build_search_handler(index.clone());

        // Step 4: Build event filter and start Omnibus with the derived key.
        let event_filter = Self::build_event_filter(&config);

        // Step 5: Build connection defense.
        // Start with empty allowlist — populated after Omnibus boots and gospel loads.
        let rate_limiter = RateLimiter::new(
            config.rate_limit_config.clone().unwrap_or_default(),
        );
        let allowlist = IpAllowlist::new();
        let guard = ConnectionGuard::new(
            config.connection_policy,
            allowlist,
            rate_limiter,
        );
        let connection_guard = Arc::new(Mutex::new(guard));

        // Build the ConnectionFilter closure that wraps the guard.
        let guard_for_filter = connection_guard.clone();
        let connection_filter: ConnectionFilter = Arc::new(move |addr: std::net::SocketAddr| {
            let mut g = guard_for_filter.lock().unwrap_or_else(|e| e.into_inner());
            matches!(g.check(addr.ip()), ConnectionVerdict::Accept)
        });

        // Build the on_disconnect callback to release rate-limiter slots.
        let guard_for_release = connection_guard.clone();
        let on_disconnect: globe::server::listener::OnDisconnect =
            Arc::new(move |addr: std::net::SocketAddr| {
                let mut g = guard_for_release.lock().unwrap_or_else(|e| e.into_inner());
                g.release(addr.ip());
            });

        let server_config = globe::ServerConfig {
            max_connections: config.max_connections,
            event_filter: Some(event_filter),
            search_handler: Some(search_handler),
            data_dir: Some(config.data_dir.clone()),
            storage_key: Some(storage_key),
            connection_filter: Some(connection_filter),
            on_disconnect: Some(on_disconnect),
            require_auth: config.require_auth,
            ..Default::default()
        };

        let omnibus_config = OmnibusConfig {
            data_dir: Some(config.data_dir.clone()),
            device_name: config.name.clone(),
            port: config.port,
            bind_all: config.bind_all,
            home_node: None,
            server_config: Some(server_config),
            log_capture_capacity: 1000,
            privacy: omnibus::PrivacyConfig::default(),
            enable_upnp: config.enable_upnp,
        };

        let omnibus = Omnibus::start(omnibus_config)?;

        // Load user identity into Omnibus if it exists.
        if has_user_identity {
            if let Some(parent) = config.data_dir.parent() {
                match omnibus.load_identity(parent.to_str().unwrap_or("")) {
                    Ok(id) => log::info!("tower: loaded user identity {id}"),
                    Err(e) => log::warn!("tower: failed to load user identity: {e}"),
                }
            }
        } else {
            log::info!("tower: no user identity yet — relay running without Crown");
        }

        // Step 6: Populate the allowlist from existing gospel data.
        // This runs after Omnibus has loaded the event store.
        if !matches!(config.connection_policy, ConnectionPolicy::AllowAll) {
            let ips = Self::extract_tower_ips(&omnibus, &config);
            if !ips.is_empty() {
                connection_guard.lock().unwrap_or_else(|e| e.into_inner()).update_allowlist(ips);
            }
        }

        // Connect to seed peers.
        for seed in &config.seed_peers {
            if let Err(e) = omnibus.connect_relay(seed.as_str()) {
                log::warn!("tower: failed to connect seed peer {seed}: {e}");
            }
        }

        let peering = PeeringLoop::new(
            &config.seed_peers,
            Duration::from_secs(config.gospel_interval_secs),
            config.max_gospel_peers,
            config.effective_gospel_tiers(),
        );

        // Populate the pre-created search index from existing events.
        let existing = omnibus.query(&OmniFilter::default());
        let mut indexed = 0;
        let mut high_water: i64 = 0;
        for event in &existing {
            if index.index_event(event).is_ok() {
                indexed += 1;
            }
            if event.created_at > high_water {
                high_water = event.created_at;
            }
        }
        if indexed > 0 {
            log::info!("tower: indexed {indexed} existing events for search");
        }

        Ok(Self {
            omnibus,
            peering: Mutex::new(peering),
            index,
            last_indexed_at: Mutex::new(high_water),
            config,
            started_at: Instant::now(),
            connection_guard,
        })
    }

    /// Publish a lighthouse announcement to the network.
    pub fn announce(&self) -> Result<(), TowerError> {
        let relay_url = self
            .config
            .public_url
            .as_ref()
            .map(|u| u.to_string())
            .unwrap_or_else(|| self.omnibus.relay_url());

        let all_events = self
            .omnibus
            .query(&OmniFilter::default());
        let event_count = all_events.len() as u64;
        let gospel_count = all_events
            .iter()
            .filter(|e| globe::kind::is_gospel_registry(e.kind))
            .count() as u64;

        let ann = TowerAnnouncement {
            mode: self.config.mode,
            relay_url,
            name: self.config.name.clone(),
            gospel_count,
            event_count,
            uptime_secs: self.started_at.elapsed().as_secs(),
            version: env!("CARGO_PKG_VERSION").into(),
            communities: if self.config.mode == TowerMode::Harbor {
                self.config.communities.clone()
            } else {
                vec![]
            },
        };

        let keypair = self.extract_keypair()?;
        let event = ann.to_event(&keypair)?;

        // Store locally (always succeeds) and publish to network (best-effort).
        self.omnibus.seed_event(event.clone());
        if let Err(e) = self.omnibus.publish(event) {
            log::warn!("tower: announcement publish failed (will propagate via gospel): {e}");
        }

        log::info!(
            "tower: announced as {} at {}",
            self.config.mode,
            ann.relay_url
        );

        // Publish semantic profile alongside lighthouse announcement.
        if let Err(e) = self.publish_semantic_profile() {
            log::warn!("tower: semantic profile failed: {e}");
        }

        Ok(())
    }

    /// Check if an event should be accepted by this Tower.
    ///
    /// Pharos: only gospel registry events.
    /// Harbor: gospel + events from member communities.
    pub fn should_accept(&self, kind: u32, author: &str) -> bool {
        // Gospel is always accepted.
        if globe::kind::is_gospel_registry(kind) {
            return true;
        }

        // Standard metadata events (profile, contacts) are always useful.
        if kind == kind::PROFILE || kind == kind::CONTACT_LIST {
            return true;
        }

        match self.config.mode {
            TowerMode::Pharos | TowerMode::Intermediary => false,
            TowerMode::Harbor => {
                // Accept content from member communities or federated communities.
                // For now, accept if author is in either list.
                // In production, this would check Kingdom membership.
                self.config.communities.contains(&author.to_string())
                    || self.config.federated_communities.contains(&author.to_string())
                    || self.config.communities.is_empty() // open Harbor
            }
        }
    }

    /// Get the current status of this Tower node.
    pub fn status(&self) -> TowerStatus {
        let omnibus_status = self.omnibus.status();
        let peering = self.peering.lock().unwrap_or_else(|e| e.into_inner());
        let conn_stats = self.connection_stats();

        TowerStatus {
            mode: self.config.mode,
            name: self.config.name.clone(),
            relay_url: omnibus_status.relay_url.clone(),
            relay_port: omnibus_status.relay_port,
            relay_connections: omnibus_status.relay_connections,
            has_identity: omnibus_status.has_identity,
            pubkey: omnibus_status.pubkey.clone(),
            gospel_peers: peering.peer_count(),
            gospel_peer_urls: peering.peer_urls(),
            uptime_secs: self.started_at.elapsed().as_secs(),
            event_count: self
                .omnibus
                .query(&OmniFilter::default())
                .len(),
            indexed_count: self.indexed_count(),
            communities: self.config.communities.clone(),
            federated_communities: self.config.federated_communities.clone(),
            connection_policy: format!("{:?}", self.config.connection_policy),
            allowlist_size: conn_stats.allowlist_size,
            connections_rejected: conn_stats.total_rejected,
        }
    }

    /// Access the underlying Omnibus runtime.
    pub fn omnibus(&self) -> &Omnibus {
        &self.omnibus
    }

    /// The Tower's relay port.
    pub fn port(&self) -> u16 {
        self.omnibus.port()
    }

    /// The Tower's relay URL.
    pub fn relay_url(&self) -> String {
        self.omnibus.relay_url()
    }

    /// The Tower's public key, if identity is loaded.
    pub fn pubkey(&self) -> Option<String> {
        self.omnibus.pubkey()
    }

    /// Add a gospel peer dynamically (e.g., discovered from lighthouse announcements).
    pub fn add_gospel_peer(&self, url: url::Url) -> bool {
        let mut peering = self.peering.lock().unwrap_or_else(|e| e.into_inner());
        let added = peering.add_peer(url.clone());
        if added {
            // Also connect via Omnibus pool.
            if let Err(e) = self.omnibus.connect_relay(url.as_str()) {
                log::warn!("tower: failed to connect gospel peer {url}: {e}");
            }
        }
        added
    }

    // =================================================================
    // Connection defense
    // =================================================================

    /// Extract IP addresses from known Tower lighthouse announcements.
    ///
    /// Queries the event store for kind 7032 events, parses the relay URLs,
    /// and extracts IP addresses. Hostnames are resolved via std::net.
    /// Only includes Towers whose communities overlap with our own or our
    /// federation. Pharos nodes (no communities) and open-mode Towers are
    /// always included.
    fn extract_tower_ips(omnibus: &Omnibus, config: &TowerConfig) -> HashSet<IpAddr> {
        let filter = OmniFilter {
            kinds: Some(vec![globe::kind::LIGHTHOUSE_ANNOUNCE]),
            ..Default::default()
        };
        let events = omnibus.query(&filter);
        let mut ips = HashSet::new();

        for event in &events {
            if let Ok(announcement) = TowerAnnouncement::from_event(event) {
                // Only include IPs from Towers whose communities overlap with
                // our communities or our federation. Always include Towers with
                // no communities (Pharos nodes) and always include if we have
                // no communities (open mode).
                let should_include = announcement.communities.is_empty()
                    || config.communities.is_empty()
                    || announcement.communities.iter().any(|c| config.communities.contains(c))
                    || announcement.communities.iter().any(|c| config.federated_communities.contains(c));

                if !should_include {
                    continue;
                }

                if let Ok(url) = url::Url::parse(&announcement.relay_url) {
                    if let Some(host) = url.host_str() {
                        // Try parsing as IP directly first.
                        if let Ok(ip) = host.parse::<IpAddr>() {
                            ips.insert(ip);
                        } else {
                            // Try DNS resolution for hostnames.
                            if let Ok(addrs) = std::net::ToSocketAddrs::to_socket_addrs(
                                &(host, url.port().unwrap_or(7777)),
                            ) {
                                for addr in addrs {
                                    ips.insert(addr.ip());
                                }
                            }
                        }
                    }
                }
            }
        }

        log::info!(
            "tower: extracted {} Tower IPs from {} lighthouse announcements",
            ips.len(),
            events.len()
        );
        ips
    }

    /// Refresh the connection allowlist from current gospel data.
    ///
    /// Called periodically during the gospel cycle to pick up new Towers.
    pub fn refresh_allowlist(&self) {
        let ips = Self::extract_tower_ips(&self.omnibus, &self.config);
        let mut guard = self.connection_guard.lock().unwrap_or_else(|e| e.into_inner());
        guard.update_allowlist(ips);
        log::debug!(
            "tower: allowlist refreshed, {} IPs",
            guard.stats().allowlist_size
        );
    }

    /// Get current connection defense statistics.
    pub fn connection_stats(&self) -> globe::server::network_defense::ConnectionStats {
        self.connection_guard.lock().unwrap_or_else(|e| e.into_inner()).stats()
    }

    /// Process live gospel events and index new content.
    ///
    /// Non-blocking. Called on a fast timer (default 2s) to:
    /// 1. Drain gospel events from persistent subscriptions.
    /// 2. Index any new events in the store for search.
    ///
    /// This keeps both the gospel registry and search index fresh
    /// within seconds of new content arriving.
    pub fn process_live_events(&self) {
        // Drain live gospel subscriptions.
        if let Some(registry) = self.omnibus.gospel_registry() {
            let mut peering = self.peering.lock().unwrap_or_else(|e| e.into_inner());
            let received = peering.recv_live_all(registry);
            if received > 0 {
                log::info!("gospel live: {received} new events");
                self.omnibus.save_gospel();
            }
        }

        // Index new events for search.
        self.index_new_events();
    }

    // =================================================================
    // Search
    // =================================================================

    /// Search this Tower's indexed content.
    ///
    /// Returns keyword-matched results ranked by BM25 relevance.
    /// Without Advisor, this is keyword-only. With Advisor (future),
    /// semantic results would be merged in — same API either way.
    pub fn search(&self, query: &SearchQuery) -> Result<SearchResponse, TowerError> {
        self.index
            .search(query)
            .map_err(|e| TowerError::ConfigError(format!("search: {e}")))
    }

    /// Index events that arrived since the last indexing pass.
    ///
    /// Queries the event store for events newer than `last_indexed_at`,
    /// indexes each one, and advances the watermark. Called on the
    /// live sync ticker so new content becomes searchable within seconds.
    pub fn index_new_events(&self) {
        let since = { *self.last_indexed_at.lock().unwrap_or_else(|e| e.into_inner()) };

        let filter = OmniFilter {
            since: Some(since),
            ..Default::default()
        };
        let events = self.omnibus.query(&filter);
        if events.is_empty() {
            return;
        }

        let mut indexed = 0;
        let mut new_high_water = since;
        for event in &events {
            if self.index.index_event(event).is_ok() {
                indexed += 1;
            }
            if event.created_at > new_high_water {
                new_high_water = event.created_at;
            }
        }

        *self.last_indexed_at.lock().unwrap_or_else(|e| e.into_inner()) = new_high_water;

        if indexed > 0 {
            log::info!("tower: indexed {indexed} new events for search");
        }
    }

    /// Number of events currently in the search index.
    pub fn indexed_count(&self) -> usize {
        self.index.indexed_count().unwrap_or(0)
    }

    // =================================================================
    // Intermediary forwarding
    // =================================================================

    /// Apply privacy transforms to raw event data for intermediary forwarding.
    ///
    /// This function is the core of Intermediary mode: it takes raw event
    /// bytes, applies the configured transforms (timestamp jitter, metadata
    /// stripping, etc.), and returns transformed data ready for upstream
    /// relay forwarding.
    ///
    /// The intermediary path deliberately does NOT:
    /// - Store events locally (no EventStore writes).
    /// - Index events (no MagicalIndex writes).
    /// - Validate event signatures (that is the upstream relay's job).
    ///
    /// This keeps the intermediary as a pure privacy-preserving relay.
    pub fn handle_intermediary_event(
        event_data: &[u8],
        transforms: &crate::privacy_transforms::PrivacyTransforms,
    ) -> Vec<u8> {
        // Try to parse as JSON to apply structured transforms.
        // If parsing fails, return data as-is (opaque forwarding).
        let mut parsed: serde_json::Value = match serde_json::from_slice(event_data) {
            Ok(v) => v,
            Err(_) => return event_data.to_vec(),
        };

        // Apply timestamp jitter.
        if transforms.randomize_timestamps {
            if let Some(ts) = parsed.get("created_at").and_then(|v| v.as_i64()) {
                let jittered = crate::privacy_transforms::apply_timestamp_jitter(ts);
                parsed["created_at"] = serde_json::Value::Number(
                    serde_json::Number::from(jittered),
                );
            }
        }

        // Strip metadata tags.
        if transforms.strip_ip_metadata {
            if let Some(tags) = parsed.get("tags").and_then(|v| v.as_array()) {
                let tag_vecs: Vec<Vec<String>> = tags
                    .iter()
                    .filter_map(|tag| {
                        tag.as_array().map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                    })
                    .collect();
                let cleaned = crate::privacy_transforms::strip_metadata_tags(&tag_vecs);
                parsed["tags"] = serde_json::to_value(cleaned)
                    .unwrap_or(serde_json::Value::Array(vec![]));
            }
        }

        // Re-serialize.
        serde_json::to_vec(&parsed).unwrap_or_else(|_| event_data.to_vec())
    }

    // =================================================================
    // Gospel
    // =================================================================

    /// Run one gospel evangelization cycle (called from the main loop).
    ///
    /// Uses the server's persistent gospel registry (DB-backed, survives
    /// restarts) and persistent GospelPeer connections (preserving sync
    /// cursors across cycles). Opens live subscriptions on first run,
    /// then does full bilateral catch-up sync. After sync, discovers new
    /// peers from lighthouse announcements and saves the registry to disk.
    pub async fn run_gospel_cycle(&self) {
        // Use the server's persistent gospel registry (loaded from DB at startup).
        let registry = match self.omnibus.gospel_registry() {
            Some(r) => r,
            None => {
                log::warn!("tower: no gospel registry available");
                return;
            }
        };

        // Take peers out of the mutex for async work, then restore after.
        // Can't hold a std::sync::Mutex across await points.
        let mut peers = {
            let mut peering = self.peering.lock().unwrap_or_else(|e| e.into_inner());
            peering.take_peers()
        };

        if peers.is_empty() {
            // Restore empty vec.
            let mut peering = self.peering.lock().unwrap_or_else(|e| e.into_inner());
            peering.restore_peers(peers);
            return;
        }

        let mut total_received = 0;
        let mut total_sent = 0;

        for peer in &mut peers {
            // Open live subscription if not already open (first cycle).
            if !peer.has_live_subscription() {
                if let Err(e) = peer.open_live_subscription().await {
                    log::warn!(
                        "gospel: failed to open live subscription to {}: {e}",
                        peer.url()
                    );
                }
            }

            // Full bilateral sync (catch-up for anything the live sub missed).
            match peer.evangelize(registry).await {
                Ok((received, sent)) => {
                    total_received += received;
                    total_sent += sent;
                    if received > 0 || sent > 0 {
                        log::info!(
                            "gospel sync with {}: received={received}, sent={sent}",
                            peer.url()
                        );
                    }
                }
                Err(e) => {
                    log::warn!("gospel sync with {} failed: {e}", peer.url());
                }
            }
        }

        // Restore peers before lighthouse discovery (which may add more).
        {
            let mut peering = self.peering.lock().unwrap_or_else(|e| e.into_inner());
            peering.restore_peers(peers);
        }

        // Discover new Tower nodes from lighthouse announcements in the registry.
        self.discover_lighthouses(registry);

        if total_received > 0 || total_sent > 0 {
            log::info!(
                "gospel cycle complete: received={total_received}, sent={total_sent}"
            );
        }

        // Refresh connection allowlist from updated gospel.
        self.refresh_allowlist();

        // Persist gospel registry to encrypted database.
        self.omnibus.save_gospel();
    }

    /// Discover new gospel peers from lighthouse announcements.
    ///
    /// Checks the local relay store for LIGHTHOUSE_ANNOUNCE events (kind 7032),
    /// parses their relay URLs, and adds any we're not already peered with.
    fn discover_lighthouses(&self, _registry: &globe::GospelRegistry) {
        // Query all lighthouse events from the local relay store.
        let lighthouse_events = self.omnibus.query(&OmniFilter {
            kinds: Some(vec![kind::LIGHTHOUSE_ANNOUNCE]),
            ..Default::default()
        });

        let our_pubkey_hex = self.omnibus.pubkey_hex();
        let our_communities = &self.config.communities;
        let federated = &self.config.federated_communities;

        for event in &lighthouse_events {
            // Don't peer with ourselves.
            if let Some(ref pk) = our_pubkey_hex {
                if &event.author == pk {
                    continue;
                }
            }

            if let Ok(ann) = crate::announcement::TowerAnnouncement::from_event(event) {
                // Check if this Tower's communities overlap with ours or our federation.
                // Always peer with Towers that have no communities (Pharos nodes).
                // Always peer if WE have no communities (open mode).
                let should_peer = ann.communities.is_empty()
                    || our_communities.is_empty()
                    || ann.communities.iter().any(|c| our_communities.contains(c))
                    || ann.communities.iter().any(|c| federated.contains(c));

                if should_peer {
                    if let Ok(url) = ann.relay_url.parse::<url::Url>() {
                        if self.add_gospel_peer(url.clone()) {
                            log::info!(
                                "tower: discovered peer '{}' at {} via lighthouse (communities: {:?})",
                                ann.name,
                                ann.relay_url,
                                ann.communities,
                            );
                        }
                    }
                } else {
                    log::debug!(
                        "tower: skipping peer '{}' — no federation overlap",
                        ann.name,
                    );
                }
            }
        }
    }


    /// Build a search handler closure from the KeywordIndex.
    ///
    /// Maps search query text + OmniFilter constraints → Vec<SearchHit>.
    fn build_search_handler(index: KeywordIndex) -> SearchHandler {
        Arc::new(move |query_text: &str, filter: &OmniFilter| {
            let mut search_query = SearchQuery::new(query_text);

            // Map OmniFilter constraints to SearchQuery.
            if let Some(ref kinds) = filter.kinds {
                search_query = search_query.with_kinds(kinds.clone());
            }
            if let Some(ref authors) = filter.authors {
                search_query = search_query.with_authors(authors.clone());
            }
            if filter.since.is_some() || filter.until.is_some() {
                search_query = search_query.with_time_range(filter.since, filter.until);
            }
            if let Some(limit) = filter.limit {
                search_query = search_query.with_limit(limit);
            }

            match index.search(&search_query) {
                Ok(response) => response
                    .results
                    .into_iter()
                    .map(|r| SearchHit {
                        event_id: r.event_id,
                        relevance: r.relevance,
                        snippet: r.snippet,
                        suggestions: r.suggestions,
                    })
                    .collect(),
                Err(e) => {
                    log::warn!("search handler error: {e}");
                    Vec::new()
                }
            }
        })
    }

    /// Publish a semantic profile event (kind 26000) advertising search capabilities.
    ///
    /// The profile tells the network what this Tower can do:
    /// - `keyword_search`: FTS5-backed keyword search (always true)
    /// - `semantic_search`: AI-powered semantic search (false until Advisor is wired)
    /// - `suggestions`: concept suggestions (false for now)
    fn publish_semantic_profile(&self) -> Result<(), TowerError> {
        let keypair = self.extract_keypair()?;

        let capabilities = serde_json::json!({
            "keyword_search": true,
            "semantic_search": false,
            "suggestions": false,
            "indexed_count": self.indexed_count(),
        });

        let pubkey_hex = keypair.public_key_hex();
        let unsigned = UnsignedEvent::new(kind::SEMANTIC_PROFILE, capabilities.to_string())
            .with_d_tag(&pubkey_hex);

        let event = globe::EventBuilder::sign(&unsigned, &keypair)
            .map_err(|e| TowerError::AnnounceFailed(format!("sign semantic profile: {e}")))?;

        self.omnibus.seed_event(event.clone());
        if let Err(e) = self.omnibus.publish(event) {
            log::warn!("tower: semantic profile publish failed (will propagate via gospel): {e}");
        }

        log::info!("tower: published semantic profile (keyword_search=true)");
        Ok(())
    }

    /// Build an event filter closure that enforces Tower content policy.
    fn build_event_filter(config: &TowerConfig) -> Arc<dyn Fn(&globe::OmniEvent) -> bool + Send + Sync> {
        let mode = config.mode;
        let communities = config.communities.clone();
        let federated = config.federated_communities.clone();

        Arc::new(move |event| {
            // Gospel is always accepted.
            if globe::kind::is_gospel_registry(event.kind) {
                return true;
            }

            // Standard metadata events (profile, contacts) are always useful.
            if event.kind == kind::PROFILE || event.kind == kind::CONTACT_LIST {
                return true;
            }

            match mode {
                TowerMode::Pharos | TowerMode::Intermediary => false,
                TowerMode::Harbor => {
                    communities.contains(&event.author)
                        || federated.contains(&event.author)
                        || communities.is_empty() // open Harbor
                }
            }
        })
    }

    /// Extract keypair for signing (delegates to Omnibus internals).
    fn extract_keypair(&self) -> Result<CrownKeypair, TowerError> {
        let export = self
            .omnibus
            .export_keyring()
            .map_err(|e| TowerError::IdentityFailed(format!("export: {e}")))?;

        let storage: serde_json::Value = serde_json::from_slice(&export)
            .map_err(|e| TowerError::IdentityFailed(format!("parse: {e}")))?;

        let hex_key = storage
            .get("primary_private_key")
            .and_then(|v| v.as_str())
            .ok_or(TowerError::IdentityFailed("no primary key".into()))?;

        let key_bytes: Vec<u8> = (0..hex_key.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex_key[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| TowerError::IdentityFailed(format!("hex: {e}")))?;

        CrownKeypair::from_private_key(&key_bytes)
            .map_err(|e| TowerError::IdentityFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU64, Ordering};
    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Create a test config with an isolated directory structure:
    /// `/tmp/tower_test_{name}_{pid}_{id}/tower/` as data_dir.
    /// Identity files go in the parent (the unique test root).
    fn temp_config(name: &str) -> TowerConfig {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let test_root = std::env::temp_dir().join(format!(
            "tower_test_{}_{}_{}", name, std::process::id(), id
        ));
        TowerConfig {
            mode: TowerMode::Pharos,
            name: name.into(),
            data_dir: test_root.join("tower"),
            port: 0, // OS-assigned
            bind_all: false,
            ..Default::default()
        }
    }

    /// Create a test identity (keyring.dat + soul/) in the parent of
    /// `config.data_dir` so that Tower::start() can load it.
    fn seed_identity(config: &TowerConfig) {
        let parent = config.data_dir.parent().expect("data_dir has parent");
        std::fs::create_dir_all(parent).expect("create parent dir");

        // Generate keypair and save keyring.
        let mut keyring = crown::Keyring::new();
        keyring.generate_primary().expect("generate keypair");
        let data = keyring.export().expect("export keyring");
        std::fs::write(parent.join("keyring.dat"), &data).expect("write keyring");

        // Create a minimal Soul directory.
        let soul_dir = parent.join("soul");
        crown::Soul::create(&soul_dir, None).expect("create soul");
    }

    fn cleanup(config: &TowerConfig) {
        // Remove the entire test root (parent of data_dir).
        if let Some(parent) = config.data_dir.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }

    #[test]
    fn tower_starts_pharos() {
        let config = temp_config("pharos_start");
        seed_identity(&config);
        let tower = Tower::start(config.clone()).expect("tower should start");
        assert_eq!(tower.status().mode, TowerMode::Pharos);
        assert!(tower.status().has_identity);
        assert!(tower.port() > 0);
        cleanup(&config);
    }

    #[test]
    fn tower_starts_harbor() {
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            communities: vec!["community_a".into()],
            ..temp_config("harbor_start")
        };
        let tower = Tower::start(config.clone()).expect("tower should start");
        assert_eq!(tower.status().mode, TowerMode::Harbor);
        assert_eq!(tower.status().communities, vec!["community_a"]);
        cleanup(&config);
    }

    #[test]
    fn tower_starts_without_identity() {
        let config = temp_config("no_identity");
        let tower = Tower::start(config.clone()).expect("tower should start without identity");
        assert!(!tower.status().has_identity);
        assert!(tower.pubkey().is_none());
        assert!(tower.port() > 0);
        cleanup(&config);
    }

    #[test]
    fn tower_loads_identity() {
        let config = temp_config("identity");
        seed_identity(&config);
        let tower = Tower::start(config.clone()).expect("tower should start");
        assert!(tower.pubkey().is_some());
        assert!(tower.pubkey().unwrap().starts_with("cpub1"));
        cleanup(&config);
    }

    #[test]
    fn tower_announces() {
        let config = temp_config("announce");
        seed_identity(&config);
        let tower = Tower::start(config.clone()).expect("tower should start");
        tower.announce().expect("should announce");

        // The announcement event should be in the local store.
        let events = tower.omnibus().query(&OmniFilter {
            kinds: Some(vec![kind::LIGHTHOUSE_ANNOUNCE]),
            ..Default::default()
        });
        assert_eq!(events.len(), 1);

        let ann = TowerAnnouncement::from_event(&events[0]).unwrap();
        assert_eq!(ann.mode, TowerMode::Pharos);
        assert_eq!(ann.name, "announce");
        cleanup(&config);
    }

    #[test]
    fn tower_announces_semantic_profile() {
        let config = temp_config("semantic_profile");
        seed_identity(&config);
        let tower = Tower::start(config.clone()).expect("tower should start");
        tower.announce().expect("should announce");

        // Semantic profile (kind 26000) should be in the local store.
        let profiles = tower.omnibus().query(&OmniFilter {
            kinds: Some(vec![kind::SEMANTIC_PROFILE]),
            ..Default::default()
        });
        assert_eq!(profiles.len(), 1);

        // Parse capabilities from the profile.
        let content: serde_json::Value =
            serde_json::from_str(&profiles[0].content).expect("valid JSON");
        assert_eq!(content["keyword_search"], true);
        assert_eq!(content["semantic_search"], false);
        cleanup(&config);
    }

    #[test]
    fn tower_search_handler_returns_results() {
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            ..temp_config("search_handler")
        };
        let tower = Tower::start(config.clone()).expect("tower should start");

        // Seed searchable content.
        tower.omnibus().seed_event(globe::event::OmniEvent {
            id: "sh-1".into(),
            author: "a".repeat(64),
            created_at: 1000,
            kind: 1,
            tags: vec![],
            content: "woodworking with dovetail joints".into(),
            sig: "c".repeat(128),
        });
        tower.index_new_events();

        // The search handler is wired into the relay — test it directly.
        let response = tower.search(&SearchQuery::new("woodworking")).unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].event_id, "sh-1");
        assert!(response.results[0].relevance > 0.0);
        cleanup(&config);
    }

    #[test]
    fn pharos_rejects_content() {
        let config = temp_config("pharos_filter");
        let tower = Tower::start(config.clone()).expect("tower should start");

        // Gospel is accepted.
        assert!(tower.should_accept(kind::NAME_CLAIM, "anyone"));
        assert!(tower.should_accept(kind::RELAY_HINT, "anyone"));
        assert!(tower.should_accept(kind::BEACON, "anyone"));
        assert!(tower.should_accept(kind::LIGHTHOUSE_ANNOUNCE, "anyone"));

        // Profile/contacts accepted.
        assert!(tower.should_accept(kind::PROFILE, "anyone"));
        assert!(tower.should_accept(kind::CONTACT_LIST, "anyone"));

        // Content rejected.
        assert!(!tower.should_accept(kind::TEXT_NOTE, "anyone"));
        assert!(!tower.should_accept(1000, "anyone")); // Advisor range
        cleanup(&config);
    }

    #[test]
    fn harbor_accepts_community_content() {
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            communities: vec!["member_pubkey".into()],
            ..temp_config("harbor_filter")
        };
        let tower = Tower::start(config.clone()).expect("tower should start");

        // Gospel always accepted.
        assert!(tower.should_accept(kind::NAME_CLAIM, "anyone"));

        // Community member content accepted.
        assert!(tower.should_accept(kind::TEXT_NOTE, "member_pubkey"));

        // Non-member content rejected.
        assert!(!tower.should_accept(kind::TEXT_NOTE, "outsider"));
        cleanup(&config);
    }

    #[test]
    fn open_harbor_accepts_all() {
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            communities: vec![], // empty = open
            ..temp_config("open_harbor")
        };
        let tower = Tower::start(config.clone()).expect("tower should start");

        // Open harbor accepts everything.
        assert!(tower.should_accept(kind::TEXT_NOTE, "anyone"));
        assert!(tower.should_accept(1000, "anyone"));
        cleanup(&config);
    }

    #[test]
    fn tower_status() {
        let config = temp_config("status");
        seed_identity(&config);
        let tower = Tower::start(config.clone()).expect("tower should start");
        let status = tower.status();

        assert_eq!(status.mode, TowerMode::Pharos);
        assert_eq!(status.name, "status");
        assert!(status.has_identity);
        assert!(status.relay_port > 0);
        assert!(status.relay_url.starts_with("ws://"));
        assert_eq!(status.gospel_peers, 0);
        cleanup(&config);
    }

    #[test]
    fn tower_encrypts_storage() {
        let config = temp_config("encrypted_storage");
        let _tower = Tower::start(config.clone()).expect("tower should start");

        // relay.db should exist (SQLCipher encrypted).
        let db_path = config.data_dir.join("relay.db");
        assert!(db_path.exists());

        // The database should NOT be readable as plain SQLite.
        // SQLCipher databases start with random bytes, not "SQLite format 3".
        let header = std::fs::read(&db_path).unwrap();
        if header.len() >= 16 {
            let sqlite_magic = b"SQLite format 3\0";
            assert_ne!(&header[..16], sqlite_magic, "database should be encrypted");
        }

        cleanup(&config);
    }

    #[test]
    fn tower_storage_key_deterministic_from_identity() {
        // Use Harbor mode so we can post content (Pharos rejects text notes).
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            ..temp_config("deterministic_key")
        };
        seed_identity(&config);

        // Same identity → same storage key → same database can be opened.
        // First start creates identity + encrypted database.
        {
            let tower = Tower::start(config.clone()).expect("first start");
            tower.announce().expect("should announce");
        }

        // Second start re-derives the same key from the same identity.
        // If the key were wrong, Omnibus would fail to open the database.
        {
            let tower = Tower::start(config.clone()).expect("second start");
            // Query should find the announcement from the first run.
            let events = tower.omnibus().query(&OmniFilter {
                kinds: Some(vec![kind::LIGHTHOUSE_ANNOUNCE]),
                ..Default::default()
            });
            assert!(!events.is_empty(), "should find announcement from first run");
        }

        cleanup(&config);
    }

    #[test]
    fn tower_has_gospel_registry() {
        let config = temp_config("gospel_registry");
        let tower = Tower::start(config.clone()).expect("tower should start");

        // Tower should have a gospel registry from the relay server.
        assert!(tower.omnibus().gospel_registry().is_some());

        let registry = tower.omnibus().gospel_registry().unwrap();
        assert_eq!(registry.total_count(), 0);
        cleanup(&config);
    }

    #[test]
    fn tower_gospel_persists_across_restarts() {
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            ..temp_config("gospel_persist")
        };
        seed_identity(&config);

        // First start: announce (creates a lighthouse event in the store).
        {
            let tower = Tower::start(config.clone()).expect("first start");
            tower.announce().expect("should announce");

            // Insert a gospel event into the registry and save.
            let registry = tower.omnibus().gospel_registry().unwrap();
            let name_event = globe::event::OmniEvent {
                id: "a".repeat(64),
                author: "b".repeat(64),
                created_at: 1000,
                kind: globe::kind::NAME_CLAIM,
                tags: vec![vec!["d".into(), "test.idea".into()]],
                content: String::new(),
                sig: "c".repeat(128),
            };
            registry.insert(&name_event);
            tower.omnibus().save_gospel();
        }

        // Second start: gospel registry should have the name from the first run.
        {
            let tower = Tower::start(config.clone()).expect("second start");
            let registry = tower.omnibus().gospel_registry().unwrap();
            assert!(
                registry.lookup_name("test.idea").is_some(),
                "gospel should persist across restarts"
            );
        }

        cleanup(&config);
    }

    #[test]
    fn tower_discovers_lighthouses() {
        let config = temp_config("lighthouse_discovery");
        let tower = Tower::start(config.clone()).expect("tower should start");

        // Simulate receiving a lighthouse announcement from another Tower.
        let other_ann = TowerAnnouncement {
            mode: TowerMode::Pharos,
            relay_url: "wss://other-tower.example.com".into(),
            name: "Other Tower".into(),
            gospel_count: 0,
            event_count: 0,
            uptime_secs: 100,
            version: "0.1.0".into(),
            communities: vec![],
        };

        // Create a fake keypair to sign the announcement.
        let other_keypair = crown::CrownKeypair::generate();
        let ann_event = other_ann.to_event(&other_keypair).expect("should create event");

        // Seed the announcement into the local store.
        tower.omnibus().seed_event(ann_event);

        // Enter Omnibus runtime so add_gospel_peer can spawn relay connections.
        let _guard = tower.omnibus().runtime().enter();

        // Run lighthouse discovery.
        let registry = tower.omnibus().gospel_registry().unwrap();
        tower.discover_lighthouses(registry);

        // Should have added the peer.
        let status = tower.status();
        assert_eq!(status.gospel_peers, 1);
        assert!(status.gospel_peer_urls.iter().any(|u| u.contains("other-tower.example.com")));

        cleanup(&config);
    }

    #[test]
    fn tower_reloads_identity() {
        let config = temp_config("reload");
        seed_identity(&config);

        // First start loads identity.
        let pubkey1 = {
            let tower = Tower::start(config.clone()).expect("first start");
            tower.pubkey().unwrap()
        };

        // Second start loads same identity.
        let pubkey2 = {
            let tower = Tower::start(config.clone()).expect("second start");
            tower.pubkey().unwrap()
        };

        assert_eq!(pubkey1, pubkey2);
        cleanup(&config);
    }

    // =================================================================
    // Search tests
    // =================================================================

    #[test]
    fn tower_indexes_seeded_events() {
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            ..temp_config("index_seed")
        };
        let tower = Tower::start(config.clone()).expect("tower should start");

        // Seed some content.
        tower.omnibus().seed_event(globe::event::OmniEvent {
            id: "search-1".into(),
            author: "a".repeat(64),
            created_at: 1000,
            kind: 1,
            tags: vec![],
            content: "woodworking with dovetail joints".into(),
            sig: "c".repeat(128),
        });
        tower.omnibus().seed_event(globe::event::OmniEvent {
            id: "search-2".into(),
            author: "a".repeat(64),
            created_at: 2000,
            kind: 1,
            tags: vec![],
            content: "cooking pasta recipes".into(),
            sig: "c".repeat(128),
        });

        // Index the new events.
        tower.index_new_events();
        assert_eq!(tower.indexed_count(), 2);

        cleanup(&config);
    }

    #[test]
    fn tower_search_finds_content() {
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            ..temp_config("search_find")
        };
        let tower = Tower::start(config.clone()).expect("tower should start");

        tower.omnibus().seed_event(globe::event::OmniEvent {
            id: "s-1".into(),
            author: "a".repeat(64),
            created_at: 1000,
            kind: 1,
            tags: vec![],
            content: "woodworking with dovetail joints".into(),
            sig: "c".repeat(128),
        });
        tower.omnibus().seed_event(globe::event::OmniEvent {
            id: "s-2".into(),
            author: "a".repeat(64),
            created_at: 2000,
            kind: 1,
            tags: vec![],
            content: "cooking pasta recipes".into(),
            sig: "c".repeat(128),
        });

        tower.index_new_events();

        let response = tower.search(&SearchQuery::new("woodworking")).unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].event_id, "s-1");
        assert!(response.results[0].relevance > 0.0);

        cleanup(&config);
    }

    #[test]
    fn tower_search_no_results() {
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            ..temp_config("search_empty")
        };
        let tower = Tower::start(config.clone()).expect("tower should start");

        tower.omnibus().seed_event(globe::event::OmniEvent {
            id: "s-1".into(),
            author: "a".repeat(64),
            created_at: 1000,
            kind: 1,
            tags: vec![],
            content: "hello world".into(),
            sig: "c".repeat(128),
        });
        tower.index_new_events();

        let response = tower.search(&SearchQuery::new("woodworking")).unwrap();
        assert!(response.results.is_empty());

        cleanup(&config);
    }

    #[test]
    fn tower_incremental_indexing() {
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            ..temp_config("incremental")
        };
        let tower = Tower::start(config.clone()).expect("tower should start");

        // First batch.
        tower.omnibus().seed_event(globe::event::OmniEvent {
            id: "inc-1".into(),
            author: "a".repeat(64),
            created_at: 1000,
            kind: 1,
            tags: vec![],
            content: "first post".into(),
            sig: "c".repeat(128),
        });
        tower.index_new_events();
        assert_eq!(tower.indexed_count(), 1);

        // Second batch — only new events get indexed.
        tower.omnibus().seed_event(globe::event::OmniEvent {
            id: "inc-2".into(),
            author: "a".repeat(64),
            created_at: 2000,
            kind: 1,
            tags: vec![],
            content: "second post".into(),
            sig: "c".repeat(128),
        });
        tower.index_new_events();
        assert_eq!(tower.indexed_count(), 2);

        cleanup(&config);
    }

    #[test]
    fn tower_status_includes_indexed_count() {
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            ..temp_config("status_indexed")
        };
        let tower = Tower::start(config.clone()).expect("tower should start");

        let status = tower.status();
        assert_eq!(status.indexed_count, 0);

        tower.omnibus().seed_event(globe::event::OmniEvent {
            id: "si-1".into(),
            author: "a".repeat(64),
            created_at: 1000,
            kind: 1,
            tags: vec![],
            content: "searchable content".into(),
            sig: "c".repeat(128),
        });
        tower.index_new_events();

        let status = tower.status();
        assert_eq!(status.indexed_count, 1);

        cleanup(&config);
    }

    #[test]
    fn tower_indexes_on_startup() {
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            ..temp_config("index_startup")
        };

        // First start: seed an event and persist.
        {
            let tower = Tower::start(config.clone()).expect("first start");
            tower.omnibus().seed_event(globe::event::OmniEvent {
                id: "persist-1".into(),
                author: "a".repeat(64),
                created_at: 1000,
                kind: 1,
                tags: vec![],
                content: "persistent content for search".into(),
                sig: "c".repeat(128),
            });
        }

        // Second start: the event should be indexed from the store on boot.
        {
            let tower = Tower::start(config.clone()).expect("second start");
            assert!(tower.indexed_count() > 0, "events should be indexed on startup");

            let response = tower.search(&SearchQuery::new("persistent")).unwrap();
            assert_eq!(response.results.len(), 1);
        }

        cleanup(&config);
    }

    // =================================================================
    // Intermediary forwarding tests
    // =================================================================

    #[test]
    fn intermediary_handler_applies_timestamp_jitter() {
        let transforms = crate::privacy_transforms::PrivacyTransforms {
            randomize_timestamps: true,
            strip_ip_metadata: false,
            inject_decoy_events: false,
            decoy_rate: 0.0,
        };
        let event = serde_json::json!({
            "id": "abc",
            "author": "def",
            "created_at": 1_700_000_000i64,
            "kind": 1,
            "tags": [],
            "content": "hello",
            "sig": "ghi"
        });
        let data = serde_json::to_vec(&event).unwrap();
        let result = Tower::handle_intermediary_event(&data, &transforms);
        let parsed: serde_json::Value = serde_json::from_slice(&result).unwrap();
        let ts = parsed["created_at"].as_i64().unwrap();
        let diff = (ts - 1_700_000_000i64).unsigned_abs();
        assert!(
            (5..=60).contains(&diff),
            "jittered timestamp diff {diff} not in [5, 60]",
        );
    }

    #[test]
    fn intermediary_handler_strips_metadata() {
        let transforms = crate::privacy_transforms::PrivacyTransforms {
            randomize_timestamps: false,
            strip_ip_metadata: true,
            inject_decoy_events: false,
            decoy_rate: 0.0,
        };
        let event = serde_json::json!({
            "id": "abc",
            "author": "def",
            "created_at": 1000,
            "kind": 1,
            "tags": [
                ["p", "pubkey123"],
                ["ip_address", "1.2.3.4"],
                ["geo_lat", "51.5"],
                ["e", "event123"]
            ],
            "content": "hello",
            "sig": "ghi"
        });
        let data = serde_json::to_vec(&event).unwrap();
        let result = Tower::handle_intermediary_event(&data, &transforms);
        let parsed: serde_json::Value = serde_json::from_slice(&result).unwrap();
        let tags = parsed["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 2, "should strip ip and geo tags");
        assert_eq!(tags[0][0], "p");
        assert_eq!(tags[1][0], "e");
    }

    #[test]
    fn intermediary_handler_no_transforms() {
        let transforms = crate::privacy_transforms::PrivacyTransforms::default();
        let event = serde_json::json!({
            "id": "abc",
            "created_at": 1000,
            "tags": [["ip", "1.2.3.4"]],
            "content": "hello"
        });
        let data = serde_json::to_vec(&event).unwrap();
        let result = Tower::handle_intermediary_event(&data, &transforms);
        let parsed: serde_json::Value = serde_json::from_slice(&result).unwrap();
        // No transforms applied — timestamp and tags unchanged.
        assert_eq!(parsed["created_at"], 1000);
        assert_eq!(parsed["tags"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn intermediary_handler_invalid_json_passthrough() {
        let transforms = crate::privacy_transforms::PrivacyTransforms {
            randomize_timestamps: true,
            strip_ip_metadata: true,
            ..Default::default()
        };
        let garbage = b"not valid json at all";
        let result = Tower::handle_intermediary_event(garbage, &transforms);
        assert_eq!(result, garbage, "invalid JSON should pass through unchanged");
    }
}
