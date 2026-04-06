import COmnideaFFI
import Foundation

/// Errors from the Omnidea Rust core, retrieved via `divi_last_error()`.
public struct OmnideaError: Error, CustomStringConvertible {
    public let message: String

    public init(message: String) {
        self.message = message
    }

    public var description: String { message }

    /// Check the thread-local error from Rust and throw if present.
    static func check() throws {
        let ptr = divi_last_error()
        guard ptr != nil else { return }
        let message = String(cString: ptr!)
        throw OmnideaError(message: message)
    }
}
