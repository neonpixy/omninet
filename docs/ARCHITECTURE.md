# Omninet Architecture

Omninet is a protocol suite for a sovereign internet. 29 Rust crates, minimal coupling, connected by string-routed messages. No crate "knows about" the others at runtime — they discover each other through Equipment's string-based routing, like microservices that happen to live in the same binary.

This document explains how the pieces fit together.

---

## The Layers

Omninet's crates organize into 6 layers. Lower layers have zero or minimal dependencies. Higher layers compose what's below them. The rule: **depend downward, never sideways within a layer** (with rare, explicit exceptions).

```
Layer 6 ── Meta ──────────────────────────────────────────────────────
           Undercroft    Oracle    Quest    Yoke    Zeitgeist
           (observe)    (guide)   (play)   (remember) (discover)

Layer 5 ── World ─────────────────────────────────────────────────────
           Omnibus      Tower     MagicalIndex    Physical
           (node)       (relay)   (search)        (geography)

Layer 4 ── Intelligence ──────────────────────────────────────────────
           Advisor    Magic     Regalia    Nexus
           (think)    (render)  (design)   (bridge)

Layer 3 ── Civilization ──────────────────────────────────────────────
           Polity    Kingdom    Fortune    Bulwark    Jail
           (rights)  (govern)   (trade)    (protect)  (account)

Layer 2 ── Core Infrastructure ───────────────────────────────────────
           Crown      Sentinal    Vault    Hall    Globe    Lingo
           (identity) (encrypt)   (store)  (file)  (relay)  (translate)

Layer 1 ── Communication + Content ───────────────────────────────────
           Equipment              Ideas
           (messages)             (content)

Layer 0 ── Foundation ────────────────────────────────────────────────
           X
           (utilities)
```

### Layer 0: Foundation

**X** is the shared utility crate. Value types, vector clocks, a generic CRDT engine, geometry primitives, color math (HSL, HSB, WCAG contrast), and geographic coordinates. Zero dependencies on any other Omninet crate. Everything imports X.

### Layer 1: Communication + Content

**Equipment** is the nervous system. Five actors for inter-module communication — all string-routed, zero imports between modules:

| Actor | Pattern | Example |
|-------|---------|---------|
| **Phone** | Request/response RPC | `"vault.getEntries"` -> entries JSON |
| **Email** | Fire-and-forget pub/sub | `"crdt.documentChanged"` -> all subscribers notified |
| **Contacts** | Module registry | "Who handles `studio.createFrame`?" |
| **Pager** | Notification queue | Priority levels, read/dismiss/badge |
| **Communicator** | Real-time sessions | Voice calls, collaboration, live cursors |

Equipment has zero internal dependencies. It defines traits (`PhoneCall`, `EmailEvent`, `CommunicatorChannel`) and routes by string IDs. This is how 29 crates talk to each other without importing each other.

**Ideas** is the universal content format. Everything in Omnidea is an `.idea` — a note, a spreadsheet, a design, a storefront. An `.idea` is a directory containing a header, a tree of digits (the atomic content units), authority records, bonds, and optionally coinage. Depends only on X.

### Layer 2: Core Infrastructure

| Crate | What It Does | Key Insight |
|-------|-------------|-------------|
| **Crown** | Cryptographic identity (secp256k1 BIP-340 Schnorr) | Zero deps. No crate can revoke your identity. |
| **Sentinal** | Encryption (AES-256-GCM, PBKDF2, HKDF, BIP-39) | Crown keys never touch Sentinal directly — traits bridge them. |
| **Vault** | Encrypted storage (SQLCipher, lock/unlock lifecycle) | Keys are held in memory, never on disk. |
| **Hall** | File I/O for `.idea` packages | Header plaintext, content encrypted, assets obfuscated. |
| **Globe** | Networking via ORP (Omninet Relay Protocol) | Signed events over WebSocket. See [ORP spec](../_Specs/orp.md). |
| **Lingo** | Language and translation (Babel semantic obfuscation) | 90K+ Unicode symbols defeat frequency analysis. |

### Layer 3: Civilization

| Crate | What It Does | Lines | Tests |
|-------|-------------|------:|------:|
| **Polity** | Constitutional enforcement — rights, duties, protections, consent | 9,674 | 279 |
| **Kingdom** | Community governance — proposals, 6 voting algorithms, federation | 13,802 | 387 |
| **Fortune** | Economics — Cool currency, UBI, demurrage, bearer cash, cooperatives | 7,357 | 217 |
| **Bulwark** | Safety — trust layers, reputation, Kids Sphere, child safety protocol | 10,999 | 361 |
| **Jail** | Accountability — trust graphs, graduated response, restorative justice | 6,890 | 225 |

Every crate in this layer depends on X and Crown (you need identity to participate in civilization). None depend on each other — they communicate through Equipment.

### Layer 4: Intelligence

| Crate | What It Does | Depends On |
|-------|-------------|-----------|
| **Advisor** | AI cognition — cognitive loop, thoughts, synapses, pluggable providers | X, Crown, Ideas, Magic, Equipment |
| **Magic** | Rendering — CRDT document state, actions, code projection | X, Ideas, Regalia |
| **Regalia** | Design language — Aura tokens, Arbiter layout, Surge animation, Reign theming | X |
| **Nexus** | Federation — export to 15 formats, import from 7, SMTP bridge | X, Ideas, Magic, Equipment, Regalia |

### Layer 5: World

The network infrastructure that makes everything actually run.

| Crate | What It Does | Lines | Tests |
|-------|-------------|------:|------:|
| **Omnibus** | Node runtime — boots relay server, mDNS discovery, identity, pool in one call | 2,434 | 71 |
| **Tower** | Always-on relay servers — Pharos (gospel-only) and Harbor (community content) | 3,348 | 96 |
| **MagicalIndex** | Search engine — FTS5, BM25 ranking, faceted search, aggregation | 4,235 | 99 |
| **Physical** | Geographic — places, regions, meetups, presence (never stored!), deliveries | 4,818 | 171 |

### Layer 6: Meta

The crates that observe, guide, and remember.

| Crate | What It Does | Lines | Tests |
|-------|-------------|------:|------:|
| **Undercroft** | Observatory — system health, app catalog, device management | 5,298 | 194 |
| **Oracle** | Guidance — onboarding flows, sovereignty tiers, recovery, workflows | 4,395 | 145 |
| **Quest** | Gamification — missions, achievements, XP, cooperative challenges | 11,774 | 455 |
| **Yoke** | History — versions, timelines, ceremonies, contribution graphs | 4,744 | 173 |
| **Zeitgeist** | Discovery — Tower directory, query routing, trends, caching | 3,671 | 121 |

---

## Dependency Graph

The actual Rust crate dependencies (internal only, external deps omitted):

```
X ─────────────────────────────────────────────────────────────────────
 |-- Ideas
 |-- Regalia
 |-- Polity ──── Crown
 |-- Kingdom ─── Crown
 |-- Fortune ─── Crown, Ideas
 |-- Bulwark ─── Crown
 |-- Jail ────── Crown
 +-- Yoke ────── Crown

Crown ─────────────────────────────────────────────────────────────────
 |-- Globe
 +-- (Polity, Kingdom, Fortune, Bulwark, Jail, Yoke via Crown)

Equipment ─────────────────────────────────────────────────────────────
 +-- (zero deps — standalone nervous system)

Ideas ─────────────────────────────────────────────────────────────────
 +-- X

Sentinal ──────────────────────────────────────────────────────────────
 |-- Vault ──── Ideas
 |-- Hall ───── Ideas, Lingo, X
 +-- Lingo

Magic ─────── X, Ideas, Regalia
Advisor ───── X, Crown, Ideas, Magic, Equipment
Nexus ─────── X, Ideas, Magic, Equipment, Regalia

Globe ─────── Crown
 |-- Zeitgeist ─── Crown, MagicalIndex
 |-- Tower ─────── Omnibus, Crown, Sentinal, MagicalIndex
 +-- Omnibus ───── Crown

Physical ──── X, Crown, Bulwark

Undercroft ── Globe, Kingdom, Fortune, Bulwark, Quest
  |-- AppCatalog ──── Crown
  +-- DeviceManager ── Crown, Globe, X

Oracle ────── (zero deps)
Quest ─────── (zero deps)
```

Key observations:
- **Equipment has zero deps.** It's the nervous system — it doesn't need to know what it's connecting.
- **Crown has zero internal deps.** Crypto is injected via traits, not imports.
- **Oracle and Quest have zero deps.** Guidance and gamification are pure logic.
- **Globe only depends on Crown.** The network layer only needs identity to sign events.
- **No circular dependencies.** The graph is a DAG. Equipment's string routing makes this possible.

---

## The FFI Stack

How Rust protocol crates become an API that TypeScript programs can call:

```
+─────────────────────────────────────────────────────────────────+
|  Program (TypeScript)                                           |
|                                                                 |
|  import { crown, vault } from "@omnidea/net"                    |
|  const key = await crown.keyringGeneratePrimary()               |
|                                                                 |
|  Under the hood:                                                |
|    exec("crown.keyring_generate_primary", {})                   |
|      -> window.omninet.run({ steps: [{ op: "crown..." }] })    |
+────────────────────┬────────────────────────────────────────────+
                     |  JSON over bridge (WebIDL or fetch)
                     v
+─────────────────────────────────────────────────────────────────+
|  Zig Orchestrator (libomnidea_orchestrator.a)                   |
|                                                                 |
|  Pipeline executor receives JSON, iterates steps.               |
|  Each step:                                                     |
|    1. Look up "crown.keyring_generate_primary"                  |
|    2. Comptime auto-dispatch finds divi_crown_keyring_generate_ |
|    3. Call the C function with marshalled args                   |
|    4. Run modifiers: Polity -> Bulwark -> [step] -> Yoke -> Que |
|    5. Store result, resolve $step_id.field references           |
|                                                                 |
|  ~119 orch_* lifecycle functions (init, shutdown, identity, etc.)|
|  ~860 pipeline-callable operations (auto-discovered at comptime)|
+────────────────────┬────────────────────────────────────────────+
                     |  C function calls
                     v
+─────────────────────────────────────────────────────────────────+
|  Rust FFI (libdivinity_ffi.a)                                   |
|                                                                 |
|  1,040 extern "C" functions with divi_ prefix.                  |
|  Each wraps a safe Rust API:                                    |
|    - JSON in -> serde_json::from_str -> Rust types              |
|    - Call the actual crate function                              |
|    - Rust types -> serde_json::to_string -> JSON out            |
|    - Opaque pointers for stateful types (Mutex-wrapped)         |
|  Thread-local error storage via divi_last_error().              |
+────────────────────┬────────────────────────────────────────────+
                     |  safe Rust calls
                     v
+─────────────────────────────────────────────────────────────────+
|  29 Rust Crates (the protocol)                                  |
|                                                                 |
|  Pure Rust. No FFI awareness. No C types.                       |
|  Tested independently: 6,700+ tests, zero clippy warnings.     |
|  Each crate has its own Cargo.toml, its own tests, its own     |
|  CLAUDE.md documentation.                                       |
+─────────────────────────────────────────────────────────────────+
```

### How Comptime Auto-Dispatch Works

The Zig orchestrator uses Zig's comptime (compile-time execution) to discover all `divi_*` functions from the C header automatically. No manual registration needed.

1. At compile time, Zig's `@cImport` reads `divinity_ffi.h`
2. `@typeInfo` enumerates every function declaration
3. For each `divi_*` function, the orchestrator generates a dispatch entry
4. The function signature determines the calling pattern (16 patterns total)
5. At runtime, `"crown.keyring_generate_primary"` resolves to `divi_crown_keyring_generate_primary` via simple string transformation

Adding a new `divi_*` function to the Rust FFI automatically makes it available in the pipeline. Zero manual registration, zero code generation.

### Pipeline Modifiers

Every pipeline step runs through mandatory cross-cutting modifiers:

```
Before: Polity (Covenant check) -> Bulwark (permission/safety)
  |
  [Step executes]
  |
After:  Sentinal (no-op*) -> Lingo (no-op*) -> Yoke (provenance) -> Quest (XP)
```

*Sentinal and Lingo are no-ops at the pipeline level by design. Encryption belongs at the storage layer, translation at the content layer.

---

## Communication: How Crates Talk

Equipment's string-routed messaging is the reason Omninet has no circular dependencies. Here's how it works:

### Phone (RPC) — "I need something from you"

```rust
// Crown crate defines a call type
struct GetProfile;
impl PhoneCall for GetProfile {
    const CALL_ID: &'static str = "crown.getProfile";
    type Response = Profile;
}

// Somewhere else, a handler is registered
phone.register(|_req: GetProfile| -> Result<Profile> {
    Ok(current_profile())
});

// Any crate can call it by ID — no import needed
let profile = phone.call::<GetProfile>(GetProfile)?;
```

### Email (Pub/Sub) — "Something happened, FYI"

```rust
// Ideas crate defines an event
struct DocumentChanged { idea_id: Uuid }
impl EmailEvent for DocumentChanged {
    const EMAIL_ID: &'static str = "crdt.documentChanged";
}

// Any number of subscribers
email.subscribe(|event: DocumentChanged| {
    // React to the change
});

// Publisher doesn't know or care who's listening
email.emit(DocumentChanged { idea_id });
```

### Why Strings?

Import-based routing creates dependency cycles. If Crown imported Vault to call it, and Vault imported Crown for identity, you'd have a cycle. String routing eliminates this entirely. Crates define their call/event IDs as constants, and Equipment routes by string matching at runtime. The cost is a HashMap lookup. The benefit is that 29 crates compose without ever importing each other.

---

## Key Design Principles

### Extend, Never Break

New features are always additive. Five architectural firewalls enforce this:

1. **Digit types are strings.** Adding `commerce.product` never touches `text.paragraph`.
2. **Traits with registries.** New `DigitRenderer` implementations register alongside existing ones.
3. **FFI is JSON + opaque pointers.** New `divi_*` functions are added next to old ones, never replace them.
4. **Equipment uses strings.** No recompilation cascades when a new module registers.
5. **Globe kinds are numbers.** Relays forward unknown kinds without crashing. Old nodes talk to new nodes.

### The Covenant

The Covenant is the project's governing framework. Technical decisions align with three principles:

1. **Dignity** — worth that cannot be taken, traded, or measured.
2. **Sovereignty** — the right to choose, refuse, and reshape.
3. **Consent** — voluntary, informed, continuous, and revocable.

About 70% of the Covenant's articles are encoded as type constraints in Polity.

### Architecture Over Policy

Every defense is structural, not aspirational. Type constraints instead of Terms of Service. Behavioral drift detection watches the shape of actions, not their content. Financial pattern detection catches structuring without reading transaction details. Kids Sphere requires physical proximity for family bonds — not an admin toggle.

---

## Crate Reference

| Crate | Layer | Lines | Tests | Internal Deps | One-Liner |
|-------|:-----:|------:|------:|---------------|-----------|
| X | 0 | 6,900 | 279 | none | Value types, CRDT engine, geometry, color, geo |
| Equipment | 1 | 7,291 | 255 | none | Phone RPC, Email pub/sub, Contacts, Pager, Communicator |
| Ideas | 1 | 8,852 | 240 | X | .idea format: digits, headers, schemas, domains |
| Crown | 2 | 6,901 | 223 | none | secp256k1 identity, keyring, soul, social graph |
| Sentinal | 2 | 2,902 | 125 | none | AES-256-GCM, PBKDF2, HKDF, BIP-39, obfuscation |
| Vault | 2 | 3,256 | 96 | Sentinal, Ideas | SQLCipher manifest, lock/unlock, collectives |
| Hall | 2 | 1,706 | 51 | Ideas, Lingo, Sentinal, X | Encrypted .idea package I/O, asset pipeline |
| Globe | 2 | 21,143 | 715 | Crown | ORP relay protocol, events, filters, server, privacy |
| Lingo | 2 | 7,177 | 253 | Sentinal | Babel obfuscation, translation, formula engine |
| Polity | 3 | 9,674 | 279 | X, Crown | Constitutional guard: rights, duties, protections |
| Kingdom | 3 | 13,802 | 387 | X, Crown | Communities, charters, proposals, 6 voting algorithms |
| Fortune | 3 | 7,357 | 217 | X, Ideas, Crown | Cool currency, UBI, demurrage, cash, cooperatives |
| Bulwark | 3 | 10,999 | 361 | X, Crown | Trust layers, reputation, Kids Sphere, child safety |
| Jail | 3 | 6,890 | 225 | X, Crown | Trust graphs, graduated response, restorative justice |
| Regalia | 4 | 7,437 | 271 | X | Aura tokens, Arbiter layout, Surge animation, theming |
| Magic | 4 | 10,510 | 301 | X, Ideas, Regalia | CRDT documents, actions, renderers, code projection |
| Advisor | 4 | 13,163 | 431 | X, Crown, Ideas, Magic, Equipment | AI cognitive loop, thoughts, synapses, skills |
| Nexus | 4 | 9,301 | 248 | X, Ideas, Magic, Equipment, Regalia | 15 exporters, 7 importers, SMTP bridge |
| Oracle | 6 | 4,395 | 145 | none | Onboarding flows, sovereignty tiers, workflows |
| Quest | 6 | 11,774 | 455 | none | Missions, achievements, XP, challenges, raids |
| Yoke | 6 | 4,744 | 173 | X, Crown | Versions, timelines, ceremonies, provenance |
| Zeitgeist | 6 | 3,671 | 121 | Globe, Crown, MagicalIndex | Tower directory, query routing, trends, caching |
| Omnibus | 5 | 2,434 | 71 | Crown, Globe | Node runtime: relay + mDNS + identity + pool |
| Tower | 5 | 3,348 | 96 | Omnibus, Crown, Sentinal, Globe, MagicalIndex | Pharos (gospel) and Harbor (community) relay modes |
| MagicalIndex | 5 | 4,235 | 99 | Globe | FTS5 search, BM25, faceted search, aggregation |
| Physical | 5 | 4,818 | 171 | X, Crown, Bulwark | Places, regions, meetups, presence, deliveries |
| Undercroft | 6 | 2,739 | 112 | Globe, Kingdom, Fortune, Bulwark, Quest | System health, economic vitals, network observatory |
| AppCatalog | 6 | 1,392 | 47 | Crown | App manifest registry, install lifecycle |
| DeviceManager | 6 | 1,167 | 35 | Crown, Globe, X | Multi-device pairing, fleet management, sync |
| _Codecs | -- | 373 | 12 | none | Jitter buffer, audio codec utilities |
| Divinity/ffi | -- | 32,979 | 82 | *(all)* | 1,040 extern "C" functions wrapping every crate |
| Divinity/Web | -- | 1,441 | 28 | Equipment, Oracle | WASM bindings for Equipment + PWA hardening |

**Total: ~235,000 lines of Rust, 6,700+ tests**

---

## Directory Structure

```
Omninet/
|-- Advisor/          # AI cognition (Layer 4)
|-- Bulwark/          # Safety (Layer 3)
|-- Crown/            # Identity (Layer 2)
|-- Divinity/         # Platform bridges
|   |-- ffi/          #   Rust FFI (1,040 C functions)
|   |-- Orchestrator/ #   Zig pipeline executor
|   |-- Apple/        #   Swift wrappers + CrystalKit
|   |-- Desktop/      #   C++ wrappers
|   |-- Android/      #   Kotlin/JNI wrappers
|   |-- Web/          #   WASM/wasm-bindgen
|   |-- Linux/        #   (placeholder)
|   +-- Microsoft/    #   (placeholder)
|-- Equipment/        # Communication (Layer 1)
|-- Fortune/          # Economics (Layer 3)
|-- Globe/            # Networking (Layer 2)
|-- Hall/             # File I/O (Layer 2)
|-- Ideas/            # Content format (Layer 1)
|-- Jail/             # Accountability (Layer 3)
|-- Kingdom/          # Governance (Layer 3)
|-- Lingo/            # Language (Layer 2)
|-- Magic/            # Rendering (Layer 4)
|-- Nexus/            # Federation (Layer 4)
|-- Oracle/           # Guidance (Layer 6)
|-- Polity/           # Constitution (Layer 3)
|-- Quest/            # Gamification (Layer 6)
|-- Regalia/          # Design language (Layer 4)
|-- Sentinal/         # Encryption (Layer 2)
|-- Target/           # Cargo build output
|-- Undercroft/       # Observatory (Layer 6)
|   |-- AppCatalog/   #   Extension lifecycle
|   +-- DeviceManager/#   Fleet + pairing + sync
|-- Vault/            # Encrypted storage (Layer 2)
|-- World/            # Digital + Physical (Layer 5)
|   |-- Digital/
|   |   |-- Omnibus/  #   Node runtime
|   |   |-- Tower/    #   Relay servers
|   |   +-- MagicalIndex/ # Search engine
|   +-- Physical/     #   Geographic + real-world
|-- X/                # Shared utilities (Layer 0)
|-- Yoke/             # History (Layer 6)
|-- Zeitgeist/        # Discovery (Layer 6)
|-- _Codecs/          # Audio/video utilities
|-- _SDK/             # Pipeline operation docs
|-- _Specs/           # Protocol specifications
|-- _Tests/           # Integration tests
|-- docs/             # This file lives here
|-- Cargo.toml        # Workspace manifest
+-- README.md         # Start here
```

---

## What's Next

The protocol layer is complete. The focus is now on building [Omny](https://github.com/neonpixy/omny), the sovereign browser. Omny uses Throne (a Tauri 2.x shell with WKWebView) for the desktop app, Castle as the contract engine mediating all program communication, and Chancellor as the state authority. Programs are manifest-driven Solid.js apps in the [Apps](https://github.com/neonpixy/apps) repo, communicating via the `@omnidea/net` SDK over the `window.omninet` bridge.

For protocol specifications, see [`_Specs/`](../_Specs/).
For SDK reference, see [`_SDK/`](../_SDK/).
