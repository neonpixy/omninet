import COmnideaFFI
import Foundation

// MARK: - Local Peer

/// A discovered Omnidea relay on the local network.
public struct LocalPeer: Codable, Sendable {
    public let name: String
    public let addresses: [String]
    public let port: UInt16
    public let pubkeyHex: String?
    public let wsUrl: String?

    enum CodingKeys: String, CodingKey {
        case name, addresses, port
        case pubkeyHex = "pubkey_hex"
        case wsUrl = "ws_url"
    }
}

// MARK: - Advertiser

/// Advertises this device's relay on the local network via mDNS.
///
/// Other Omnidea devices will discover this relay automatically.
/// The advertisement stops when this object is deallocated.
public final class LocalAdvertiser: @unchecked Sendable {
    private let ptr: OpaquePointer

    /// Start advertising a relay.
    ///
    /// - Parameters:
    ///   - name: Human-readable name (e.g., "Sam's Mac").
    ///   - port: The relay server port.
    ///   - pubkeyHex: Optional public key to include in discovery.
    public init(name: String, port: UInt16, pubkeyHex: String? = nil) throws {
        guard let p = divi_discovery_advertise(name, port, pubkeyHex) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to start mDNS advertiser")
        }
        ptr = p
    }

    deinit {
        divi_discovery_advertiser_free(ptr)
    }
}

// MARK: - Browser

/// Discovers Omnidea relays on the local network via mDNS.
///
/// Peers are discovered automatically in the background.
/// Call `peers` to get the current list at any time.
public final class LocalBrowser: @unchecked Sendable {
    private let ptr: OpaquePointer

    /// Start browsing for Omnidea relays.
    public init() throws {
        guard let p = divi_discovery_browse() else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to start mDNS browser")
        }
        ptr = p
    }

    deinit {
        divi_discovery_browser_free(ptr)
    }

    /// Number of currently discovered peers.
    public var peerCount: UInt32 {
        divi_discovery_peer_count(ptr)
    }

    /// All currently discovered peers.
    public var peers: [LocalPeer] {
        guard let json = divi_discovery_peers(ptr) else { return [] }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return (try? JSONDecoder().decode([LocalPeer].self, from: data)) ?? []
    }
}
