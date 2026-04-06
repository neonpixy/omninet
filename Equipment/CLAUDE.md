# Equipment — Communication

The nervous system. All inter-module communication flows through Equipment. No module ever imports another module directly — they speak through typed, string-routed messages with serialized boundaries to eliminate import cycles entirely.

## Source Layout

```
Equipment/
├── Cargo.toml
├── src/
│   ├── lib.rs                ← module declarations + re-exports
│   ├── error.rs              ← PhoneError, CommunicatorError, ContactsError
│   ├── catalog.rs            ← CallDescriptor, EventDescriptor, ChannelDescriptor, ModuleCatalog, MessageEdge, MessageTopology
│   ├── notification.rs       ← Notification, NotificationPriority, NotificationDelivery, PagerState
│   ├── phone.rs              ← Phone (RPC) + PhoneCall trait
│   ├── email.rs              ← Email (pub/sub) + EmailEvent trait
│   ├── contacts.rs           ← Contacts (communal module registry + catalog queries) + ModuleInfo + ModuleType
│   ├── pager.rs              ← Pager (notification queue)
│   ├── communicator.rs       ← Communicator (real-time session management) + CommunicatorChannel trait + CommunicatorSession + SessionStatus
│   ├── communicator_types.rs ← CommunicatorOffer, CommunicatorAnswer, CommunicatorEnd, OfferEncryption, WrappedKey, EndReason
│   ├── programs.rs           ← Standard call/event/channel IDs for Throne's seven programs
│   ├── binding_wire.rs       ← DataSubscription, DataUpdate, BindingManager for live data bindings
│   ├── program_registry.rs   ← ProgramRegistration builder + pre-built registrations for all 7 Throne programs
│   ├── federation_scope.rs   ← FederationScope — controls module visibility across federated communities
│   ├── mail.rs               ← Mail system (async message delivery)
│   ├── mail_types.rs         ← Mail message types and delivery state
│   └── presence.rs           ← Cursor and presence data types for collaborative editing
└── tests/
    └── integration.rs
```

## Five Independent Actors (all sync, Mutex-based, no tokio)

- **Phone** — Request/response RPC. `PhoneCall` trait with `CALL_ID` + `Response` type. Typed and raw register/call. Arc-clone-then-release pattern prevents deadlocks on reentrant calls.
- **Email** — Fire-and-forget pub/sub. `EmailEvent` trait with `EMAIL_ID`. Multiple subscribers per event. UUID-based unsubscription. Errors silently ignored.
- **Contacts** — Communal module registry with catalog. All modules are sovereign peers. `ModuleInfo` with optional `depends_on` (declared dependencies) and optional `catalog` (self-describing message-passing capabilities). Shutdown respects the dependency graph — dependents before dependencies. `FnOnce` shutdown callbacks. Catalog queries: `who_handles()`, `who_emits()`, `who_subscribes()`, `topology()`, `calls_for()`, `events_emitted_by()`, `all_calls()`, `all_events()`. Runtime catalog updates via `update_catalog()`.
- **Pager** — Pure notification queue (state machine). NO dependency on Email. Notification with builder pattern, priority/delivery enums. Mark read, dismiss, badge count, prune expired, export/restore state.
- **Communicator** — Real-time communication session management. `CommunicatorChannel` trait with `CHANNEL_ID` routing key (convention: `"domain.type"`, e.g. `"voice.call"`, `"music.stream"`). Session state machine: `Offering -> Active -> Ended/Failed`. Typed and raw handler registration. `offer()`, `accept()`, `end()`, `fail()`, `deliver()` for session lifecycle. Active session queries, channel filtering, handler management, prune expired sessions. Separate locks for sessions and handlers — no deadlock risk.

### Communicator Wire Types (`communicator_types.rs`)

Data types that flow over Globe as ORP events for session signaling:

- **CommunicatorOffer** — session_id, channel_id, initiator, participants, optional OfferEncryption (kind 5100)
- **CommunicatorAnswer** — session_id, responder, accepted (kind 5101)
- **CommunicatorEnd** — session_id, reason (kind 5102)
- **OfferEncryption** — ephemeral_public_key + wrapped_session_keys (per-participant ECDH key exchange)
- **WrappedKey** — recipient + encrypted_key
- **EndReason** — Normal, Declined, Timeout, Error(String)

### Catalog Types (`catalog.rs`)

- **CallDescriptor** — Phone call a module handles
- **EventDescriptor** — Email event a module emits/subscribes to
- **ChannelDescriptor** — Communicator channel a module supports (channel_id, description, group_support, max_participants)
- **ModuleCatalog** — a module's complete self-description (calls handled, events emitted/subscribed, channels supported)
- **MessageEdge** + **EdgeType** + **MessageTopology** — message-passing graph

## Program Wiring Layer (Phase 2F)

Three modules that turn Throne from "seven separate programs" into "one unified app where everything talks to everything":

### Standard IDs (`programs.rs`)

Well-known constants for inter-program routing. Three sub-modules:

- **`call_ids`** — 17 Phone RPC call IDs across 7 programs (e.g., `STUDIO_CREATE_FRAME`, `ABACUS_GET_ROWS`, `COURIER_SEND`). Convention: `"program.action"`.
- **`event_ids`** — 11 Email pub/sub event IDs (e.g., `ABACUS_DATA_CHANGED`, `LIBRARY_PUBLISHED`, `CONTENT_SAVED`). Convention: `"program.eventName"` or `"domain.eventName"`.
- **`channel_ids`** — 3 Communicator channel IDs for real-time collaboration (`COLLABORATION_EDIT`, `COLLABORATION_CURSOR`, `COLLABORATION_PRESENCE`). Convention: `"domain.type"`.

### Data Binding Wire Types (`binding_wire.rs`)

Types for live data binding subscriptions (e.g., a Studio design element bound to an Abacus cell):

- **`DataSubscription`** — subscriber/source module, .idea reference, source path, created timestamp. Serde-ready.
- **`DataUpdate`** — notification when source data changes, carrying the new value as JSON.
- **`BindingManager`** — plain struct (caller-owned, no internal locking) tracking active subscriptions. Lookup by source reference or subscriber module. Bulk unsubscribe by module or source.

### Program Registration (`program_registry.rs`)

Convenience builder for declaring a program's communication capabilities:

- **`ProgramRegistration`** — builder with `handles_call()`, `emits_event()`, `subscribes_to()`, `supports_channel()`. Converts to `ModuleCatalog` via `to_catalog()`.
- **Pre-built registrations** — `studio_registration()`, `abacus_registration()`, `quill_registration()`, `library_registration()`, `courier_registration()`, `podium_registration()`, `tome_registration()`. Each populates the standard IDs that program handles/emits/subscribes.

### Wiring Graph (who talks to whom)

| Program | Handles Calls | Emits Events | Subscribes To | Channels |
|---------|--------------|-------------|---------------|----------|
| Studio | createFrame, setFill, export | selectionChanged, designUpdated, content.saved/modified | abacus.dataChanged, library.updated | edit, cursor, presence |
| Abacus | getRows, setCell, createView | dataChanged, rowAdded, rowDeleted, content.saved/modified | — | edit, cursor, presence |
| Quill | getContent, insertBlock | content.saved/modified | abacus.dataChanged, library.updated | edit, cursor, presence |
| Library | publish, listIdeas, tag | published, updated, content.saved | content.saved | — |
| Courier | send, compose | sent, received | abacus.rowAdded, library.published | — |
| Podium | addSlide, setTransition | content.saved/modified | abacus.dataChanged, studio.designUpdated, library.updated | edit, cursor, presence |
| Tome | createNote, search | content.saved/modified | — | — |

## Key Design Decisions

- **Zero dependencies** on X or Ideas. String-routed, JSON-serialized messages.
- **Sync, not async** — designed for Swift FFI. No tokio.
- **No global singletons** — plain structs the caller owns.
- **Pager is independent** — no Email dependency. The Swift/UI layer wires up broadcasts if needed.
- **Extension/renderer registry excluded** — `IdeaExtension` and `DigitRenderer` registration belongs in Ideas/Magic/Divinity, not communication.
- **Catalog lives in Contacts** — modules self-describe their capabilities via `ModuleCatalog` on `ModuleInfo`. Catalog is optional and runtime-updateable.
- **Topology is computed** — `contacts.topology()` builds a `MessageTopology` from registered catalogs. Event edges are complete (emitter -> subscriber). Call edges show handler side only.
- **Communicator is transport-agnostic** — tracks session state and notifies handlers. Actual audio/video transport happens in Globe (binary frames) and codecs (encoding/decoding). Globe's `SignalingBuilder` creates the ORP events that carry the wire types.

## Covenant Alignment

**Sovereignty** — modules are autonomous actors that communicate by choice, not by forced coupling. **Consent** — every handler is explicitly registered; nothing is automatic.

## Dependencies

```toml
uuid, chrono, serde, serde_json, log, thiserror
```

None. Equipment has zero internal Omninet dependencies — that's the whole point.
