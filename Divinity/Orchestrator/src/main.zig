// Omnidea Orchestrator
//
// Composes the existing 990 divinity FFI functions into smart,
// app-level operations. Written in Zig for universal C interop.
//
// Equipment is the telephone hardware. The Orchestrator is the switchboard.
//
// Modules:
//   state          — Global state, init/shutdown, handle accessors
//   identity       — Crown + Sentinal: create/import identity, profile
//   storage        — Vault: encrypted storage lifecycle
//   content        — Ideas + Magic + Hall: document pipeline
//   infrastructure — Omnibus: Tower lifecycle, networking, health
//   intercom       — Equipment Phone: inter-program intent routing
//   governance     — Kingdom + Polity + Bulwark + Jail: rights, consent, trust
//   federation     — Kingdom federation: cross-community agreements and registry
//   commerce       — Fortune + Commerce: wallet, cart, checkout
//   discovery      — Zeitgeist + Yoke: search, trends, history
//   ai             — Advisor + Oracle: AI cognition, guidance
//   lingo          — Lingo (Babel): text obfuscation/deobfuscation
//   registry       — Operation registry for the dynamic pipeline executor
//   pipeline       — Dynamic pipeline executor: multi-step operation chaining

pub const state = @import("state.zig");
pub const identity = @import("identity.zig");
pub const storage = @import("storage.zig");
pub const content = @import("content.zig");
pub const infrastructure = @import("infrastructure.zig");
pub const intercom = @import("intercom.zig");
pub const governance = @import("governance.zig");
pub const federation = @import("federation.zig");
pub const commerce = @import("commerce.zig");
pub const discovery = @import("discovery.zig");
pub const ai = @import("ai.zig");
pub const lingo = @import("lingo.zig");
pub const registry = @import("registry.zig");
pub const pipeline = @import("pipeline.zig");

// Re-export C API functions so they're visible to the linker
comptime {
    _ = state;
    _ = identity;
    _ = storage;
    _ = content;
    _ = infrastructure;
    _ = intercom;
    _ = governance;
    _ = federation;
    _ = commerce;
    _ = discovery;
    _ = ai;
    _ = lingo;
    _ = registry;
    _ = pipeline;
}

// Tests — run all module tests
test {
    _ = state;
    _ = identity;
    _ = storage;
    _ = content;
    _ = infrastructure;
    _ = intercom;
    _ = governance;
    _ = federation;
    _ = commerce;
    _ = discovery;
    _ = ai;
    _ = lingo;
    _ = registry;
    _ = pipeline;
}
