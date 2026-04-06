# Orchestrator — The Universal Switchboard

Composes the existing 1,040 divinity FFI functions into smart, app-level operations. Written in Zig for universal C interop. Every platform links against the same library.

Equipment is the telephone hardware. The Orchestrator is the switchboard.

## Why Zig

- Seamless C interop — `@cImport("divinity_ffi.h")` imports all 1,040 functions directly
- No new FFI boundary — Zig calls the existing C functions, not Rust
- Universal — compiles to every platform, produces a C-compatible static library
- Modular — modules, comptime, proper error handling
- Simple — the whole language fits in your head

## Architecture

```
Rust crates (29, protocol primitives)
    ↓ compiled to
libdivinity_ffi.a (1,040 C functions, existing)
    ↑ calls directly
libomnidea_orchestrator.a (Zig, smart composition)
    ↑ links against
Platform wrappers (Swift @Observable / Kotlin StateFlow / TS signals)
    ↑
UI (SwiftUI / Compose / DOM)
```

The orchestrator does NOT create a new FFI boundary. It calls the existing `divi_*` C functions and composes them into higher-level operations. Platforms link against both `libdivinity_ffi` and `libomnidea_orchestrator`.

## Modules (16 files, 14 semantic modules)

| Module | File | Functions | What It Composes |
|--------|------|-----------|-----------------|
| **State** | `state.zig` | 4 | Global singleton, init/shutdown, Equipment + registries |
| **Identity** | `identity.zig` | — | Crown + Sentinal: identity lifecycle types |
| **Storage** | `storage.zig` | — | Vault: encrypted storage lifecycle |
| **Content** | `content.zig` | 5 | Ideas + Hall + Vault: note CRUD (create, save, load, delete, list) |
| **Infrastructure** | `infrastructure.zig` | — | Omnibus: Tower lifecycle, networking, health |
| **Intercom** | `intercom.zig` | 10 | Equipment Phone: intent routing, program registry, handler delivery |
| **Governance** | `governance.zig` | 9 | Polity + Bulwark + Jail: init/shutdown for 9 handles |
| **Commerce** | `commerce.zig` | 4 | Fortune + Commerce: init/shutdown for 4 handles |
| **Discovery** | `discovery.zig` | 6 | Zeitgeist + Yoke: init/shutdown for 5 handles |
| **AI** | `ai.zig` | 6 | Advisor + Oracle: init/shutdown for 5 handles |
| **Lingo** | `lingo.zig` | — | Lingo (Babel): text obfuscation/deobfuscation |
| **Registry** | `registry.zig` | 6 | Comptime auto-dispatch registry. ~99% of divi_* ops callable (auto-counted at comptime). Four dispatch layers: stateless (8 patterns), handle-bearing (9), array-input (8), universal marshaller (mixed types, out-params, secondary handles). Only ~8 callback functions remain as multi_arg (handled by Intercom). Third-party ops via runtime HashMap. |
| **Federation** | `federation.zig` | — | Kingdom federation: propose, accept, suspend, reactivate, withdraw agreements. Registry + path-finding. |
| **Pipeline** | `pipeline.zig` | 3 | Dynamic pipeline executor with modifier system. Source context, Polity/Bulwark pre-checks, Yoke provenance, handle tracking. |

**Total: ~119 exported C functions, ~8,580 lines, 139 tests.**

## Operation Registry (`registry.zig`)

The foundation of the dynamic pipeline executor. Maps string keys to `OpHandler` structs that wrap `divi_*` FFI calls behind a uniform `fn([*:0]const u8) -> ?[*:0]u8` interface.

### Types
- **`HandlerFn`** — `*const fn([*:0]const u8) callconv(.c) ?[*:0]u8`. Universal handler signature.
- **`HandleReq`** — Enum of all opaque handles (global + module-level). For pre-flight validation.
- **`PermissionLevel`** — `free`, `granted_once`, `per_action`, `always_ask`.
- **`ModifierSet`** — Packed bitfield: polity, bulwark, sentinal, yoke, lingo, quest.
- **`OpHandler`** — `{ call, handles, permission, modifiers }`.

### Architecture: Comptime Auto-Dispatch
All dispatchable `divi_*` functions (from the 1,040 total `extern "C"` functions) are automatically discovered at compile time via `@typeInfo(@cImport(...)).@"struct".decls`. Each function's signature is inspected and classified into calling patterns. ~99% are fully callable through the pipeline. The remaining ~8 are callback-registration functions handled by Intercom. Exact counts are verified by comptime tests in registry.zig.

No manual registration. No HashMap for built-in ops. Adding a new `divi_*` function to the C header automatically makes it available in the registry.

### Calling Patterns (auto-detected at comptime)

**Stateless patterns (8):** `() -> str/i32/bool/usize` and `(str) -> str/i32/bool/usize`

**Handle-bearing patterns (9):** `(handle) -> str/i32/bool/i64/void` and `(handle, str) -> str/i32/bool/void`. Handle auto-resolved from state field or module getter at comptime.

**Array-input patterns (8):** `(handle?, str...) -> str/i32/bool/void`. JSON array parsed into N string args.

**Universal marshaller:** Mixed param types (str/int/float/bool/handle/out-param). Handles:
- Secondary handles (2+ opaque pointers) — parsed as integer addresses from JSON (from `$ref` pipeline step outputs)
- Out-params (`uint8_t**`, `uintptr_t*`) — resolved via thread-local storage, binary results hex-encoded
- Handle return types (`_new` constructors) — address returned as integer
- All numeric types (i32/u32/i64/u64/u16/f64/f32/bool)

**Only callbacks remain as multi_arg (8 functions):** These take function pointers and are registered via Intercom/Equipment, not the pipeline.

### Convention-Based Metadata
Permission levels and modifier sets are inferred from function names at comptime:
- Functions containing "delete", "remove", "revoke" → `per_action`
- Functions containing "encrypt", "sign", "derive" → `per_action`
- Vault/Hall functions → `sentinal + yoke` modifiers
- Sentinal functions → `sentinal` modifier
- Content creation functions → `polity + yoke` modifiers
- Polity/Kingdom functions → `polity` modifier
- Bulwark functions → `bulwark` modifier

### Third-Party Operations
Runtime-registered operations (from platform code) use a small `StringHashMap(OpHandler)` behind `RwLock`. Dispatch checks comptime first, falls through to third-party.

### Exported C API
- `orch_registry_init()` / `orch_registry_shutdown()` — lifecycle (only manages third-party HashMap).
- `orch_registry_has_op(key)` — check if operation exists (comptime + third-party).
- `orch_registry_list_ops()` — JSON array of all operation names.
- `orch_registry_count()` — number of registered operations.
- `orch_registry_register(key, handler)` — register custom operation from platform.

### Thread Safety
Third-party HashMap behind `std.Thread.RwLock`. Multiple concurrent readers for lookup/has/count. Exclusive lock for init/deinit/register. Comptime dispatch is inherently thread-safe (read-only).

## Key Compositions (multi-step operations)

Smart operations that compose multiple FFI calls:

- **`orch_note_create(title, content_json)`** — keyring pubkey -> digit_new -> set title property -> header_create -> personal_path -> build package JSON -> content_key -> hall_write -> vault register -> return ManifestEntry
- **`orch_note_save(idea_id, package_json)`** — vault get_idea (path) -> resolve_path -> content_key -> hall_write
- **`orch_note_load(idea_id)`** — vault get_idea (path) -> resolve_path -> content_key -> hall_read
- **`orch_note_delete(idea_id)`** — vault unregister_idea (manifest only, no disk cleanup)
- **`orch_note_list(filter_json)`** — inject extended_type="note" into filter -> vault list_ideas
- **`orch_intercom_register_handler(id, handler, ctx)`** — alloc HandlerContext -> register trampoline with Phone -> store on Program
- **`orch_intercom_fire(action, digit_type, payload, source)`** — find matching program -> build intent JSON -> deliver via phone_call_raw_if_available -> trampoline -> platform handler
- **`orch_init()`** — runtime + equipment + registries (13 handles)
- **`orch_shutdown()`** — module shutdowns (reverse order) -> free all 23+ handles

## State Management

### Global State (`state.zig`)
Holds all cross-cutting handles: runtime, keyring, soul, vault, equipment (phone/email/contacts/pager), omnibus, theme, registries. Created by `orch_init()`, freed by `orch_shutdown()`.

### Module State
Each module (governance, commerce, discovery, ai) holds its own opaque handles as module-level globals. Each has an `orch_*_shutdown()` function to free its state.

### Lifecycle
```
orch_init()                    → runtime + equipment + registries
orch_create_identity()         → keyring + soul in state
orch_vault_setup(pw, path)     → vault in state
orch_tower_start(config)       → omnibus in state
orch_*_init()                  → module-specific registries
... app runs ...
orch_*_shutdown()              → module registries freed
orch_shutdown()                → everything freed in reverse order
```

## Building

```bash
cd Divinity/Orchestrator
zig build              # produces zig-out/lib/libomnidea_orchestrator.a
zig build test         # runs all tests (64 currently)
```

Requires `libdivinity_ffi.a` in `../../target/debug/` (build Omninet Rust first).

## Conventions

- All exported functions use `orch_` prefix
- All exported functions are `export fn` (C-compatible)
- Strings are null-terminated `[*:0]const u8` (C strings)
- Return strings must be freed by caller via `divi_free_string`
- Return bytes must be freed via `divi_free_bytes(ptr, len)`
- Out-params use `[*c][*c]u8` for `uint8_t**` and `[*c]usize` for `uintptr_t*`
- Error codes: 0 = success, negative = error, check `orch_last_error()`
- Tri-state returns: 1 = yes, 0 = no, -1 = error (rights/mandates/consent checks)
- Each module is one `.zig` file that imports `ffi.zig` and `state.zig`
- Tests live alongside the code they test (Zig convention)
- Module-specific handles freed by `orch_*_shutdown()`, cross-cutting handles freed by `orch_shutdown()`

## Thread Safety

The orchestrator is fully thread-safe. Multiple threads can call `orch_*` functions concurrently.

### Global State (`state.zig`): RwLock
- **Readers** (`acquireShared()` / `releaseShared()`): Multiple threads read handles concurrently. Held during FFI calls to prevent shutdown from freeing handles mid-use.
- **Writers** (`setKeyring()`, `setVault()`, etc.): Exclusive access. Setters free the old handle and set the new one atomically.
- **Lifecycle** (`orch_init()` / `orch_shutdown()`): Exclusive access. Shutdown blocks until all in-flight readers complete.

### Module State: Mutex per module
Each module (intercom, governance, commerce, discovery, ai) has its own `std.Thread.Mutex` protecting module-level globals. Init functions check-then-allocate inside the lock — no double-alloc races.

### Lock ordering (deadlock prevention)
- Never hold a shared state lock while calling a setter (setters acquire exclusive)
- Module mutexes are independent — no cross-module lock nesting
- Pattern for write paths: `isInitialized()` → FFI work → `setKeyring(result)`
- Pattern for read paths: `acquireShared()` → read handle → FFI call → `releaseShared()`
- **Intercom fire:** releases mod_mutex BEFORE calling `divi_phone_call_raw_if_available`. The trampoline is called synchronously by Phone inside that call. If fire held mod_mutex, and the trampoline also needed it, that would deadlock. The trampoline avoids this by reading only from its own heap-allocated `HandlerContext` (stable pointer, no mutex needed).

## Extending

To add a new orchestrator module:
1. Create `src/newmodule.zig` importing `ffi.zig` and `state.zig`
2. Add `var mod_mutex: std.Thread.Mutex = .{};` for module state protection
3. Write `export fn orch_*` functions composing `c.divi_*` calls
4. Lock `mod_mutex` in every function that touches module state
5. Use `state.acquireShared()` / `releaseShared()` for reading global handles
6. Add module-level state if needed, with `orch_newmodule_shutdown()`
7. Add `pub const newmodule = @import("newmodule.zig");` to `main.zig`
8. Add to both `comptime` and `test` blocks in `main.zig`
9. Write tests inline
