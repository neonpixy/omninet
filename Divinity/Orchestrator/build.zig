const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // The orchestrator module — composes existing divinity FFI calls
    const orch_mod = b.addModule("omnidea_orchestrator", .{
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });

    // Import divinity_ffi.h — the existing 990 C functions
    orch_mod.addIncludePath(b.path("../Apple/Sources/COmnideaFFI/include"));

    // Link against libdivinity_ffi (the existing Rust library)
    orch_mod.addLibraryPath(b.path("../../target/debug"));
    orch_mod.addLibraryPath(b.path("../../target/release"));
    orch_mod.linkSystemLibrary("divinity_ffi", .{});
    orch_mod.linkSystemLibrary("c", .{});

    // macOS frameworks that libdivinity_ffi depends on (SQLCipher, timezone, crypto)
    orch_mod.linkFramework("CoreFoundation", .{});
    orch_mod.linkFramework("Security", .{});
    orch_mod.linkFramework("SystemConfiguration", .{});

    // Static library artifact
    const lib = b.addLibrary(.{
        .name = "omnidea_orchestrator",
        .root_module = orch_mod,
        .linkage = .static,
    });
    b.installArtifact(lib);

    // Tests
    const tests = b.addTest(.{
        .root_module = orch_mod,
    });
    const run_tests = b.addRunArtifact(tests);
    const test_step = b.step("test", "Run orchestrator tests");
    test_step.dependOn(&run_tests.step);
}
