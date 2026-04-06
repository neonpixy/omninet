# Globe — Networking

The connected world. Globe defines the **Omninet Relay Protocol (ORP)** — a custom relay protocol inspired by relay architecture but designed for Omninet's full needs. It moves signed, content-addressed events between people over a decentralized relay network. Globe is a transport layer — it doesn't understand the content it carries.

## Source Layout

```
Globe/
├── Cargo.toml
├── src/
│   ├── lib.rs              ← module declarations + re-exports
│   ├── error.rs            ← GlobeError (is_retryable/is_configuration_error)
│   ├── config.rs           ← GlobeConfig (all tunables with defaults)
│   ├── event.rs            ← OmniEvent (7-field signed event, tag accessors, validation)
│   ├── event_id.rs         ← deterministic JSON serialization + SHA-256 ID computation
│   ├── kind.rs             ← event kinds, 26 ABC ranges, Subsystem enum, Yoke/Zeitgeist kinds
│   ├── filter.rs           ← OmniFilter (custom serde for #-tags, client-side matching)
│   ├── protocol.rs         ← ClientMessage / RelayMessage (JSON array wire format, bidirectional)
│   ├── auth.rs             ← challenge-response relay authentication (kind 22242)
│   ├── event_builder.rs    ← UnsignedEvent + EventBuilder (signs via Crown)
│   ├── name.rs             ← domain name system (claim/update/transfer/delegate/revoke)
│   ├── health.rs           ← ConnectionState, RelayHealth, composite scoring
│   ├── asset.rs            ← AssetBuilder + AssetRecord (kind 7020 asset announcements)
│   ├── chunk.rs            ← ChunkManifest, ChunkBuilder, ChunkInfo (resumable large file transfer)
│   ├── signaling.rs        ← SignalingBuilder (Communicator session events over ORP)
│   ├── deeplink.rs         ← OmnideaUri, GlobeName, UriAction, UriHandler, UriRouter, LinkBuilder
│   ├── gospel/
│   │   ├── mod.rs          ← module declarations + re-exports
│   │   ├── config.rs       ← GospelConfig (evangelize interval, capacity, peers)
│   │   ├── registry.rs     ← GospelRegistry (local cache, conflict resolution)
│   │   ├── hints.rs        ← HintBuilder + parse_hint (relay hint events, kind 7010)
│   │   ├── sync.rs         ← GospelSync (filter building, merge helpers, diff)
│   │   ├── peer.rs         ← GospelPeer (relay-to-relay peering, evangelize())
│   │   ├── tier.rs         ← GospelTier (Universal/Community/Extended propagation control)
│   │   └── digest.rs       ← SemanticDigest, ConceptEquivalence, SynapseEdge (Tower knowledge exchange)
│   ├── discovery/
│   │   ├── mod.rs          ← module declarations
│   │   ├── local.rs        ← LocalAdvertiser + LocalBrowser (mDNS/DNS-SD discovery)
│   │   ├── beacon.rs       ← BeaconBuilder + BeaconRecord (community discovery, kind 7030)
│   │   ├── invitation.rs   ← Invitation, InvitationBuilder, InvitationLink (onboarding, kind 7042)
│   │   ├── network_key.rs  ← NetworkKeyMaterial, NetworkKeyEnvelope, KeyRotation, NetworkKeyBuilder (kind 7040-7041)
│   │   ├── pairing.rs      ← PairingChallenge, PairingResponse, DevicePair (multi-device pairing)
│   │   ├── profile.rs      ← DeviceProfile, DeviceType, DeviceCondition, ServingPolicy, ConnectionType
│   │   ├── address.rs      ← AddressInfo, EncryptedAddresses (Network Key encrypted relay addresses)
│   │   └── upnp.rs         ← UPnP port mapping for Tower relay nodes (non-fatal, auto-detected)
│   ├── client/
│   │   ├── mod.rs          ← module declarations
│   │   ├── connection.rs   ← RelayHandle + connection task (channels, state machine)
│   │   ├── pool.rs         ← RelayPool (multi-relay, LRU dedup, concurrent publish)
│   │   └── blocking.rs     ← BlockingGlobe sync wrappers (feature-gated)
│   ├── collaboration.rs  ← Real-time multiplayer bridge (Equipment Communicator → Globe relay events)
│   ├── idea_sync.rs      ← Cross-device .idea sync via CRDT operations over relay events
│   ├── commons.rs        ← R4A Globe Commons (cross-community public square)
│   ├── camouflage.rs     ← R5A Protocol Camouflage (traffic padding + shaping)
│   ├── jurisdiction.rs   ← R5B Multi-Jurisdiction Relay Mesh (diversity scoring)
│   └── server/
│       ├── mod.rs          ← module declarations
│       ├── database.rs     ← RelayDatabase (shared SQLCipher-encrypted connection)
│       ├── storage.rs      ← EventStore (SQLite-backed, indexed by kind+author+created_at)
│       ├── session.rs      ← per-client WebSocket handler (subscriptions, live + binary broadcast)
│       ├── listener.rs     ← RelayServer, ServerConfig, EventFilter, SearchHandler, SearchHit
│       ├── asset_store.rs  ← AssetStore (SQLite BLOBs, content-addressed, SHA-256, eviction)
│       ├── asset_http.rs   ← HTTP asset endpoints (GET/PUT/HEAD /asset/{hash})
│       ├── asset_fetch.rs  ← FetchCoalescer (pull-through caching, gospel-aware peers)
│       ├── sfu.rs          ← SfuRouter (selective forwarding for group video calls)
│       └── network_defense.rs ← Connection-level defense (IP allowlists, rate limiting, policies)
└── tests/
    └── integration.rs
```

## Architecture

```
Globe (Omninet Relay Protocol)
    ├── OmniEvent: Content-addressed signed events
    │   ├── 7 fields: id, author, created_at, kind, tags, content, sig
    │   ├── ID = SHA-256 of canonical serialization (manual JSON)
    │   └── Signed via Crown's BIP-340 Schnorr signatures
    ├── ORP Wire Protocol (text + binary frames)
    │   ├── Client → Relay: EVENT, REQ, CLOSE, AUTH
    │   ├── Relay → Client: EVENT, STORED, OK, NOTICE, CLOSED, AUTH
    │   └── Binary frames: 0x01 = MessagePack, 0x02 = raw blob
    ├── Kind Taxonomy: 26 ABC ranges (1000 per letter)
    │   ├── Standard: 0-999 (profile=0, text=1, contacts=3)
    │   ├── A-Z: 1000-26999 (Equipment=5000, Fortune=6000, Globe=7000, Ideas=9000, etc.)
    │   ├── Yoke: 25000-25006 (relationship, version_tag, branch, merge, milestone, ceremony, activity)
    │   ├── Zeitgeist: 26000+ (semantic_profile=26000)
    │   ├── Replaceable: 30000-39999, Parameterized: 40000-49999
    │   └── Extension: 50000+
    ├── Signaling (Equipment Communicator over ORP)
    │   ├── 5100 OFFER, 5101 ANSWER, 5102 END, 5103 ICE_CANDIDATE
    │   └── 5110 STREAM_ANNOUNCE, 5111 STREAM_UPDATE, 5112 STREAM_END, 5113 STREAM_RECORDING
    ├── Discovery
    │   ├── 7030 BEACON, 7031 BEACON_UPDATE, 7032 LIGHTHOUSE_ANNOUNCE
    │   ├── 7040 KEY_DELIVERY, 7041 KEY_ROTATION, 7042 INVITATION
    │   └── Local: mDNS/DNS-SD via _omnidea._tcp (LocalAdvertiser + LocalBrowser)
    ├── Naming System (Globe kinds 7000-7004)
    │   ├── Domain-style names: sam.idea, shop.sam.idea
    │   ├── claim, update, transfer, delegate, revoke
    │   └── Resolution via relay subscription filters
    ├── Deep Linking
    │   ├── omnidea:// URI scheme (OmnideaUri parse/serialize)
    │   ├── .idea TLD (GlobeName parse/resolve via GospelRegistry)
    │   ├── UriRouter + UriHandler trait (app-registered routing)
    │   └── LinkBuilder (convenience: post, design, community, profile, invite, app)
    ├── Assets
    │   ├── AssetBuilder: announce events (kind 7020, d-tag=hash, SHA-256 validated)
    │   └── AssetRecord: parsed announcement (hash, mime, size, relay_urls)
    ├── Chunks (kind 9000)
    │   ├── ChunkManifest: content_hash, total_size, chunk_size, ordered chunks
    │   ├── ChunkBuilder: split/manifest/parse_manifest/verify/missing_chunks
    │   └── Resumable: fetch only missing chunks by hash
    ├── Gospel (evangelized discovery)
    │   ├── GospelRegistry: local cache of names + relay hints
    │   │   ├── Names: first-claim for different authors, latest for same author
    │   │   ├── Hints: latest from same author always wins
    │   │   ├── Thread-safe (Arc<RwLock<>>), snapshot/restore for persistence
    │   │   └── Capacity limits with hard rejection
    │   ├── GospelTier: propagation scope (Universal/Community/Extended)
    │   │   ├── Universal: names, relay hints, lighthouse (every node)
    │   │   ├── Community: beacons, asset announcements (community peers)
    │   │   └── Extended: pull-on-demand only
    │   ├── SemanticDigest: concept knowledge exchanged during Tower peering
    │   │   ├── ConceptEquivalence (e.g., "woodworking" ≈ "carpentry")
    │   │   ├── SynapseEdge (weighted concept relationships)
    │   │   └── Merge with dedup (higher confidence/weight wins)
    │   ├── HintBuilder: relay hint events (kind 7010)
    │   ├── GospelSync: bilateral sync helpers (filter, merge, diff)
    │   └── GospelPeer: relay-to-relay peering (evangelize cycle)
    ├── Commons (R4A): Cross-community public square
    │   ├── COMMONS_PUBLICATION kind 7100 (wrapper event referencing original)
    │   ├── CommonsTag: CrossCommunity, PublicDiscourse, SharedKnowledge, OpenQuestion, Announcement
    │   ├── CommonsPolicy: per-community publish strategy (Default/OptIn/OptOut/Disabled)
    │   └── CommonsFilter: query extension layered on OmniFilter
    ├── Camouflage (R5A): Traffic analysis resistance
    │   ├── CamouflageMode: Standard (ORP over WSS), Padded, Shaped
    │   ├── TrafficPadder: 4-byte BE length prefix + data + random padding (pad/unpad)
    │   └── TrafficShaper: schedule message sends per ShapingProfile (Browsing/Streaming/Messaging)
    ├── Jurisdiction (R5B): Multi-jurisdiction relay diversity
    │   ├── RelayJurisdiction: per-relay metadata (country, region, LegalFramework)
    │   ├── LegalFramework: StrongPrivacy, Moderate, Weak, Adversarial, Unknown
    │   ├── JurisdictionAnalyzer: Simpson's diversity index (1 - D)
    │   └── DiversityRecommendation: Healthy, AddDiversity, CriticallyHomogeneous
    ├── RelayPool: Multi-relay coordinator
    │   ├── LRU deduplication (proper eviction, not random)
    │   ├── Concurrent publish (at-least-once semantics)
    │   └── Aggregated subscriptions with resubscribe-on-reconnect
    ├── RelayHandle: Per-connection task (tokio::spawn)
    │   ├── Channel-based: mpsc (commands), watch (state), broadcast (events)
    │   └── Connection task processes commands, manages lifecycle
    ├── Auth: Challenge-response (kind 22242)
    ├── Health: Relay scoring (uptime, latency, error rate)
    └── Server: Built-in relay server (every device can be a relay)
        ├── EventStore (SQLite-backed, indexed by kind+author+created_at)
        │   ├── Configurable max_events, oldest evicted when full
        │   └── In-memory SQLite for tests, file-backed SQLCipher for production
        ├── AssetStore (SQLite BLOBs, content-addressed, SHA-256 keyed)
        │   ├── Configurable max_bytes (default 512 MB), age-based eviction
        │   └── HTTP endpoints: PUT/GET/HEAD /asset/{hash}
        ├── FetchCoalescer (pull-through caching + gospel-aware peer discovery)
        ├── Session handler (per-client WebSocket, subscriptions, live + binary broadcast)
        ├── SFU Router (selective forwarding for group video calls)
        │   ├── SfuSession: per-call participant tracking
        │   ├── MediaLayer: simulcast (180p/360p/720p)
        │   └── route(): sender + layer → ForwardTarget[] (who gets what)
        ├── SearchHandler: pluggable full-text/semantic search (Tower wires MagicalIndex)
        │   └── SearchHit: event_id + relevance score + optional snippet
        └── RelayServer (TCP listener, connection limit, spawns sessions)
            ├── Configurable max_connections (default 1000)
            ├── Protocol detection (HTTP assets vs WebSocket on same port)
            └── EventFilter: content policy closure (Pharos=gospel-only, Harbor=community)
```

## Key Types

- **OmniEvent** — 7-field content-addressed signed event. ID is SHA-256 of canonical serialization. Signature is BIP-340 Schnorr via Crown. Tags as `Vec<Vec<String>>` for extensibility.
- **EventBuilder** — `sign(unsigned, keypair) -> OmniEvent`, `verify(event) -> bool`. Standard helpers for profile/text/contacts.
- **UnsignedEvent** — Builder pattern: `new(kind, content).with_tag().with_d_tag().with_application_tag()`.
- **OmniFilter** — Subscription filter with custom serde for `#e`, `#p`, `#<letter>` tag encoding. `matches(event)` for client-side filtering.
- **ClientMessage** / **RelayMessage** — Wire protocol enums with custom JSON array serialization.
- **RelayHandle** — External API for a relay connection. Send + Sync. Communicates with background task via channels.
- **RelayPool** — Multi-relay coordinator. `add_relay()`, `publish()`, `subscribe()`. LRU dedup via `lru::LruCache`.
- **NameBuilder** — `claim("sam.idea", keypair)`, `transfer()`, `delegate_subdomain()`, `revoke()`.
- **NameRecord** / **NameParts** — Parsed from name events: name, owner, target, description, updated_at.
- **GlobeConfig** — All tunables: relay URLs, reconnect delays, heartbeat interval, dedup cache size, etc.
- **ConnectionState** — Disconnected, Connecting, Connected, Reconnecting { attempt }, Failed { reason }.
- **RelayHealth** — URL, state, metrics (send/receive/error counts), latency window, composite `score()`.

### Server Types
- **EventStore** — SQLite-backed event store (via RelayDatabase). In-memory SQLite for tests, file-backed SQLCipher for production. Configurable `max_events` with oldest-first eviction.
- **RelayDatabase** — Shared SQLCipher-encrypted connection. Thread-safe via `Arc<Mutex<Connection>>`.
- **RelayServer** — TCP/WebSocket relay server. Configurable `max_connections` with graceful rejection.
- **ServerConfig** — max_connections, store_config, broadcast_buffer, data_dir, storage_key, event_filter, search_handler.
- **EventFilter** — `Arc<dyn Fn(&OmniEvent) -> bool + Send + Sync>`. Content policy enforcement for Tower.
- **SearchHandler** — `Arc<dyn Fn(&str, &OmniFilter) -> Vec<SearchHit> + Send + Sync>`. Tower wires MagicalIndex here.
- **SearchHit** — event_id, relevance score, optional text snippet.
- **AssetStore** / **AssetStoreConfig** — SQLite BLOB store, content-addressed by SHA-256, configurable max_bytes.

### Gospel Types
- **GospelRegistry** — Thread-safe local cache of registry records (names + hints). Conflict resolution: first-claim for names (different authors), latest-wins for updates (same author) and hints. Snapshot/restore for persistence.
- **GospelConfig** — Evangelize interval, max peers, peer URLs, capacity limits, signature verification toggle.
- **GospelPeer** / **PeerState** — Wraps `RelayHandle` for relay-to-relay peering. Bilateral sync via `evangelize()`.
- **GospelSync** / **MergeStats** — Static helpers: `sync_filter(since)`, `merge_events()`, `diff()`, `merge_sets()`.
- **GospelTier** — Universal (every node), Community (community peers), Extended (pull-on-demand).
- **SemanticDigest** — Concept knowledge exchanged during Tower peering. ConceptEquivalence + SynapseEdge with dedup merge.
- **HintBuilder** — `relay_hints(urls, keypair) -> OmniEvent` (kind 7010).
- **RelayHintRecord** — Parsed from hint events: author, relay URLs, published_at.
- **InsertResult** — Inserted / Rejected / Duplicate.
- **RegistrySnapshot** — Serializable snapshot of all names + hints + high water mark.

### Discovery Types
- **LocalAdvertiser** — mDNS/DNS-SD advertising via `_omnidea._tcp`. Publishes instance name, port, optional pubkey.
- **LocalBrowser** — mDNS/DNS-SD browsing. Background thread collects `LocalPeer` entries. Auto-cleanup on service removal.
- **LocalPeer** — name, addresses (IPv4/v6), port, optional pubkey_hex. `ws_url()` helper.
- **BeaconBuilder** / **BeaconRecord** — Community beacon events (kind 7030). Self-describing, gospel-propagated entry points with tags, member_count, relay_urls, preview, icon_hash.
- **Invitation** / **InvitationBuilder** / **InvitationLink** — Onboarding flow (kind 7042). Carries Network Key envelope, relay URLs, one-time pairing token.
- **NetworkKeyMaterial** — 256-bit key (versioned, base64-encoded, always encrypted in transit).
- **NetworkKeyEnvelope** — Encrypted delivery of Network Key to a specific recipient (ECDH + AES-256-GCM).
- **KeyRotation** / **RotationReason** — Key rotation announcement (kind 7041). Grace period, reason (Scheduled/Compromise/Upgrade).
- **NetworkKeyBuilder** — Builds ORP events for key delivery and rotation.
- **PairingChallenge** / **PairingResponse** / **DevicePair** — Multi-device pairing via challenge-response (QR code or mDNS).
- **DeviceProfile** / **DeviceType** / **DeviceCondition** / **ServingPolicy** / **ConnectionType** — Device capability profiles. `compute_policy()` derives serving behavior from device type + conditions (battery, connection type, etc.).
- **AddressInfo** / **EncryptedAddresses** — Device network addresses, encrypted with Network Key for relay hints.

### Signaling Types
- **SignalingBuilder** — Builds ORP events for Communicator session signaling: `offer()` (5100), `answer()` (5101), `end()` (5102), `ice_candidate()` (5103), `stream_announce()` (5110), `stream_update()` (5111), `stream_end()` (5112), `stream_recording()` (5113). Content is opaque (typically encrypted by caller). `parse_session_id()` extracts session tags.

### Deep Linking Types
- **OmnideaUri** — Parsed `omnidea://` URI. Fields: app, resource_type, resource_id, params. Parse/serialize/Display.
- **GlobeName** — Parsed `.idea` address (e.g., `sam.idea/portfolio`). `resolve()` via GospelRegistry.
- **UriAction** — Navigate, Open, Invite, Unknown.
- **UriHandler** — Trait for app-specific URI handling.
- **UriRouter** — Routes URIs to registered handlers. First-match wins.
- **LinkBuilder** — Convenience: `post()`, `design()`, `community()`, `profile()`, `invite()`, `app()`.

### Asset / Chunk Types
- **AssetBuilder** — `announce(hash, mime, size, relay_url, keypair)` (kind 7020). SHA-256 hash validation.
- **AssetRecord** — Parsed announcement: hash, mime, size, relay_urls, announced_at.
- **ChunkManifest** — content_hash, total_size, chunk_size, ordered ChunkInfo list.
- **ChunkBuilder** — `split()`, `manifest()`, `parse_manifest()`, `verify()`, `missing_chunks()`.
- **ChunkInfo** — hash, size, index for a single chunk.

### Commons Types (R4A)
- **CommonsEvent** -- Wraps an `OmniEvent` (kind 7100) with commons-specific metadata: `source_community`, `commons_tags`. `referenced_event_id()` gets the original event ID from the `e` tag. `is_valid()` checks kind + reference.
- **CommonsTag** -- Classification: CrossCommunity, PublicDiscourse, SharedKnowledge, OpenQuestion, Announcement. String round-trip via `as_str()` / `from_str()`.
- **CommonsPublishPolicy** -- Per-community opt-in: Default (member choice), OptIn (community selects), OptOut (auto-publish), Disabled.
- **CommonsPolicy** -- Stored in community Charter. Fields: `publish_to_commons`, `read_from_commons`, `commons_relay_urls`. Helpers: `can_publish()`, `auto_publishes()`, `member_choice_available()`.
- **CommonsFilter** -- Query extension: `commons_only()`, `from_community()`, `with_tag()`. `to_omni_filter()` converts to an `OmniFilter` for relay queries.
- **`commons_publication_tags()`** -- Builds ORP event tags for a Commons publication.

### Camouflage Types (R5A)
- **CamouflageConfig** -- Top-level: `enabled` + `CamouflageMode`. Default: enabled, Standard mode.
- **CamouflageMode** -- Standard (ORP over WSS, no extra processing), Padded(PaddingConfig), Shaped(ShapingProfile).
- **PaddingConfig** -- `min_pad` (32), `max_pad` (256), `pad_interval_ms` (0 = no decoy traffic). Validates min <= max.
- **TrafficPadder** -- Stateless. Wire format: `[4-byte BE data length][data][random padding]`. `pad(data, config) -> Vec<u8>`, `unpad(padded) -> Option<Vec<u8>>`.
- **ShapingProfile** -- Browsing (800ms base, 1200ms jitter), Streaming (50ms base, 30ms jitter), Messaging (300ms base, 2000ms jitter).
- **TrafficShaper** -- Stateless. `shape(messages, profile) -> Vec<ShapedMessage>`. Computes `scheduled_send_at_ms` offsets; caller is responsible for holding messages until their scheduled time.
- **ShapedMessage** -- `data` + `scheduled_send_at_ms` (offset from shaping start).

### Jurisdiction Types (R5B)
- **RelayJurisdiction** -- Per-relay: `relay_url`, `country_code` (ISO 3166-1 alpha-2), `region`, `legal_framework`. Builder: `new()`, `unknown()`, `.with_framework()`.
- **LegalFramework** -- StrongPrivacy (CH, IS), Moderate (EU GDPR), Weak (US), Adversarial (censorship/backdoors), Unknown.
- **JurisdictionAnalyzer** -- Stateless. `analyze(relays) -> JurisdictionDiversity`. Uses Simpson's diversity index: `1 - sum(n_i * (n_i - 1)) / (N * (N - 1))`. Score 0.0 = all same jurisdiction, approaching 1.0 = perfectly diverse.
- **JurisdictionDiversity** -- `connected_relays`, `unique_jurisdictions`, `diversity_score`, `adversarial_only`, `recommendation`.
- **DiversityRecommendation** -- Healthy (3+ jurisdictions, score > 0.5, not all adversarial), AddDiversity (suggestion), CriticallyHomogeneous (warning).

## Dependencies

```toml
crown = { path = "../Crown" }      # signing events
tokio, tokio-tungstenite, futures-util  # async WebSocket
backon                              # exponential backoff
lru                                 # deduplication cache
url                                 # relay URL parsing
sha2, hex                           # event ID computation
rmp-serde                           # binary frame support (MessagePack)
rusqlite = { bundled-sqlcipher }    # encrypted persistent storage
mdns-sd                             # local network discovery (mDNS/DNS-SD)
libc                                # system hostname
serde, serde_json, thiserror, chrono, log
```

**Crown is the ONLY internal Omninet dependency.** No X, no Sentinal, no Equipment.

Features: `blocking` — enables `BlockingGlobe` sync wrappers.

## Protocol: ORP v1

**Not Nostr-compatible.** Omninet's own relay protocol. Key differences:
- `"author"` instead of `"pubkey"` in events
- `"STORED"` instead of `"EOSE"` for end-of-stored-events
- Kind numbering: 26 ABC ranges (1000-26999) instead of Nostr's 31xxx
- Built-in naming system (Globe kinds 7000-7004)
- Binary frame support (MessagePack + raw blobs)
- Nexus (N) handles legacy interop later

## Gospel — The Discovery Flow

Gospel enables global discovery. The full flow from "I know a name" to "I can see their content":

1. Alice wants to find `bob.idea`
2. She checks her local `GospelRegistry` — if cached, skip to step 4
3. She queries connected relays: `OmniFilter::for_name("bob.idea")`
4. She gets Bob's `NAME_CLAIM` event -> knows his public key
5. She queries: `OmniFilter::for_relay_hints(bob_pubkey)`
6. She gets Bob's `RELAY_HINT` event -> knows which relays he uses
7. She connects to Bob's relays and subscribes to his content

Gospel ensures steps 3 and 5 work globally because registry records evangelize between peered relays. Relay peering is relay-to-relay — when Relay A and Relay B connect as gospel peers, they exchange all registry records the other is missing.

**Not yet wired:** GospelPeer's `evangelize()` has the full bilateral sync architecture but event collection from the broadcast channel is stubbed until RelayHandle's connection task is fully wired with live WebSocket connections.

## What Does NOT Live Here

- **Content encryption** -> Sentinal (caller encrypts before publishing)
- **CRDT logic** -> X (synced over Globe events)
- **Pact integration** -> Equipment/Divinity wiring
- **TLD governance** -> Kingdom/Polity
- **Name payment** -> Fortune
- **NAT traversal** -> needed for peer-to-peer relay connections behind routers

## Covenant Alignment

**Sovereignty** — relays are decentralized; no single relay can censor you. Your keys, your identity, your content. **Consent** — subscriptions are explicit and revocable. **Dignity** — all events are signed; you can verify who said what. Domain names are sovereign — no registrar can take yours. The Network Key encrypts relay addresses so the network is opaque to outside observers. Presence-related data (from World/Physical) never flows through Globe as persistent events.
