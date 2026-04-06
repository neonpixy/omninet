import COmnideaFFI
import Foundation

/// Shared async runtime for all Omnidea async operations.
///
/// Create once at app startup, pass to Globe pool and any future
/// async services. All crates share the same runtime.
public final class OmnideaRuntime: @unchecked Sendable {
    let ptr: OpaquePointer

    public init() {
        ptr = divi_runtime_new()!
    }

    deinit {
        divi_runtime_free(ptr)
    }
}
