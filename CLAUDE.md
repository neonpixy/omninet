> **[Omnidea](https://github.com/neonpixy/omnidea)** / **[Omninet](https://github.com/neonpixy/omninet)** · [README](README.md) · [WIRING.md](https://github.com/neonpixy/omnidea/blob/main/WIRING.md)

# Omninet

The protocol layer of Omnidea -- a sovereign internet. 26 building blocks (A through Z) for creation, commerce, socialization, and communication. Built on protocols, not frameworks. Governed by the Covenant.

---

## Vision

Omninet is a set of protocols — like the internet — with native implementations per platform. .idea is the universal content format (like HTML). Pact is the communication protocol (like HTTP). Regalia is the design language (like CSS). Three apps — Throne (create), Scry (experience), Omny (govern) — are platform-native via Divinity. Human and AI collaborators work through the same abstractions.

Users own their identity, data, and history. The architecture enforces this structurally, not through policy alone.

---

## The ABCs

26 directories, one per letter.

| | Name | What It Is |
|---|---|---|
| **A** | **Advisor** | AI cognition |
| **B** | **Bulwark** | Safety and protection |
| **C** | **Crown** | Identity and self |
| **D** | **Divinity** | Platform interface (FFI + rendering) |
| **E** | **Equipment** | Communication (Pact) |
| **F** | **Fortune** | Economics |
| **G** | **Globe** | Networking (ORP) |
| **H** | **Hall** | File I/O |
| **I** | **Ideas** | Universal content format (.idea) |
| **J** | **Jail** | Verification and accountability |
| **K** | **Kingdom** | Community governance |
| **L** | **Lingo** | Language and translation |
| **M** | **Magic** | Rendering and code translation |
| **N** | **Nexus** | Federation and interop |
| **O** | **Oracle** | Guidance and onboarding |
| **P** | **Polity** | Rights enforcement and consensus |
| **Q** | **Quest** | Gamification and progression |
| **R** | **Regalia** | Design language |
| **S** | **Sentinal** | Encryption |
| **T** | **Target** | Cargo build output |
| **U** | **Undercroft** | System health and observatory |
| **V** | **Vault** | Encrypted storage |
| **W** | **World** | Digital and physical worlds |
| **X** | **X** | Shared utilities |
| **Y** | **Yoke** | History and provenance |
| **Z** | **Zeitgeist** | Discovery and culture |

---

## The Apps

Three apps. Each has programs accessed via a collapsible sidebar. See `_Plans/Master Vision.md` and individual blueprints for full specs.

| App | What It Is | Programs |
|-----|-----------|----------|
| **Throne** | No-code creation platform. Creative suite + CMS + business platform + no-code app builder. | Abacus (sheets), Courier (mail), Library (idea/asset management), Podium (presentations), Quill (documents), Studio (design + Genius), Tome (notes) |
| **Scry** | Your life on the network. Browse, shop, socialize, manage account & finances. | Browser (.idea + legacy HTML), Cart (universal shopping), Net (social), Satchel (account + finances) |
| **Omny** | Admin & node management. Governance, devices, node console, network health. | Home (network dashboard), HQ (community governance), Hub (app & device management), Pulse (node console) |

---

## How They Relate

**The spine (zero/minimal dependencies):**
- **X** — pure utilities, imported by everything
- **Equipment** — Pact communication (Phone, Email, Contacts, Pager). Zero deps. String-based routing eliminates import cycles.
- **Ideas** — the universal .idea format everything operates on

**The body (core infrastructure):**
- **Sentinal** encrypts. **Vault** stores. **Hall** reads and writes.
- **Crown** is who you are. **Globe** is how you reach others.
- **Lingo** translates across languages and module boundaries.

**The higher-level systems:**
- **Fortune** runs the economy. **Kingdom** governs communities. **Polity** enforces the Covenant.
- **Bulwark** protects the vulnerable. **Jail** handles accountability.

**Intelligence and creation:**
- **Advisor** handles AI cognition. CognitiveProvider trait for pluggable backends (Claude API, Apple Intelligence, Ollama, MLX). AI companions are first-class Omninet participants -- one per person via sponsorship, bound by the Covenant like everyone else.
- **Magic** renders (NeonPixy: Triple-I architecture) and projects code — design = code via live projection, Regalia's Sanctum layouts map 1:1 to platform primitives (Rank→HStack/flex-row, Column→VStack/flex-column, etc.).
- **Divinity** is the platform interface -- FFI bridges to the Rust core + native rendering + material system (glass, iris, SDF shapes) + platform-specific push notifications (APNs, FCM). CrystalKit as Apple reference implementation, portable math in Rust (70% portable, 30% platform GPU API).
- **Regalia** is the design vocabulary everything wears. Aura tokens, Arbiter layout, Surge animation, Reign theming.

**Digital and physical:**
- **World** splits into Digital and Physical.
- **World/Digital** houses the Omninet infrastructure: **Omnibus** (the node runtime every app embeds), **Tower** (always-on network nodes), **MagicalIndex** (demand-driven search).
- **World/Physical** bridges geographic reality — Place, Region, Rendezvous, Presence, Lantern, Handoff, OmniTag, Caravan. Pull-based location (you declare where you are, the network doesn't track you). Presence is never persisted. Handoff contains zero location data.

**Meta-layer:**
- **Undercroft** is the one crate that depends on everything above it. System health, network topology, economic vitals. Observes, never controls. HQ is the app that reads from the Undercroft.

**Surface (experience):**
- **Throne** is the creative suite (7 programs). **Scry** is the browser and social hub (4 programs). **Omny** is admin and governance (4 programs).
- **Oracle** guides new participants. **Quest** makes engagement meaningful.
- **Zeitgeist** shows the pulse. **Yoke** remembers the past.
- **Nexus** bridges to the outside world — export to legacy formats, SMTP, platform code generation.

---

## Languages

- **Swift** — Apple platforms. SwiftUI, Metal. Primary development language.
- **Rust** — Cross-platform core. Encryption, .idea parsing, Pact engine, CRDT, ORP. Compiles to macOS, Linux, Windows, WebAssembly. Called from Swift via C FFI.

Divinity demonstrates the per-platform pattern: `ffi/` (1,040 C functions), `Orchestrator/` (Zig, 109 `orch_*` compositions + comptime auto-dispatch pipeline — the SDK), `Apple/` (Swift FFI + Metal), `Desktop/` (C++), `Android/` (Kotlin/JNI), `Web/` (WASM).

---

## The Covenant

Every technical decision answers to three principles:

1. **Dignity** — worth that cannot be taken, traded, or measured.
2. **Sovereignty** — the right to choose, refuse, and reshape.
3. **Consent** — voluntary, informed, continuous, and revocable.

The Covenant is the project's governing framework. See `/Developer/Covenant/` for the full text (14 documents).

---

## Principles

- **Equipment is the nervous system.** All inter-module communication goes through Pact (Phone/Email/Contacts/Pager).
- **Regalia is the design language.** No hardcoded colors, spacing, or typography.
- **Ideas is the universal format.** Everything is an .idea.
- **Protocols over implementations.** Design specs that any language can implement.
- **Apps are views into the protocol.** The platform is the product. Throne, Scry, and Omny are lenses.
- **The Covenant governs.** Dignity, Sovereignty, and Consent are architectural constraints.
- **Build what you need, when you need it.** No speculative architecture.
- **Never `print()`.** Use structured logging.
- **Everything is Sendable and Codable.** Design for concurrency and persistence from day one.
- **Every module and submodule has tests.** Write them alongside the code, run them before calling it done. No untested modules ship.
- **Clippy includes tests.** Always run `cargo clippy --workspace --tests`, not just `cargo clippy --workspace`. Test code gets the same lint discipline as library code.
- **Extend, never break.** New features add types and traits. They never modify existing public APIs. Every existing test continues to pass. Five architectural firewalls enforce this:
  1. **Digit type is a String, not an enum.** Adding a digit type never touches existing code.
  2. **Traits, not concrete types.** New implementations register with registries. Existing implementations are untouched. FallbackRenderer handles unknowns gracefully.
  3. **FFI is JSON + opaque pointers.** New FFI functions are added alongside existing ones, never replace them. `#[serde(default)]` on new optional fields keeps old JSON compatible.
  4. **Equipment uses strings, not imports.** No crate imports another to communicate. String-keyed routing eliminates recompilation cascades. `call_if_available()` returns `Ok(None)` for missing handlers.
  5. **Globe kinds are just numbers.** Relays store and forward unknown kinds without crashing. Old nodes talk to new nodes seamlessly.
  - **Never** change an existing FFI function signature — add a new one. Remove the old one only after confirming zero callers across all platforms.
  - **Never** remove a field from a serialized struct without migration — add `#[serde(default)]`, stop writing it, remove after all old data has been migrated.
  - **Never** change OmniEvent's wire format — add new message types alongside. Deprecate old kinds only after a version bump and grace period.
  - **Never** change Vault's manifest schema destructively — add columns, migrate data, then drop old columns.
  - Internal Rust code has no such constraints — delete dead code freely. Clippy enforces this.
- **Architecture over policy.** Defenses are structural -- type constraints, not Terms of Service.
- **The Covenant is encoded in Polity.** ~70% of the 10 Covenant Parts are expressed as type constraints in Polity's `covenant_code.rs`.
- **Detect the shape, not the content.** Behavioral drift, power indices, provenance scores, and financial patterns detect the SHAPE of problems without reading the CONTENT of actions.
