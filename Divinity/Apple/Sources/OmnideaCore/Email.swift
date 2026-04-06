import COmnideaFFI
import Foundation

/// Swift wrapper around the Rust Email (pub/sub hub).
public final class Email: @unchecked Sendable {
    private let ptr: OpaquePointer

    public init() {
        ptr = divi_email_new()!
    }

    deinit {
        divi_email_free(ptr)
    }

    /// Subscribe a handler for an email ID. Returns a subscriber UUID.
    public func subscribeRaw(_ emailId: String, handler: @escaping (Data) -> Void) -> UUID {
        let boxed = Unmanaged.passRetained(EmailHandlerBox(handler)).toOpaque()

        var uuidBytes: [UInt8] = Array(repeating: 0, count: 16)
        divi_email_subscribe_raw(ptr, emailId, emailTrampoline, boxed, &uuidBytes)

        return UUID(uuid: (
            uuidBytes[0], uuidBytes[1], uuidBytes[2], uuidBytes[3],
            uuidBytes[4], uuidBytes[5], uuidBytes[6], uuidBytes[7],
            uuidBytes[8], uuidBytes[9], uuidBytes[10], uuidBytes[11],
            uuidBytes[12], uuidBytes[13], uuidBytes[14], uuidBytes[15]
        ))
    }

    /// Send raw bytes to all subscribers. Fire-and-forget.
    public func sendRaw(_ emailId: String, data: Data = Data()) {
        data.withUnsafeBytes { buffer in
            divi_email_send_raw(
                ptr,
                emailId,
                buffer.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(buffer.count)
            )
        }
    }

    /// Unsubscribe by subscriber UUID.
    public func unsubscribe(_ subscriberId: UUID) {
        withUnsafeBytes(of: subscriberId.uuid) { buffer in
            divi_email_unsubscribe(ptr, buffer.baseAddress?.assumingMemoryBound(to: UInt8.self))
        }
    }

    /// Unsubscribe all subscribers for an email ID.
    public func unsubscribeAll(_ emailId: String) {
        divi_email_unsubscribe_all(ptr, emailId)
    }

    /// Check if any subscribers exist for an email ID.
    public func hasSubscribers(_ emailId: String) -> Bool {
        divi_email_has_subscribers(ptr, emailId)
    }

    /// Get all active email IDs.
    public func activeEmailIds() -> [String] {
        guard let json = divi_email_active_email_ids(ptr) else { return [] }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return (try? JSONDecoder().decode([String].self, from: data)) ?? []
    }
}

// MARK: - Callback

private final class EmailHandlerBox: @unchecked Sendable {
    let handler: (Data) -> Void
    init(_ handler: @escaping (Data) -> Void) { self.handler = handler }
}

private let emailTrampoline: DiviEmailHandler = { data, len, context in
    guard let context else { return }
    let box = Unmanaged<EmailHandlerBox>.fromOpaque(context).takeUnretainedValue()

    let eventData: Data
    if let data, len > 0 {
        eventData = Data(bytes: data, count: Int(len))
    } else {
        eventData = Data()
    }
    box.handler(eventData)
}
