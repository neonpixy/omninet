use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::accept_async;

use crate::error::GlobeError;
use crate::event::OmniEvent;

use crate::gospel::{GospelConfig, GospelRegistry};

use super::asset_fetch::FetchCoalescer;
use super::asset_http;
use super::asset_store::{AssetStore, AssetStoreConfig};
use super::database::RelayDatabase;
use super::session::handle_session;
use super::storage::{EventStore, StoreConfig};

/// A function that decides whether an incoming event should be accepted.
///
/// Return `true` to store/broadcast the event, `false` to reject it.
/// Used by Tower to enforce content policy (Pharos: gospel only,
/// Harbor: gospel + community content).
pub type EventFilter = Arc<dyn Fn(&OmniEvent) -> bool + Send + Sync>;

/// A single search hit from the search handler.
#[derive(Clone, Debug)]
pub struct SearchHit {
    /// The matching event's ID.
    pub event_id: String,
    /// Relevance score (0.0 = irrelevant, 1.0 = perfect match).
    pub relevance: f64,
    /// Text snippet with matched terms highlighted (if available).
    pub snippet: Option<String>,
    /// Concept suggestions based on the match.
    pub suggestions: Vec<String>,
}

/// A function that handles search queries from clients.
///
/// Takes a search query string and an OmniFilter for additional constraints.
/// Returns a list of SearchHit results ranked by relevance.
/// Used by Tower to wire MagicalIndex into the relay's session handler.
pub type SearchHandler = Arc<dyn Fn(&str, &crate::filter::OmniFilter) -> Vec<SearchHit> + Send + Sync>;

/// A function that decides whether an incoming connection should be accepted.
///
/// Called with the peer's socket address before the WebSocket handshake.
/// Return `true` to accept the connection, `false` to reject it.
/// Used by Tower to enforce IP allowlists (Gospel-fed Tower registry).
pub type ConnectionFilter = Arc<dyn Fn(SocketAddr) -> bool + Send + Sync>;

/// Called when a connection closes, with the peer's socket address.
///
/// Used by Tower to release rate-limiter slots so the per-IP counter
/// stays in sync with actual active connections.
pub type OnDisconnect = Arc<dyn Fn(SocketAddr) + Send + Sync>;

// ---------------------------------------------------------------------------
// Proxy WebSocket compatibility
// ---------------------------------------------------------------------------

/// Check if peeked HTTP data is a WebSocket upgrade that has been
/// mangled by a reverse proxy (e.g. cloudflared).
///
/// Cloudflare Tunnel strips both `Upgrade: websocket` and
/// `Connection: Upgrade` (hop-by-hop headers per HTTP spec), but
/// preserves the WebSocket-specific headers (`Sec-WebSocket-Key`,
/// `Sec-WebSocket-Version`). It also rewrites `Connection` to
/// `keep-alive`.
///
/// Returns `true` when WebSocket headers are present but the
/// standard upgrade handshake headers are missing or wrong.
fn needs_proxy_ws_fixup(peeked: &[u8]) -> bool {
    let text = match std::str::from_utf8(peeked) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let lower = text.to_ascii_lowercase();
    // Has WebSocket-specific headers (survives proxy stripping).
    let has_ws_key = lower.contains("sec-websocket-key:");
    if !has_ws_key {
        return false;
    }
    // Missing either `Upgrade: websocket` or `Connection:` containing `upgrade`.
    let has_upgrade = lower.contains("upgrade: websocket");
    let has_connection_upgrade = lower.lines().any(|line| {
        line.starts_with("connection:") && line.contains("upgrade")
    });
    !has_upgrade || !has_connection_upgrade
}

/// Wraps a `TcpStream` with a prefix buffer. Reads drain the prefix
/// first, then fall through to the inner stream. Writes pass through.
///
/// Used to replay modified HTTP headers (with an injected
/// `Connection: Upgrade` line) before the WebSocket handshake.
struct PrefixedStream {
    prefix: Vec<u8>,
    offset: usize,
    inner: TcpStream,
}

impl AsyncRead for PrefixedStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        if this.offset < this.prefix.len() {
            let remaining = &this.prefix[this.offset..];
            let n = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..n]);
            this.offset += n;
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut this.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for PrefixedStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

impl Unpin for PrefixedStream {}

/// Configuration for the relay server.
///
/// All fields have sensible defaults via `Default::default()`. For testing,
/// leave `data_dir` as `None` to run fully in-memory.
pub struct ServerConfig {
    /// Maximum simultaneous connections. `None` means no limit.
    pub max_connections: Option<usize>,
    /// Event store configuration.
    pub store_config: StoreConfig,
    /// Broadcast channel buffer size for live events.
    pub broadcast_buffer: usize,
    /// Asset store configuration.
    pub asset_store_config: AssetStoreConfig,
    /// Whether this relay accepts and serves binary assets.
    pub enable_assets: bool,
    /// Peer relay URLs for pull-through asset caching.
    /// On GET cache miss, these peers are tried in order.
    pub asset_peer_urls: Vec<url::Url>,
    /// Directory for persistent storage. If `None`, everything stays in-memory.
    ///
    /// When set, the relay creates `relay.db` (SQLCipher encrypted) inside
    /// this directory. Events, assets, and gospel snapshots all live in
    /// one encrypted database file.
    ///
    /// The platform layer provides this path (e.g., `~/Library/Application Support/Omnidea/`
    /// on macOS). Globe is path-agnostic.
    pub data_dir: Option<PathBuf>,
    /// 32-byte SQLCipher encryption key for the relay database.
    ///
    /// Derived from Crown via Sentinal HKDF by the platform layer.
    /// Required when `data_dir` is set. Ignored when `data_dir` is `None`.
    pub storage_key: Option<Vec<u8>>,
    /// Optional event filter. When set, only events passing this check
    /// are stored and broadcast. `None` means accept all events.
    pub event_filter: Option<EventFilter>,
    /// Optional search handler. When set, REQ filters with a `search`
    /// field are delegated to this handler for full-text/semantic search.
    pub search_handler: Option<SearchHandler>,
    /// Optional connection filter. When set, incoming connections are
    /// checked before the WebSocket handshake. `None` means accept all.
    pub connection_filter: Option<ConnectionFilter>,
    /// Whether clients must authenticate (AUTH kind 22242) before
    /// sending EVENT or REQ messages. Default `false`.
    pub require_auth: bool,
    /// Called when a WebSocket session closes. Used by Tower to release
    /// rate-limiter slots so per-IP counters stay in sync.
    pub on_disconnect: Option<OnDisconnect>,
}

impl Clone for ServerConfig {
    fn clone(&self) -> Self {
        Self {
            max_connections: self.max_connections,
            store_config: self.store_config.clone(),
            broadcast_buffer: self.broadcast_buffer,
            asset_store_config: self.asset_store_config.clone(),
            enable_assets: self.enable_assets,
            asset_peer_urls: self.asset_peer_urls.clone(),
            data_dir: self.data_dir.clone(),
            storage_key: self.storage_key.clone(),
            event_filter: self.event_filter.clone(),
            search_handler: self.search_handler.clone(),
            connection_filter: self.connection_filter.clone(),
            require_auth: self.require_auth,
            on_disconnect: self.on_disconnect.clone(),
        }
    }
}

impl std::fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerConfig")
            .field("max_connections", &self.max_connections)
            .field("store_config", &self.store_config)
            .field("broadcast_buffer", &self.broadcast_buffer)
            .field("asset_store_config", &self.asset_store_config)
            .field("enable_assets", &self.enable_assets)
            .field("asset_peer_urls", &self.asset_peer_urls)
            .field("data_dir", &self.data_dir)
            .field("storage_key", &self.storage_key.as_ref().map(|_| "[redacted]"))
            .field("event_filter", &self.event_filter.as_ref().map(|_| "<fn>"))
            .field("search_handler", &self.search_handler.as_ref().map(|_| "<fn>"))
            .field("connection_filter", &self.connection_filter.as_ref().map(|_| "<fn>"))
            .field("require_auth", &self.require_auth)
            .field("on_disconnect", &self.on_disconnect.as_ref().map(|_| "<fn>"))
            .finish()
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_connections: Some(50),
            store_config: StoreConfig::default(),
            broadcast_buffer: 4096,
            asset_store_config: AssetStoreConfig::default(),
            enable_assets: true,
            asset_peer_urls: Vec::new(),
            data_dir: None,
            storage_key: None,
            event_filter: None,
            search_handler: None,
            connection_filter: None,
            require_auth: false,
            on_disconnect: None,
        }
    }
}

/// An Omnidea relay server.
///
/// Accepts WebSocket connections, stores events, serves subscriptions,
/// and broadcasts live events between connected clients.
///
/// Every Omnidea device can run one. Desktop machines can run it
/// always-on. Mobile devices relay while the app is open.
pub struct RelayServer {
    db: Option<RelayDatabase>,
    store: EventStore,
    asset_store: AssetStore,
    coalescer: FetchCoalescer,
    gospel_registry: Option<GospelRegistry>,
    live_tx: broadcast::Sender<OmniEvent>,
    binary_tx: broadcast::Sender<super::session::BinaryBroadcast>,
    addr: SocketAddr,
    config: ServerConfig,
    active_connections: Arc<AtomicUsize>,
    session_counter: Arc<AtomicU64>,
}

impl RelayServer {
    /// Create a new relay server bound to the given address (in-memory).
    pub fn new(addr: SocketAddr) -> Self {
        Self::with_config(addr, ServerConfig::default())
    }

    /// Create a new relay server with custom configuration.
    pub fn with_config(addr: SocketAddr, config: ServerConfig) -> Self {
        let (live_tx, _) = broadcast::channel(config.broadcast_buffer);
        let (binary_tx, _) = broadcast::channel(config.broadcast_buffer);

        let (db, store, asset_store, gospel_registry) =
            Self::init_storage(&config);

        Self {
            db,
            store,
            asset_store,
            coalescer: FetchCoalescer::new(),
            gospel_registry,
            live_tx,
            binary_tx,
            addr,
            config,
            active_connections: Arc::new(AtomicUsize::new(0)),
            session_counter: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Get a reference to the event store.
    pub fn store(&self) -> &EventStore {
        &self.store
    }

    /// Broadcast an event to all connected relay sessions.
    ///
    /// Used by Omnibus to push locally-published events to remote peers.
    /// Without this, events published by the relay's own Omnibus only get
    /// stored locally — the pool's dedup cache prevents them from being
    /// re-broadcast through the normal session→live_tx path.
    pub fn broadcast_live(&self, event: OmniEvent) {
        let _ = self.live_tx.send(event);
    }

    /// Get a reference to the asset store.
    pub fn asset_store(&self) -> &AssetStore {
        &self.asset_store
    }

    /// Get a reference to the gospel registry, if any.
    pub fn gospel_registry(&self) -> Option<&GospelRegistry> {
        self.gospel_registry.as_ref()
    }

    /// The address this server will bind to.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Number of currently active connections.
    pub fn active_connections(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Save the gospel registry to the database (call periodically or on shutdown).
    pub fn save_gospel(&self) {
        if let (Some(db), Some(registry)) = (&self.db, &self.gospel_registry) {
            registry.save_to_db(db);
        }
    }

    /// Run the relay server, accepting connections until stopped.
    pub async fn run(&self) -> Result<(), GlobeError> {
        let listener = TcpListener::bind(self.addr)
            .await
            .map_err(|e| GlobeError::ConnectionFailed {
                url: url::Url::parse(&format!("ws://{}", self.addr)).unwrap_or_else(|_| {
                    // Safety: literal URL is always valid.
                    url::Url::parse("ws://0.0.0.0:0").expect("literal URL parses")
                }),
                reason: e.to_string(),
            })?;

        log::info!("relay server listening on {}", self.addr);
        self.accept_loop(listener).await;
        Ok(())
    }

    /// Start the server on a random available port and return the actual address.
    ///
    /// Useful for testing — bind to `127.0.0.1:0` to get an OS-assigned port.
    pub async fn start_on_available_port() -> Result<(Self, SocketAddr), GlobeError> {
        Self::start_with_config(ServerConfig::default()).await
    }

    /// Start the server with custom config on a random port (localhost only).
    pub async fn start_with_config(
        config: ServerConfig,
    ) -> Result<(Self, SocketAddr), GlobeError> {
        // Safety: literal socket address is always valid.
        Self::start_at("127.0.0.1:0".parse().expect("literal socket addr parses"), config).await
    }

    /// Start the server at a specific address and return the actual bound address.
    ///
    /// Use `0.0.0.0:{port}` to accept connections from the local network.
    /// Use port 0 for an OS-assigned port.
    pub async fn start_at(
        addr: SocketAddr,
        config: ServerConfig,
    ) -> Result<(Self, SocketAddr), GlobeError> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| GlobeError::ConnectionFailed {
                // Safety: literal URL is always valid.
                url: url::Url::parse("ws://127.0.0.1:0").expect("literal URL parses"),
                reason: e.to_string(),
            })?;
        let addr = listener.local_addr().map_err(GlobeError::Io)?;

        let (live_tx, _) = broadcast::channel(config.broadcast_buffer);
        let (binary_tx, _) = broadcast::channel(config.broadcast_buffer);

        let (db, store, asset_store, gospel_registry) =
            Self::init_storage(&config);

        let active_connections = Arc::new(AtomicUsize::new(0));
        let session_counter = Arc::new(AtomicU64::new(0));

        let server = Self {
            db,
            store: store.clone(),
            asset_store: asset_store.clone(),
            coalescer: FetchCoalescer::new(),
            gospel_registry,
            live_tx: live_tx.clone(),
            binary_tx: binary_tx.clone(),
            addr,
            config: config.clone(),
            active_connections: active_connections.clone(),
            session_counter: session_counter.clone(),
        };

        let coalescer = server.coalescer.clone();
        let max_conns = config.max_connections;
        let enable_assets = config.enable_assets;
        let peer_urls = config.asset_peer_urls.clone();
        let event_filter = config.event_filter.clone();
        let search_handler = config.search_handler.clone();
        let connection_filter = config.connection_filter.clone();
        let on_disconnect = config.on_disconnect.clone();
        let require_auth = config.require_auth;
        tokio::spawn(async move {
            loop {
                let (mut stream, peer_addr) = match listener.accept().await {
                    Ok(conn) => conn,
                    Err(e) => {
                        log::warn!("accept error: {e}");
                        continue;
                    }
                };

                // Check connection filter (Tower IP allowlist, etc.).
                if let Some(ref filter) = connection_filter {
                    if !filter(peer_addr) {
                        log::debug!("connection from {peer_addr} rejected by filter");
                        drop(stream);
                        continue;
                    }
                }

                // Check connection limit.
                if let Some(max) = max_conns {
                    if active_connections.load(Ordering::Relaxed) >= max {
                        log::warn!("connection limit ({max}) reached, rejecting {peer_addr}");
                        drop(stream);
                        continue;
                    }
                }

                // Protocol detection: peek to determine HTTP asset vs WebSocket.
                // Also detects proxy-stripped Connection headers (cloudflared).
                let mut peek_buf = [0u8; 4096];
                let peeked = match stream.peek(&mut peek_buf).await {
                    Ok(n) => n,
                    Err(_) => {
                        drop(stream);
                        continue;
                    }
                };

                if enable_assets && asset_http::is_asset_request(&peek_buf[..peeked]) {
                    let ast = asset_store.clone();
                    let peers = peer_urls.clone();
                    let coal = coalescer.clone();
                    tokio::spawn(async move {
                        asset_http::handle_asset_request(stream, ast, peers, coal, None).await;
                    });
                    continue;
                }

                // Proxy compatibility: if a reverse proxy (e.g. cloudflared)
                // stripped the hop-by-hop `Connection: upgrade` header, inject
                // it so tokio-tungstenite's handshake succeeds.
                if needs_proxy_ws_fixup(&peek_buf[..peeked]) {
                    log::info!("proxy-stripped WebSocket from {peer_addr} — injecting upgrade headers");

                    // Consume the peeked bytes from the stream.
                    let mut consumed = vec![0u8; peeked];
                    if tokio::io::AsyncReadExt::read_exact(&mut stream, &mut consumed).await.is_err() {
                        drop(stream);
                        continue;
                    }

                    // Build the modified request with injected headers.
                    // Insert after the request line (first \r\n).
                    let mut modified = Vec::with_capacity(consumed.len() + 64);
                    if let Some(pos) = consumed.windows(2).position(|w| w == b"\r\n") {
                        modified.extend_from_slice(&consumed[..pos + 2]);

                        // Inject Upgrade header if missing.
                        let lower = std::str::from_utf8(&consumed)
                            .unwrap_or("")
                            .to_ascii_lowercase();
                        if !lower.contains("upgrade: websocket") {
                            modified.extend_from_slice(b"Upgrade: websocket\r\n");
                        }

                        // Replace or inject Connection header.
                        // Remove existing `Connection: keep-alive` line and add `Connection: Upgrade`.
                        let rest = &consumed[pos + 2..];
                        let rest_str = std::str::from_utf8(rest).unwrap_or("");
                        modified.extend_from_slice(b"Connection: Upgrade\r\n");
                        for line in rest_str.split("\r\n") {
                            if line.to_ascii_lowercase().starts_with("connection:") {
                                continue; // skip the proxy's Connection: keep-alive
                            }
                            modified.extend_from_slice(line.as_bytes());
                            modified.extend_from_slice(b"\r\n");
                        }
                        // Remove the trailing extra \r\n from the loop
                        // (the original already ended with \r\n\r\n).
                        if modified.ends_with(b"\r\n\r\n\r\n") {
                            modified.truncate(modified.len() - 2);
                        }
                    } else {
                        modified = consumed;
                    }

                    let prefixed = PrefixedStream {
                        prefix: modified,
                        offset: 0,
                        inner: stream,
                    };
                    let ws = match accept_async(prefixed).await {
                        Ok(ws) => ws,
                        Err(e) => {
                            log::warn!("ws handshake failed (proxied) for {peer_addr}: {e}");
                            continue;
                        }
                    };

                    let store = store.clone();
                    let live_tx = live_tx.clone();
                    let bin_tx = binary_tx.clone();
                    let conns = active_connections.clone();
                    let ef = event_filter.clone();
                    let sh = search_handler.clone();
                    let sid = session_counter.fetch_add(1, Ordering::Relaxed);
                    conns.fetch_add(1, Ordering::Relaxed);
                    let on_dc = on_disconnect.clone();
                    tokio::spawn(async move {
                        handle_session(ws, store, live_tx, bin_tx, sid, ef, sh, require_auth).await;
                        conns.fetch_sub(1, Ordering::Relaxed);
                        if let Some(ref cb) = on_dc {
                            cb(peer_addr);
                        }
                    });
                    continue;
                }

                let ws = match accept_async(stream).await {
                    Ok(ws) => ws,
                    Err(e) => {
                        log::warn!("ws handshake failed for {peer_addr}: {e}");
                        continue;
                    }
                };

                let store = store.clone();
                let live_tx = live_tx.clone();
                let bin_tx = binary_tx.clone();
                let conns = active_connections.clone();
                let ef = event_filter.clone();
                let sh = search_handler.clone();
                let sid = session_counter.fetch_add(1, Ordering::Relaxed);
                conns.fetch_add(1, Ordering::Relaxed);
                let on_dc = on_disconnect.clone();
                tokio::spawn(async move {
                    handle_session(ws, store, live_tx, bin_tx, sid, ef, sh, require_auth).await;
                    conns.fetch_sub(1, Ordering::Relaxed);
                    if let Some(ref cb) = on_dc {
                        cb(peer_addr);
                    }
                });
            }
        });

        Ok((server, addr))
    }

    // --- Private helpers ---

    fn init_storage(
        config: &ServerConfig,
    ) -> (
        Option<RelayDatabase>,
        EventStore,
        AssetStore,
        Option<GospelRegistry>,
    ) {
        match &config.data_dir {
            Some(data_dir) => {
                // Create directory if needed.
                if let Err(e) = std::fs::create_dir_all(data_dir) {
                    log::error!("failed to create data directory: {e}");
                    // Fall back to in-memory.
                    return (
                        None,
                        EventStore::with_config(config.store_config.clone()),
                        AssetStore::with_config(config.asset_store_config.clone()),
                        None,
                    );
                }

                let db_path = data_dir.join("relay.db");
                let db = match &config.storage_key {
                    Some(key) => RelayDatabase::open(&db_path, key),
                    None => {
                        // No key provided — auto-generate and persist one.
                        // Nothing unencrypted on disk.
                        RelayDatabase::open_auto(data_dir)
                    }
                };

                match db {
                    Ok(db) => {
                        let store =
                            EventStore::from_db(db.clone(), config.store_config.clone());
                        let asset_store = AssetStore::from_db(
                            db.clone(),
                            config.asset_store_config.clone(),
                        );
                        let gospel = GospelRegistry::load_from_db(
                            &db,
                            &GospelConfig::default(),
                        );

                        log::info!(
                            "relay storage opened at {} ({} events, {} assets, {} gospel records)",
                            db_path.display(),
                            store.len(),
                            asset_store.asset_count(),
                            gospel.total_count(),
                        );

                        (Some(db), store, asset_store, Some(gospel))
                    }
                    Err(e) => {
                        log::error!("failed to open relay database: {e}");
                        (
                            None,
                            EventStore::with_config(config.store_config.clone()),
                            AssetStore::with_config(config.asset_store_config.clone()),
                            None,
                        )
                    }
                }
            }
            None => {
                // In-memory mode (tests, or no persistence configured).
                (
                    None,
                    EventStore::with_config(config.store_config.clone()),
                    AssetStore::with_config(config.asset_store_config.clone()),
                    None,
                )
            }
        }
    }

    async fn accept_loop(&self, listener: TcpListener) {
        loop {
            let (stream, peer_addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    log::warn!("accept error: {e}");
                    continue;
                }
            };

            // Check connection filter (Tower IP allowlist, etc.).
            if let Some(ref filter) = self.config.connection_filter {
                if !filter(peer_addr) {
                    log::debug!("connection from {peer_addr} rejected by filter");
                    drop(stream);
                    continue;
                }
            }

            // Check connection limit.
            if let Some(max) = self.config.max_connections {
                if self.active_connections.load(Ordering::Relaxed) >= max {
                    log::warn!("connection limit ({max}) reached, rejecting {peer_addr}");
                    drop(stream);
                    continue;
                }
            }

            // Protocol detection: peek to determine HTTP asset vs WebSocket.
            if self.config.enable_assets {
                let mut peek_buf = [0u8; 256];
                let peeked = match stream.peek(&mut peek_buf).await {
                    Ok(n) => n,
                    Err(_) => {
                        drop(stream);
                        continue;
                    }
                };

                if asset_http::is_asset_request(&peek_buf[..peeked]) {
                    let ast = self.asset_store.clone();
                    let peers = self.config.asset_peer_urls.clone();
                    let coal = self.coalescer.clone();
                    let reg = self.gospel_registry.clone();
                    tokio::spawn(async move {
                        asset_http::handle_asset_request(stream, ast, peers, coal, reg).await;
                    });
                    continue;
                }
            }

            let ws = match accept_async(stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    log::warn!("websocket handshake failed for {peer_addr}: {e}");
                    continue;
                }
            };

            let store = self.store.clone();
            let live_tx = self.live_tx.clone();
            let bin_tx = self.binary_tx.clone();
            let conns = self.active_connections.clone();
            let ef = self.config.event_filter.clone();
            let sh = self.config.search_handler.clone();
            let require_auth = self.config.require_auth;
            let sid = self.session_counter.fetch_add(1, Ordering::Relaxed);
            conns.fetch_add(1, Ordering::Relaxed);
            tokio::spawn(async move {
                handle_session(ws, store, live_tx, bin_tx, sid, ef, sh, require_auth).await;
                conns.fetch_sub(1, Ordering::Relaxed);
                log::debug!("connection from {peer_addr} closed");
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_server_creates() {
        let addr: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        let server = RelayServer::new(addr);
        assert!(server.store().is_empty());
        assert_eq!(server.addr(), addr);
        assert_eq!(server.active_connections(), 0);
    }

    #[test]
    fn custom_config() {
        let addr: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        let config = ServerConfig {
            max_connections: Some(500),
            store_config: StoreConfig {
                max_events: Some(50_000),
            },
            broadcast_buffer: 8192,
            asset_store_config: AssetStoreConfig::default(),
            enable_assets: true,
            asset_peer_urls: Vec::new(),
            data_dir: None,
            storage_key: None,
            event_filter: None,
            search_handler: None,
            connection_filter: None,
            require_auth: false,
            on_disconnect: None,
        };
        let server = RelayServer::with_config(addr, config);
        assert!(server.store().is_empty());
        assert!(server.asset_store().is_empty());
    }

    #[test]
    fn persistent_config() {
        let dir = std::env::temp_dir().join(format!("globe_server_{}", std::process::id()));
        let config = ServerConfig {
            data_dir: Some(dir.clone()),
            ..Default::default()
        };
        let addr: SocketAddr = "127.0.0.1:9998".parse().unwrap();
        let server = RelayServer::with_config(addr, config);

        // Should have opened the database.
        assert!(server.store().is_empty());
        assert!(server.asset_store().is_empty());
        assert!(server.gospel_registry().is_some());

        // relay.db and relay.key should exist on disk (auto-generated key).
        assert!(dir.join("relay.db").exists());
        assert!(dir.join("relay.key").exists());

        // Key file should be exactly 32 bytes.
        let key = std::fs::read(dir.join("relay.key")).unwrap();
        assert_eq!(key.len(), 32);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn config_with_connection_filter() {
        let filter: ConnectionFilter = Arc::new(|addr: SocketAddr| {
            addr.ip() == std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
        });
        let config = ServerConfig {
            connection_filter: Some(filter),
            ..Default::default()
        };
        assert!(config.connection_filter.is_some());
    }

    #[test]
    fn config_with_auth_required() {
        let config = ServerConfig {
            require_auth: true,
            ..Default::default()
        };
        assert!(config.require_auth);
    }
}
