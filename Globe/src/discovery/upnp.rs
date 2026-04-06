//! UPnP port mapping for Tower relay nodes.
//!
//! Automatically maps a local port through the router via UPnP IGD,
//! making the Tower publicly reachable without manual router configuration.
//! Non-fatal — if UPnP is unavailable, the Tower still works locally.

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};

use igd_next::SearchOptions;
use tokio::runtime::Runtime;

/// Result of a UPnP port mapping attempt.
#[derive(Clone, Debug)]
pub struct PortMapping {
    /// The external (public) IP address.
    pub external_ip: IpAddr,
    /// The external port on the router.
    pub external_port: u16,
    /// The local port that was mapped.
    pub local_port: u16,
    /// The full public WebSocket URL.
    pub public_url: String,
}

/// UPnP port mapper for Tower relay nodes.
///
/// Attempts to map a local port through the router via UPnP IGD.
/// Automatically renews the lease and cleans up on drop.
pub struct PortMapper {
    mapping: Arc<Mutex<Option<PortMapping>>>,
    _renewal_handle: Option<tokio::task::JoinHandle<()>>,
    shutdown: Arc<tokio::sync::Notify>,
    runtime: Arc<Runtime>,
}

/// Lease duration in seconds (1 hour).
const LEASE_DURATION: u32 = 3600;

/// Renewal interval — renew every 50 minutes, before the 60-minute lease expires.
const RENEWAL_INTERVAL_SECS: u64 = 50 * 60;

/// Gateway search timeout in seconds.
const SEARCH_TIMEOUT_SECS: u64 = 5;

/// Number of gateway discovery attempts on cold boot.
const DISCOVERY_RETRIES: u32 = 3;

/// Delay between retry attempts in seconds (doubles each retry).
const RETRY_BASE_DELAY_SECS: u64 = 1;

impl PortMapper {
    /// Attempt a UPnP port mapping for the given local port.
    ///
    /// Returns `Some(PortMapper)` if the mapping succeeded, `None` if UPnP
    /// is unavailable (non-fatal). The mapper renews the lease automatically
    /// and removes the mapping on drop.
    pub async fn map(local_port: u16, runtime: Arc<Runtime>) -> Option<PortMapper> {
        // Step 1: Search for UPnP gateway with retries.
        // On cold boot the network stack may not be ready yet, so we retry
        // with exponential backoff (1s, 2s, 4s) before giving up.
        let gateway = {
            let mut last_err = String::new();
            let mut found = None;
            for attempt in 0..DISCOVERY_RETRIES {
                if attempt > 0 {
                    let delay = RETRY_BASE_DELAY_SECS << (attempt - 1);
                    log::info!("UPnP: retrying gateway search in {delay}s (attempt {}/{})",
                        attempt + 1, DISCOVERY_RETRIES);
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                }
                let search_opts = SearchOptions {
                    timeout: Some(std::time::Duration::from_secs(SEARCH_TIMEOUT_SECS)),
                    ..Default::default()
                };
                match igd_next::aio::tokio::search_gateway(search_opts).await {
                    Ok(gw) => {
                        if attempt > 0 {
                            log::info!("UPnP: gateway found on attempt {}", attempt + 1);
                        }
                        found = Some(gw);
                        break;
                    }
                    Err(e) => {
                        last_err = e.to_string();
                    }
                }
            }
            match found {
                Some(gw) => gw,
                None => {
                    log::info!("UPnP: gateway search failed after {DISCOVERY_RETRIES} attempts (non-fatal): {last_err}");
                    return None;
                }
            }
        };

        // Step 2: Get external IP.
        let external_ip = match gateway.get_external_ip().await {
            Ok(ip) => ip,
            Err(e) => {
                log::warn!("UPnP: failed to get external IP: {e}");
                return None;
            }
        };

        // Step 3: Determine our local IP by finding a local IPv4 address.
        let local_ip = match find_local_ipv4() {
            Some(ip) => ip,
            None => {
                log::warn!("UPnP: no local IPv4 address found");
                return None;
            }
        };

        let local_addr = SocketAddr::new(IpAddr::V4(local_ip), local_port);

        // Step 4: Try adding a port mapping (same port externally).
        let external_port = match gateway
            .add_port(
                igd_next::PortMappingProtocol::TCP,
                local_port,
                local_addr,
                LEASE_DURATION,
                "Omnidea Tower",
            )
            .await
        {
            Ok(()) => {
                log::info!(
                    "UPnP: mapped {local_addr} -> {}:{local_port}",
                    external_ip
                );
                local_port
            }
            Err(e) => {
                log::info!("UPnP: same-port mapping failed ({e}), trying any port");
                // Step 4b: Let the router pick an available port.
                match gateway
                    .add_any_port(
                        igd_next::PortMappingProtocol::TCP,
                        local_addr,
                        LEASE_DURATION,
                        "Omnidea Tower",
                    )
                    .await
                {
                    Ok(port) => {
                        log::info!(
                            "UPnP: mapped {local_addr} -> {}:{port}",
                            external_ip
                        );
                        port
                    }
                    Err(e2) => {
                        log::warn!("UPnP: port mapping failed: {e2}");
                        return None;
                    }
                }
            }
        };

        let public_url = format!("ws://{}:{}", external_ip, external_port);

        let mapping = PortMapping {
            external_ip,
            external_port,
            local_port,
            public_url,
        };

        let mapping_arc = Arc::new(Mutex::new(Some(mapping)));
        let shutdown = Arc::new(tokio::sync::Notify::new());

        // Step 5: Start background renewal task.
        let renewal_handle = {
            let shutdown_clone = Arc::clone(&shutdown);
            let mapping_clone = Arc::clone(&mapping_arc);
            runtime.spawn(async move {
                loop {
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_secs(RENEWAL_INTERVAL_SECS)) => {}
                        _ = shutdown_clone.notified() => {
                            log::info!("UPnP: renewal task shutting down");
                            break;
                        }
                    }

                    // Renew the lease by re-adding the mapping.
                    match gateway
                        .add_port(
                            igd_next::PortMappingProtocol::TCP,
                            external_port,
                            local_addr,
                            LEASE_DURATION,
                            "Omnidea Tower",
                        )
                        .await
                    {
                        Ok(()) => {
                            log::info!("UPnP: lease renewed for port {external_port}");
                        }
                        Err(e) => {
                            log::warn!("UPnP: lease renewal failed: {e}");
                            // Clear the mapping so callers know it's gone.
                            *mapping_clone.lock().unwrap_or_else(|e| e.into_inner()) = None;
                            break;
                        }
                    }
                }
            })
        };

        Some(PortMapper {
            mapping: mapping_arc,
            _renewal_handle: Some(renewal_handle),
            shutdown,
            runtime,
        })
    }

    /// Returns the current port mapping info, or `None` if the mapping
    /// has expired or was never established.
    pub fn mapping(&self) -> Option<PortMapping> {
        self.mapping.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Convenience: get the public WebSocket URL, or `None`.
    pub fn public_url(&self) -> Option<String> {
        self.mapping().map(|m| m.public_url)
    }
}

impl Drop for PortMapper {
    fn drop(&mut self) {
        // Signal the renewal task to stop.
        self.shutdown.notify_one();

        // Remove the port mapping from the router.
        if let Some(mapping) = self.mapping.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let rt = Arc::clone(&self.runtime);
            let external_port = mapping.external_port;

            // Best-effort removal — don't block indefinitely.
            rt.block_on(async {
                let search_opts = SearchOptions {
                    timeout: Some(std::time::Duration::from_secs(3)),
                    ..Default::default()
                };

                match igd_next::aio::tokio::search_gateway(search_opts).await {
                    Ok(gw) => {
                        match gw
                            .remove_port(igd_next::PortMappingProtocol::TCP, external_port)
                            .await
                        {
                            Ok(()) => {
                                log::info!("UPnP: removed port mapping for {external_port}");
                            }
                            Err(e) => {
                                log::warn!(
                                    "UPnP: failed to remove port mapping for {external_port}: {e}"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("UPnP: gateway search failed during cleanup: {e}");
                    }
                }
            });
        }
    }
}

/// Find a local IPv4 address that's likely on the LAN (not loopback).
fn find_local_ipv4() -> Option<std::net::Ipv4Addr> {
    // Try connecting to a public address (doesn't actually send data)
    // to determine which local interface the OS would use.
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let local_addr = socket.local_addr().ok()?;
    match local_addr.ip() {
        IpAddr::V4(v4) => Some(v4),
        IpAddr::V6(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_mapping_clone_and_debug() {
        let m = PortMapping {
            external_ip: IpAddr::V4(std::net::Ipv4Addr::new(73, 45, 123, 89)),
            external_port: 8080,
            local_port: 8080,
            public_url: "ws://73.45.123.89:8080".into(),
        };
        let cloned = m.clone();
        assert_eq!(cloned.external_port, 8080);
        assert_eq!(cloned.public_url, "ws://73.45.123.89:8080");
        // Debug output should contain the type name.
        let debug = format!("{m:?}");
        assert!(debug.contains("PortMapping"));
    }

    #[test]
    fn port_mapping_public_url_format() {
        let m = PortMapping {
            external_ip: IpAddr::V4(std::net::Ipv4Addr::new(1, 2, 3, 4)),
            external_port: 9090,
            local_port: 8080,
            public_url: "ws://1.2.3.4:9090".into(),
        };
        assert_eq!(m.public_url, "ws://1.2.3.4:9090");
    }

    #[test]
    fn find_local_ipv4_returns_something() {
        // This test may fail in CI without network access, but should
        // succeed on any machine with a LAN connection.
        if let Some(ip) = find_local_ipv4() {
            assert!(!ip.is_loopback());
        }
        // Not asserting Some — CI might not have network.
    }

    #[test]
    fn constants_are_sensible() {
        assert_eq!(LEASE_DURATION, 3600);
        assert_eq!(RENEWAL_INTERVAL_SECS, 50 * 60);
        assert!(RENEWAL_INTERVAL_SECS < LEASE_DURATION as u64);
        assert_eq!(SEARCH_TIMEOUT_SECS, 5);
        assert!(DISCOVERY_RETRIES >= 1);
        assert!(RETRY_BASE_DELAY_SECS >= 1);
    }
}
