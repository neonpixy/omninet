# Tower — Always-On Network Nodes

The infrastructure backbone of the Omninet. Tower nodes run headless, serving the network 24/7. Two modes:

- **Pharos** — lightweight directory nodes. Gospel records only. Raspberry Pi territory. Caches names, relay hints, beacons, and lighthouse announcements. Rejects non-gospel content.
- **Harbor** — community content nodes. Everything Pharos does, plus stores and serves content for member communities. A community's Harbor is where content lives when members sleep.

---

## Architecture

```
Tower/
├── Cargo.toml          ← depends on omnibus, crown, sentinal, globe, magical-index, tokio, serde, url, chrono, env_logger
└── src/
    ├── lib.rs              ← module declarations + re-exports (TowerAnnouncement, TowerConfig, TowerMode, TowerError, PeeringLoop, Tower)
    ├── config.rs           ← TowerMode (Pharos|Harbor), TowerConfig (all tunables with defaults)
    ├── error.rs            ← TowerError (7 variants, From<OmnibusError>, From<GlobeError>)
    ├── announcement.rs     ← TowerAnnouncement (lighthouse events, kind 7032, to_event/from_event)
    ├── peering.rs          ← PeeringLoop (seed peers, add_peer, evangelize_all, recv_live_all, open_live_subscriptions)
    ├── runtime.rs          ← Tower (start, announce, status, content policy, gospel cycle, event filter, search)
    └── main.rs             ← omny-tower CLI binary (arg parsing, main loop with tokio::select!)
```

---

## Key Types

- **Tower** — The main struct. Wraps `Omnibus` with Tower-specific behavior: gospel peering, lighthouse announcements, content filtering, full-text search. Holds `Omnibus`, `Mutex<PeeringLoop>`, `KeywordIndex`, `Mutex<i64>` (last indexed timestamp), `TowerConfig`, and `Instant` (start time).
- **TowerMode** — `Pharos` or `Harbor`. Serializes as lowercase strings. Controls content policy and storage defaults.
- **TowerConfig** — All tunables. Serde round-trippable (JSON config file support). Notable fields:
  - `gospel_tiers: Vec<GospelTier>` — which tiers to propagate (empty = derive from mode). Pharos: Universal only. Harbor: Universal + Community.
  - `gospel_live_interval_secs` (default 2) — how often to drain persistent gospel subscriptions for new events.
  - Defaults: Pharos mode, port 7777, 60s gospel interval, 300s announce interval, max 16 gospel peers, 1000 max connections.
- **TowerAnnouncement** — Lighthouse broadcast payload. Serialized as JSON in an OmniEvent's content field. Kind 7032, d-tagged with the Tower's pubkey for replaceable semantics.
- **PeeringLoop** — Manages gospel peer connections. Two sync modes: bilateral (`evangelize_all`) for full catch-up, and live (`recv_live_all`) for draining persistent subscriptions. `take_peers()`/`restore_peers()` pattern for async work. Tier-aware (propagates configured `GospelTier` list).
- **TowerStatus** — Snapshot struct: mode, name, relay URL/port, connection count, identity info, gospel peer count/URLs, uptime, event count, indexed count, communities.
- **TowerError** — 7 variants: OmnibusStartFailed, IdentityFailed, PeeringFailed, AnnounceFailed, ConfigError, Omnibus(_), Globe(_).

---

## Dependencies

```toml
omnibus = { path = "../Omnibus" }         # node runtime (relay server, pool, identity)
crown = { path = "../../../Crown" }       # keypair for signing announcements
sentinal = { path = "../../../Sentinal" } # storage key derivation
globe = { path = "../../../Globe" }       # OmniEvent, kinds, filters, gospel, ServerConfig, EventFilter
magical-index = { path = "../MagicalIndex" } # full-text search (KeywordIndex, SearchIndex)
tokio                                      # async runtime (rt, signal, time, macros)
serde, serde_json                          # config serialization
url                                        # peer URL parsing
chrono                                     # time
log, env_logger                            # structured logging
```

**Five internal Omninet dependencies:** Omnibus, Crown, Sentinal, Globe, MagicalIndex.

---

## CLI Binary (`omny-tower`)

```
omny-tower [OPTIONS]

OPTIONS:
    --mode <pharos|harbor>   Operating mode (default: pharos)
    --name <NAME>            Tower name (default: "Omninet Tower")
    --port <PORT>            Relay port (default: 7777)
    --data-dir <PATH>        Data directory (default: ./tower_data)
    --seed <URL>             Seed peer URL (repeatable)
    --public-url <URL>       Public URL for announcements
    --community <PUBKEY>     Community to serve, Harbor only (repeatable)
    --config <PATH>          Load config from JSON file
    --help                   Show help
```

The main loop uses `tokio::select!` over four channels: Ctrl+C (shutdown), announce timer (default 5 min), gospel timer (default 60s), and live sync timer (default 2s). Runs on Omnibus's tokio runtime (no second runtime created).

---

## Content Policy

Tower enforces content policy via Globe's `EventFilter` (`Arc<dyn Fn(&OmniEvent) -> bool + Send + Sync>`), injected into `ServerConfig` at startup. The filter is built by `Tower::build_event_filter()`.

### What Gets Accepted

| Event Type | Pharos | Harbor (with communities) | Harbor (open, no communities) |
|-----------|--------|---------------------------|-------------------------------|
| Gospel registry (names, hints, beacons, lighthouse) | Yes | Yes | Yes |
| Profile (kind 0) | Yes | Yes | Yes |
| Contact list (kind 3) | Yes | Yes | Yes |
| Content from member pubkeys | No | Yes | Yes |
| Content from non-members | No | No | Yes |

A Harbor with an empty `communities` list is an open Harbor — it accepts all content.

---

## Gospel Peering

Tower participates in gospel — the evangelized discovery layer that propagates names, relay hints, and lighthouse announcements across the network.

### Two Sync Modes

1. **Bilateral sync** (`evangelize_all`) — Full catch-up on the gospel timer (default 60s). Exchange records the other is missing.
2. **Live sync** (`recv_live_all`) — Drain persistent subscriptions on the fast timer (default 2s). Non-blocking.

### How It Works

1. **Startup** — Tower connects to all `seed_peers` via Omnibus's relay pool. Creates `PeeringLoop` with configured tiers.
2. **PeeringLoop** — Created with seed URLs, capped at `max_gospel_peers` (default 16). Deduplicates by URL.
3. **Live subscriptions** — `open_live_subscriptions()` opens persistent subscriptions on peers that don't have one yet.
4. **Dynamic discovery** — `add_gospel_peer(url)` adds peers found from lighthouse announcements.

---

## Full-Text Search (MagicalIndex)

Tower embeds a `KeywordIndex` for full-text search. Events are indexed incrementally after each gospel cycle via `index_new_events()`. The `indexed_count` is reported in TowerStatus.

Tower implements Globe's `SearchHandler` trait, enabling relay-level search: when a client sends a search query, the relay delegates to MagicalIndex and returns matching events.

---

## Lighthouse Announcements

Kind 7032 events propagated through gospel. Tags: `["d", "<tower_pubkey>"]` (replaceable), `["mode", "pharos"|"harbor"]`, `["r", "<relay_url>"]`. Content is JSON-serialized `TowerAnnouncement` with mode, relay_url, name, gospel_count, event_count, uptime_secs, version, communities.

---

## Identity

Tower does NOT auto-generate identity. The relay runs without a Crown. Identity is loaded on startup only if `keyring.dat` exists in the parent of `data_dir` (where the daemon saves the user's keyring). Without identity, the Tower can serve relay traffic but cannot sign lighthouse announcements. The Chancellor owns identity persistence — Tower is a consumer, not a creator.

---

## What's Next / Deferred

| Feature | Trigger | Notes |
|---------|---------|-------|
| **Lighthouse peer discovery** | Multiple Tower nodes running | Parse incoming lighthouse announcements to auto-discover and connect to new gospel peers. |
| **Kingdom membership checks** | Kingdom governance is built | Harbor currently checks if `author` is in the `communities` list. Production would verify Kingdom membership. |
| **Asset serving** | Harbor needs binary content | `max_asset_bytes` config exists but asset store wiring is deferred. |
| **Health monitoring** | Undercroft/HQ is built | TowerStatus exists; feeding it into HQ is future work. |

---

## Covenant Alignment

**Sovereignty** — Tower nodes are independently operated. Anyone can run a Pharos or Harbor. No central authority controls which Towers exist or what they serve. Your Tower, your rules (within the Covenant). **Dignity** — Pharos nodes are lightweight enough for a Raspberry Pi, ensuring infrastructure participation isn't gated by hardware. **Consent** — Harbor community membership is explicit. Content filtering is transparent. Open Harbors are an opt-in choice by the operator.
