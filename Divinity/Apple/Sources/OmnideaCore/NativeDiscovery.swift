import Foundation
import Network
import os

private let logger = Logger(subsystem: "com.omnidea.core", category: "Discovery")

/// Advertises this device's Omnidea relay on the local network using Apple's Network framework.
public final class NativeAdvertiser: @unchecked Sendable {
    private var listener: NWListener?

    /// Start advertising a relay.
    ///
    /// - Parameters:
    ///   - name: Human-readable name (e.g., "Sam's iPhone").
    ///   - relayPort: The relay server port to advertise.
    public init(name: String, relayPort: UInt16) throws {
        let params = NWParameters.tcp
        params.includePeerToPeer = true

        let listener = try NWListener(using: params)
        self.listener = listener

        let serviceName = "\(name)|\(relayPort)"

        listener.service = NWListener.Service(
            name: serviceName,
            type: "_omnidea._tcp"
        )

        listener.stateUpdateHandler = { state in
            switch state {
            case .ready:
                logger.info("Advertising '\(name)' (relay port \(relayPort))")
            case .failed(let error):
                logger.error("Advertiser failed: \(error)")
            default:
                break
            }
        }

        // Accept connections briefly so resolvers can complete their handshake.
        listener.newConnectionHandler = { connection in
            connection.start(queue: .global(qos: .utility))
            // Let it live for a moment then clean up.
            DispatchQueue.global().asyncAfter(deadline: .now() + 2) {
                connection.cancel()
            }
        }

        listener.start(queue: .global(qos: .utility))
    }

    deinit {
        listener?.cancel()
    }
}

/// A discovered Omnidea peer on the local network.
public struct DiscoveredPeer: Sendable, Identifiable {
    public var id: String { name }
    public let name: String
    public let relayPort: UInt16
    /// The endpoint for resolving the peer's address.
    public let endpoint: NWEndpoint
}

/// Discovers Omnidea relays on the local network using Apple's Network framework.
public final class NativeBrowser: @unchecked Sendable {
    private var browser: NWBrowser?
    private let lock = NSLock()
    private var discovered: [String: DiscoveredPeer] = [:]

    /// Start browsing for Omnidea relays.
    public init() {
        let params = NWParameters()
        params.includePeerToPeer = true

        let browser = NWBrowser(for: .bonjour(type: "_omnidea._tcp", domain: nil), using: params)
        self.browser = browser

        browser.stateUpdateHandler = { state in
            switch state {
            case .ready:
                logger.info("NativeBrowser: browsing for _omnidea._tcp")
            case .failed(let error):
                logger.error("NativeBrowser: failed — \(error)")
            default:
                break
            }
        }

        browser.browseResultsChangedHandler = { [weak self] results, _ in
            self?.handleResults(results)
        }

        browser.start(queue: .global(qos: .utility))
    }

    deinit {
        browser?.cancel()
    }

    /// All currently discovered peers.
    public var peers: [DiscoveredPeer] {
        lock.withLock { Array(discovered.values) }
    }

    private func handleResults(_ results: Set<NWBrowser.Result>) {
        lock.withLock {
            discovered.removeAll()

            for result in results {
                guard case .service(let serviceName, _, _, _) = result.endpoint else { continue }

                let parts = serviceName.split(separator: "|", maxSplits: 1)
                let name = parts.first.map(String.init) ?? serviceName
                let port = parts.last.flatMap { UInt16($0) } ?? 0

                let peer = DiscoveredPeer(
                    name: name,
                    relayPort: port,
                    endpoint: result.endpoint
                )
                discovered[serviceName] = peer
                logger.info("Found peer: '\(name)' relay port \(port)")
            }
        }
    }

    /// Resolve a peer's IP address by connecting to the Bonjour endpoint.
    public func resolveURL(for peer: DiscoveredPeer, handler: @escaping @Sendable (String?) -> Void) {
        logger.info("Resolving \(peer.name)...")

        // Force IPv4 — the relay server binds to 0.0.0.0.
        let params = NWParameters.tcp
        params.requiredInterfaceType = .wifi
        if let ipOptions = params.defaultProtocolStack.internetProtocol as? NWProtocolIP.Options {
            ipOptions.version = .v4
        }
        let connection = NWConnection(to: peer.endpoint, using: params)
        let once = SendableOnce(handler: handler)

        connection.stateUpdateHandler = { state in
            logger.info("Resolve connection state: \(String(describing: state))")
            switch state {
            case .ready:
                if let path = connection.currentPath,
                   let remote = path.remoteEndpoint,
                   case .hostPort(let host, _) = remote {
                    var hostStr = "\(host)"
                    // Strip interface suffix (e.g., "%en0").
                    if let percentIdx = hostStr.firstIndex(of: "%") {
                        hostStr = String(hostStr[..<percentIdx])
                    }
                    // IPv6 addresses need brackets in URLs.
                    if hostStr.contains(":") {
                        hostStr = "[\(hostStr)]"
                    }
                    let url = "ws://\(hostStr):\(peer.relayPort)"
                    logger.info("Resolved \(peer.name) → \(url)")
                    once.call(url)
                } else {
                    logger.warning("Resolved but no remote endpoint")
                    once.call(nil)
                }
                connection.cancel()
            case .failed(let error):
                logger.error("Resolve failed: \(error)")
                once.call(nil)
            case .cancelled:
                once.call(nil)
            default:
                break
            }
        }

        connection.start(queue: .global(qos: .utility))

        // Timeout.
        DispatchQueue.global().asyncAfter(deadline: .now() + 5) {
            once.call(nil)
            connection.cancel()
        }
    }
}

// MARK: - Thread-safe once-only callback

private final class SendableOnce: @unchecked Sendable {
    private let lock = NSLock()
    private var handler: ((String?) -> Void)?

    init(handler: @escaping (String?) -> Void) {
        self.handler = handler
    }

    func call(_ value: String?) {
        lock.withLock {
            handler?(value)
            handler = nil
        }
    }
}
