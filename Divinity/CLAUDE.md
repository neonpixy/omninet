# Divinity — Platform Interface

The divine realm. Divinity is how Omninet meets each platform — the bridge between the cross-platform Rust core and each platform's native capabilities. FFI bindings, GPU rendering, platform-specific APIs. To sit on the Throne is to emanate Divinity.

## Two Responsibilities

1. **Platform Bridges** — FFI bindings that expose the Rust core to each platform's native language. Swift on Apple, C/C++ on Linux/Windows, Kotlin on Android, WebAssembly on Web. The connection layer.
2. **Platform Rendering** — GPU-accelerated visual rendering using each platform's native API. Glass, materials, shaders, backdrop management, light tracking. The visual layer.

## What Exists

### `ffi/` — Rust FFI Crate (divinity-ffi)

Static library (`libdivinity_ffi.a`) with `divi_` prefix for all extern "C" functions. 1,040 functions across 37 source files. Depends on the entire Omninet crate ecosystem.

**FFI Modules (37 source files):**

| Module | What It Exposes | Functions |
|--------|----------------|-----------|
| `phone_ffi.rs` | Equipment Phone | ~20 |
| `email_ffi.rs` | Equipment Email | ~15 |
| `contacts_ffi.rs` | Equipment Contacts | ~15 |
| `pager_ffi.rs` | Equipment Pager | ~10 |
| `crown_ffi.rs` | Crown (identity, keyring, soul, blinding, soul encryption) | 35 |
| `globe_ffi.rs` | Globe (events, filters, pool, privacy) | 19 |
| `sentinal_ffi.rs` | Sentinal (encryption, onion, AAD, padding, soul key) | 20 |
| `bulwark_ffi.rs` | Bulwark (safety, trust, permissions, consent) | 31 |
| `kingdom_ffi.rs` | Kingdom (governance, charter, assembly) | 43 |
| `fortune_ffi.rs` | Fortune (economics, ledger, treasury) | 28 |
| `jail_ffi.rs` | Jail (accountability, trust graph) | 15 |
| `lingo_ffi.rs` | Lingo (translation, Babel) | ~5 |
| `regalia_ffi.rs` | Regalia (design tokens, themes, WCAG) | 19 |
| `magic_ffi.rs` | Magic (document, canvas, renderers, tools, history) | 93 |
| `ideas_ffi.rs` | Ideas (digits, packages, schemas, domains) | 68 |
| `vault_ffi.rs` | Vault (lock/unlock, collectives, soul key) | 34 |
| `formula_ffi.rs` | Formula engine (evaluator, registry, deps) | 22 |
| `hall_ffi.rs` | Hall (file I/O) | 11 |
| `yoke_ffi.rs` | Yoke (history, provenance, graph) | 62 |
| `commerce_ffi.rs` | Fortune commerce (products, cart, orders) | 39 |
| `physical_ffi.rs` | World/Physical (places, regions, rendezvous) | 42 |
| `oracle_ffi.rs` | Oracle (workflows, hints, recovery, sovereignty tiers) | 39 |
| `advisor_ffi.rs` | Advisor (cognitive loop, store, router, skills) | 54 |
| `polity_ffi.rs` | Polity (rights, duties, protections, consent) | 85 |
| `nexus_ffi.rs` | Nexus (export, import, bridge registries) | 20 |
| `undercroft_ffi.rs` | Undercroft (health aggregation, privacy health) | 16 |
| `appcatalog_ffi.rs` | AppCatalog (app lifecycle) | 17 |
| `device_ffi.rs` | DeviceManager (pairing, fleet, sync) | 26 |
| `zeitgeist_ffi.rs` | Zeitgeist (directory, router, cache, trends) | 32 |
| `omnibus_ffi.rs` | Omnibus (node runtime) | 34 |
| `runtime_ffi.rs` | DiviRuntime (shared tokio runtime) | 2 |
| `server_ffi.rs` | Globe RelayServer | 6 |
| `discovery_ffi.rs` | Globe local discovery | 5 |
| `pulse_ffi.rs` | Pulse dev tool helpers | ~5 |
| `helpers.rs` | divi_last_error, divi_free_string, divi_free_bytes | 3 |
| `lib.rs` | Module declarations, thread-local error | — |

**oracle_ffi.rs updates (R3E Sovereignty Tiers):**
- `divi_oracle_tier_defaults(tier_json)` -- returns JSON `TierDefaults` for a `SovereigntyTier`.
- `divi_oracle_tier_defaults_all()` -- returns JSON array of all tier defaults (one per tier).
- `divi_oracle_feature_visibility(tier_json)` -- returns JSON `FeatureVisibility` for a tier.
- `divi_oracle_disclosure_tracker_signal_counts` now returns `{"steward":<n>,"architect":<n>}` keys (was previously different key names).

**Dependencies:** equipment, ideas, x, sentinal, vault, hall, crown, globe, lingo, polity, kingdom, fortune, bulwark, jail, regalia, magic, advisor, nexus, oracle, zeitgeist, undercroft, app-catalog, device-manager, omnibus, physical, yoke, magical-index. Plus serde, tokio, uuid, chrono, hex, url, tempfile, cbindgen.

**Patterns:**
- Stateful types -> opaque pointers (`*mut T`), create with `_new`, destroy with `_free`
- Data types -> JSON strings (everything is `Serialize + Deserialize`)
- Raw bytes -> pointer + length
- Errors -> return `i32` (0=success, -1=error), details via `divi_last_error()`
- Callbacks -> `extern "C"` function pointers + `*mut c_void` context

### `Apple/` — Swift Package (OmnideaCore + CrystalKit)

Two library products. swift-tools-version 6.2, macOS 26 / iOS 26.

**OmnideaCore** — Swift wrappers for Rust FFI:
- Phone.swift, Email.swift, Contacts.swift, Pager.swift (Equipment)
- Crown.swift, Sentinal.swift, Bulwark.swift, Kingdom.swift, Fortune.swift, Jail.swift (identity + civilization)
- Globe.swift, Discovery.swift, NativeDiscovery.swift (networking + Apple-native Bonjour)
- Omnibus.swift (node runtime)
- Regalia.swift, Babel.swift (design + language)
- Runtime.swift (DiviRuntime wrapper)
- PulseDemo.swift (dev tool bridge)
- OmnideaError.swift (error handling)

**CrystalKit** — Metal rendering library. Standalone, no dependency on OmnideaCore or FFI:
- **Bedrock/** — CrystalBackdropProvider protocol, CrystalImageBackdropProvider (SwiftUI->CGImage->Metal), BackdropCapture
- **Confluence/** — CrystalGooView, CrystalGooContainer (SDF merging via smooth-minimum)
- **Facet/** — CrystalGlassStyle, CrystalGlassModifier, CrystalGlassRole, CrystalStylesheet, GlassVariant
- **Gleam/** — CrystalHoverTracker, CrystalTiltTracker, OneEuroFilter (signal smoothing), LuminanceMaskView
- **Holodeck/** — GlassRenderer, GlassShader, IrisRenderer, IrisShader, BlurRenderer, ResonanceTintCache, GlassBackgroundCache
- **Iris/** — IrisStyle, IrisModifier, IrisStylesheet (opaque/metallic surfaces)
- **Lapidary/** — ShapeDescriptor, SDFTextureGenerator, ColorUtils, CodableColor, ShaderLibraryCache
- **Material/** — MaterialRenderer protocol, MaterialRepresentable, MaterialMetalView, MaterialUIView, MaterialStyle
- **Resonance/** — CrystalInteractionEffect, CrystalInteractionBuiltins, CrystalInteractionEngine
- **Setting/** — CrystalGlassUIView (iOS), CrystalGlassNSView (macOS), CrystalGlassRepresentable

### `Desktop/` — C++ Header-Only Wrapper

C++17, namespace `divinity`, links `libdivinity_ffi.a`. CMakeLists.txt build. RAII via `std::unique_ptr` with custom deleters, callback trampolines via `std::function`.

Headers: `divinity.hpp` (umbrella), `error.hpp`, `phone.hpp`, `email.hpp`, `contacts.hpp`, `pager.hpp`.

### `Android/` — Kotlin Wrapper with JNI Bridge

CMakeLists.txt for NDK. JNI bridge in C (`jni_bridge.c`). Kotlin classes implement `AutoCloseable`, store pointers as `Long`. Functional interfaces for callbacks: `PhoneHandler`, `EmailHandler`, `ShutdownCallback`.

Kotlin classes: `OmnideaError.kt`, `Phone.kt`, `Email.kt`, `Contacts.kt`, `Pager.kt`.

### `Web/` — WASM Crate (divinity-web)

Separate Rust crate wrapping Equipment directly via `#[wasm_bindgen]` — not through C FFI. Auto-generates TypeScript definitions via `wasm-pack build`. Uses `JsFnWrapper` with `unsafe impl Send + Sync` (safe because WASM is single-threaded). Workspace member.

Source files: `lib.rs`, `phone.rs`, `email.rs`, `contacts.rs`, `pager.rs`, `pwa.rs`.

#### PWA Hardening (`pwa.rs`) — R5C

Progressive Web App resilience for browser-based access. Defines capability flags, tier-based capability matrices, service worker strategies, and offline cache management. Depends on `oracle::SovereigntyTier`.

**WebCapabilities** -- Bitflag newtype over `u32` (no external `bitflags` dep). 6 flags: RENDERING (Magic -> DOM), EDITING (Throne creation), VAULT (IndexedDB-encrypted storage), NETWORK (Globe WebSocket), OFFLINE (service worker + cache), CRYPTO (WebCrypto API for Sentinal). Full BitOr/BitAnd/Not operator support.

**WebCapabilityMatrix** -- Maps sovereignty tiers to capability sets. Default (R5C spec):

| Tier | Capabilities |
|------|-------------|
| Sheltered | Rendering, Vault, Network |
| Citizen | Rendering, Vault, Network |
| Steward | + Editing, Offline |
| Architect | + Crypto (all 6) |

`validate_monotonic()` ensures higher tiers never lose capabilities that lower tiers have.

**ServiceWorkerStrategy** -- 4 strategies, tier-gated:
- CacheIdea (all tiers) -- cache viewed `.idea` files for offline access
- QueuePublish (Steward+) -- queue publishes for sync when online
- PreCacheThemes (Steward+) -- pre-cache Regalia themes for offline rendering
- PreCacheModel (Architect) -- pre-cache Advisor local model for offline AI

**PwaConfig** -- Session configuration: tier, capabilities, strategies, offline_enabled. `for_tier()` uses default matrix. `to_json()` / `from_json()`.

**OfflineCache** -- Rust-side bookkeeping for browser Cache API. LRU eviction of non-dirty entries. Dirty entries (local modifications) are never evicted. `put()`, `get()` (updates access time), `peek()`, `mark_dirty()`, `mark_clean()`, `evict_clean()`. Default max 1000 entries.

**PublishQueue** -- FIFO queue of `.idea` publishes waiting for connectivity. `enqueue()`, `dequeue()`, `drain_all()`, `remove_for_idea()`, `to_json()` / `from_json()`.

### `Linux/` — Future

Currently contains only `Media/CLAUDE.md` (plan doc). Future: Vulkan rendering, PipeWire/PulseAudio audio, platform integration.

### `Microsoft/` — Future

Currently contains only `Media/CLAUDE.md` (plan doc). Future: DirectX rendering, WASAPI audio, platform integration.

## Subdirectories

- `ffi/` — Rust FFI crate (shared C ABI for all platforms)
- `Orchestrator/` — Zig composition layer. 16 modules, ~119 exported functions composing the 1,040 `divi_*` C functions into smart app-level operations. Thread-safe (RwLock + per-module Mutex). Builds to `libomnidea_orchestrator.a`. Every platform links against it — the Omninet SDK. See `Orchestrator/CLAUDE.md` for full reference.
- `Apple/` — Swift + Metal (primary). FFI bridge + Metal rendering pipeline.
- `Desktop/` — C++ header-only wrapper (Linux + Windows). Equipment wrappers.
- `Android/` — Kotlin wrapper with JNI C bridge. Equipment wrappers.
- `Web/` — Rust WASM crate with wasm-bindgen. Equipment wrappers.
- `Linux/` — Future (Vulkan rendering, platform integration)
- `Microsoft/` — Future (DirectX rendering, platform integration)

## Covenant Alignment

**Dignity** — beauty is not luxury. Every surface deserves visual care. Glass communicates transparency. Every platform deserves a native experience, not a lowest-common-denominator wrapper.
