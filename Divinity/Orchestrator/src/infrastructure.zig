// Infrastructure orchestration — Omnibus (Tower) + Globe + Undercroft.
//
// Tower is your personal node. Handle management lives in state.zig (omnibus).
// Operations previously here are now served by the pipeline executor.
//
// TODO: Add smart orch_* compositions here when the pipeline executor
// is insufficient for complex infrastructure workflows (e.g., Tower
// startup with Globe relay connection, Undercroft health check, and
// mDNS discovery in a single orchestrated sequence). Individual
// Omnibus/Globe/Undercroft operations are already available via the
// pipeline's registry auto-dispatch.

const std = @import("std");
const ffi = @import("ffi.zig");
const c = ffi.c;
const state = @import("state.zig");
