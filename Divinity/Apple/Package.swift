// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "OmnideaCore",
    platforms: [.macOS(.v26), .iOS(.v26)],
    products: [
        .library(name: "OmnideaCore", targets: ["OmnideaCore"]),
        .library(name: "CrystalKit", targets: ["CrystalKit"]),
    ],
    targets: [
        // C headers + modulemap for the Rust static library.
        .systemLibrary(name: "COmnideaFFI"),

        // Swift wrappers around the C FFI.
        .target(
            name: "OmnideaCore",
            dependencies: ["COmnideaFFI"],
            linkerSettings: [
                .linkedLibrary("divinity_ffi"),
            ]
        ),

        // CrystalKit — Metal rendering library. Standalone, no FFI dependency.
        .target(
            name: "CrystalKit",
            path: "Sources/CrystalKit",
            exclude: ["Holodeck/CLAUDE.md", "Lattice/CLAUDE.md"]
        ),

        .testTarget(
            name: "OmnideaCoreTests",
            dependencies: ["OmnideaCore"]
        ),
    ]
)
