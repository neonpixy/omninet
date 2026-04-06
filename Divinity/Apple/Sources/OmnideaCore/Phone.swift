import COmnideaFFI
import Foundation

/// Swift wrapper around the Rust Phone (RPC switchboard).
public final class Phone: @unchecked Sendable {
    private let ptr: OpaquePointer

    public init() {
        ptr = divi_phone_new()!
    }

    deinit {
        divi_phone_free(ptr)
    }

    /// Register a handler that receives raw bytes and returns raw bytes.
    public func registerRaw(_ callId: String, handler: @escaping (Data) -> Data?) {
        let boxed = Unmanaged.passRetained(PhoneHandlerBox(handler)).toOpaque()
        divi_phone_register_raw(ptr, callId, phoneTrampoline, boxed)
    }

    /// Make a raw call. Returns the response bytes.
    public func callRaw(_ callId: String, data: Data = Data()) throws -> Data {
        var outData: UnsafeMutablePointer<UInt8>?
        var outLen: UInt = 0

        let result = data.withUnsafeBytes { buffer in
            divi_phone_call_raw(
                ptr,
                callId,
                buffer.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(buffer.count),
                &outData,
                &outLen
            )
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Phone call failed")
        }

        guard let outData else { return Data() }
        let response = Data(bytes: outData, count: Int(outLen))
        divi_free_bytes(outData, outLen)
        return response
    }

    /// Check if a handler is registered for a call ID.
    public func hasHandler(_ callId: String) -> Bool {
        divi_phone_has_handler(ptr, callId)
    }

    /// Get all registered call IDs.
    public func registeredCallIds() -> [String] {
        guard let json = divi_phone_registered_call_ids(ptr) else { return [] }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return (try? JSONDecoder().decode([String].self, from: data)) ?? []
    }

    /// Unregister a handler by call ID.
    public func unregister(_ callId: String) {
        divi_phone_unregister(ptr, callId)
    }
}

// MARK: - Callback

private final class PhoneHandlerBox: @unchecked Sendable {
    let handler: (Data) -> Data?
    init(_ handler: @escaping (Data) -> Data?) { self.handler = handler }
}

private let phoneTrampoline: DiviPhoneHandler = { requestData, requestLen, responseData, responseLen, context in
    guard let context else { return -1 }
    let box = Unmanaged<PhoneHandlerBox>.fromOpaque(context).takeUnretainedValue()

    let request: Data
    if let requestData, requestLen > 0 {
        request = Data(bytes: requestData, count: Int(requestLen))
    } else {
        request = Data()
    }

    guard let response = box.handler(request) else { return -1 }

    let buf = UnsafeMutablePointer<UInt8>.allocate(capacity: response.count)
    response.copyBytes(to: buf, count: response.count)
    responseData?.pointee = buf
    responseLen?.pointee = UInt(response.count)
    return 0
}
