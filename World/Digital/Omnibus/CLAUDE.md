# Omnibus — The Omninet Node Runtime

The engine that makes your device a node in the web. Not an app — the runtime under every app. Every Throne app either embeds Omnibus (mobile) or connects to a running instance (desktop). Omnibus can also run headless on a Raspberry Pi, a home server, or a community anchor.

Omny is the management app that controls Omnibus instances across your devices.

## What Exists

### Rust Crate (`World/Digital/Omnibus/`)

```
Omnibus/
├── Cargo.toml            ← depends on crown, globe, tokio, serde, serde_json, chrono, url, log
└── src/
    ├── lib.rs              ← public API (re-exports all public types)
    ├── config.rs           ← OmnibusConfig (data_dir, device_name, port, bind_all, home_node, server_config, log_capture_capacity)
    ├── error.rs            ← OmnibusError (7 variants: NoIdentity, ServerFailed, DiscoveryFailed, IdentityFailed, NetworkFailed, Crown, Globe)
    ├── health_snapshot.rs  ← RelayHealthSnapshot (serializable relay health summary)
    ├── log_capture.rs      ← LogEntry + LogCapture (ring buffer for log entries)
    ├── runtime.rs          ← Omnibus struct + all public methods
    └── status.rs           ← OmnibusStatus (has_identity, pubkey, display_name, relay_port, relay_connections, relay_url, discovered_peers, pool_relays, has_home_node)
```

### Public API

**Startup:**
- `Omnibus::start(config)` — boots relay server, mDNS discovery (LocalBrowser + LocalAdvertiser), relay pool. Connects pool to own server and optional home node.
- `OmnibusConfig.server_config: Option<ServerConfig>` — optional relay server customization. Lets callers (e.g. Tower) set an event filter for content policy enforcement.

**Identity:**
- `create_identity(name)` — generate keypair, create Soul, publish profile
- `load_identity(path)` — restore identity from disk (soul dir + keyring.dat)
- `update_display_name(name)` — update and re-publish profile
- `pubkey()` / `pubkey_hex()` — get public key as crown_id bech32 or hex
- `profile_json()` — current profile as JSON
- `export_keyring()` — keyring as JSON bytes for syncing
- `import_keyring(data)` — load keyring from exported bytes (for syncing from another device)

**Network:**
- `publish(event)` — publish to all connected relays
- `post(content)` — sign + publish text note + seed to local store
- `subscribe(filters)` — subscribe with filters, returns (sub_id, broadcast::Receiver)
- `event_stream()` — broadcast receiver for all events (no filter)
- `seed_event(event)` — inject directly into local relay store
- `set_home_node(url)` — set persistent sync target
- `connect_relay(url)` — connect to a specific relay
- `query(filter)` — query events from local relay store

**Discovery:**
- `peers()` — all discovered peers via mDNS
- `connect_discovered_peers()` — connect pool to all discovered peers

**Gospel:**
- `gospel_registry()` — persistent DB-backed gospel registry
- `save_gospel()` — save registry to encrypted database

**Health & Diagnostics:**
- `relay_health()` — health snapshots for all relays in the pool (Vec<RelayHealthSnapshot>)
- `relay_health_for(url)` — health snapshot for a specific relay by URL
- `store_stats()` — event store statistics (count, oldest/newest, events by kind)

**Log Capture:**
- `push_log(entry)` — push a LogEntry into the capture ring buffer
- `recent_logs(count)` — get the most recent N log entries
- `log_capture()` — get Arc<Mutex<LogCapture>> for FFI callback wiring

**Status:**
- `status()` — full OmnibusStatus snapshot (pool_relays now reflects actual relay count)
- `port()` / `relay_url()` — local relay info
- `runtime()` — access tokio runtime for FFI layer

### Struct Internals

`Omnibus` holds: `Arc<Runtime>` (tokio), `RelayServer`, `SocketAddr`, `Mutex<RelayPool>`, `Mutex<Keyring>`, `Mutex<Option<Soul>>`, `Option<LocalAdvertiser>`, `LocalBrowser`, `Mutex<Option<Url>>` (home node), `Arc<Mutex<LogCapture>>`, `OmnibusConfig`.

### Console Types

- **RelayHealthSnapshot** — Serializable summary of Globe's `RelayHealth`. Contains url, state string, connected_since, last_activity, send/receive/error counts, average_latency_ms, score. `From<&RelayHealth>` conversion.
- **LogEntry** — Captured log entry: timestamp, level, module, message. Derives Clone, Debug, Serialize, Deserialize.
- **LogCapture** — Ring buffer for LogEntry. Push drops oldest at capacity. Methods: `push()`, `recent(n)`, `all()`, `len()`, `is_empty()`, `clear()`, `capacity()`. Does NOT install a global logger — the app/FFI layer feeds entries via `push()`.
- **StoreStats** — Re-exported from Globe. Event count, oldest/newest timestamps, events grouped by kind.

## What's Deferred

| Feature | Trigger | Why |
|---------|---------|-----|
| **Service manager** | Second Throne app on the same device | One app = everything runs. Two apps = need to coordinate. |
| **Identity isolation (IPC signing)** | Desktop daemon mode is built | Embedded mode has the keyring directly. IPC signing only matters when a daemon holds keys. |
| **Desktop daemon mode** | Two Throne apps coexisting on desktop | One app = embedded is fine. Two apps = daemon time. |
| **Sync service** | Multi-device usage beyond demo | mDNS discovery is networking. Sync is content replication. |
| **Web host service** | Webally is being built | Only needed when the browser exists and needs relays to serve Ideas. |

## Dependencies

Crown (identity, Keyring, Soul, CrownKeypair), Globe (RelayServer, RelayPool, RelayHealth, LocalBrowser, LocalAdvertiser, OmniEvent, OmniFilter, EventBuilder, ServerConfig, GlobeConfig, GospelRegistry, StoreStats), chrono (DateTime<Utc> for timestamps)

## Covenant Alignment

**Sovereignty** — your node is YOURS. Your keys never leave Omnibus. Your data lives on your devices. Home nodes and community anchors are opt-in, not required. **Dignity** — every person is a full participant, even with just a phone. **Consent** — sync, hosting, and sharing are all explicit choices.
