//! Local network discovery via mDNS/DNS-SD.
//!
//! Advertises this device's relay server on the local network and discovers
//! other Omnidea devices. Uses `_omnidea._tcp` as the service type.
//!
//! Cross-platform via the `mdns-sd` crate — works on macOS, Linux, Windows, Android.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::error::GlobeError;

/// The mDNS service type for Omnidea relays.
const SERVICE_TYPE: &str = "_omnidea._tcp.local.";

/// A discovered peer on the local network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalPeer {
    /// The peer's display name (from mDNS instance name).
    pub name: String,
    /// IP addresses (may have both IPv4 and IPv6).
    pub addresses: Vec<IpAddr>,
    /// The relay port.
    pub port: u16,
    /// The peer's public key hex (if advertised in TXT record).
    pub pubkey_hex: Option<String>,
}

impl LocalPeer {
    /// WebSocket URL for connecting to this peer (uses first address).
    pub fn ws_url(&self) -> Option<String> {
        self.addresses
            .first()
            .map(|addr| format!("ws://{}:{}", addr, self.port))
    }
}

/// Advertises this device's relay on the local network.
pub struct LocalAdvertiser {
    daemon: ServiceDaemon,
    service_fullname: String,
}

impl LocalAdvertiser {
    /// Start advertising a relay server.
    ///
    /// - `instance_name`: human-readable name (e.g., "Sam's Mac")
    /// - `port`: the relay server port
    /// - `pubkey_hex`: optional public key to include in TXT record
    pub fn start(
        instance_name: &str,
        port: u16,
        pubkey_hex: Option<&str>,
    ) -> Result<Self, GlobeError> {
        let daemon =
            ServiceDaemon::new().map_err(|e| GlobeError::ProtocolError(format!("mDNS daemon: {e}")))?;

        let mut properties = vec![];
        if let Some(pk) = pubkey_hex {
            properties.push(("pk", pk));
        }

        let host = format!("{}.local.", simple_hostname());
        let service = ServiceInfo::new(
            SERVICE_TYPE,
            instance_name,
            &host,
            "",
            port,
            &properties[..],
        )
        .map_err(|e| GlobeError::ProtocolError(format!("mDNS service info: {e}")))?;

        let fullname = service.get_fullname().to_string();

        daemon
            .register(service)
            .map_err(|e| GlobeError::ProtocolError(format!("mDNS register: {e}")))?;

        log::info!("mDNS: advertising relay as '{instance_name}' on port {port}");

        Ok(Self {
            daemon,
            service_fullname: fullname,
        })
    }

    /// Stop advertising.
    pub fn stop(&self) {
        if let Err(e) = self.daemon.unregister(&self.service_fullname) {
            log::warn!("mDNS unregister failed: {e}");
        }
    }
}

impl Drop for LocalAdvertiser {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Browses the local network for Omnidea relays.
pub struct LocalBrowser {
    daemon: ServiceDaemon,
    peers: Arc<Mutex<HashMap<String, LocalPeer>>>,
}

impl LocalBrowser {
    /// Start browsing for Omnidea relays on the local network.
    ///
    /// Discovered peers are collected internally. Call `peers()` to get the current list.
    pub fn start() -> Result<Self, GlobeError> {
        let daemon = ServiceDaemon::new()
            .map_err(|e| GlobeError::ProtocolError(format!("mDNS browser daemon: {e}")))?;

        let receiver = daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| GlobeError::ProtocolError(format!("mDNS browse: {e}")))?;

        let peers: Arc<Mutex<HashMap<String, LocalPeer>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let peers_clone = peers.clone();
        std::thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let fullname = info.get_fullname().to_string();
                        let addresses: Vec<IpAddr> = info
                            .get_addresses()
                            .iter()
                            .map(|scoped| scoped.to_ip_addr())
                            .collect();
                        let port = info.get_port();
                        let pubkey_hex = info
                            .get_properties()
                            .get("pk")
                            .map(|v| v.val_str().to_string())
                            .filter(|s| !s.is_empty());

                        // Use the instance name portion before the service type.
                        let display_name = fullname
                            .split('.')
                            .next()
                            .unwrap_or("unknown")
                            .to_string();

                        let peer = LocalPeer {
                            name: display_name.clone(),
                            addresses: addresses.clone(),
                            port,
                            pubkey_hex,
                        };

                        log::info!(
                            "mDNS: discovered peer '{}' at {:?}:{}",
                            display_name,
                            addresses,
                            port
                        );

                        peers_clone.lock().unwrap_or_else(|e| e.into_inner()).insert(fullname, peer);
                    }
                    ServiceEvent::ServiceRemoved(_, fullname) => {
                        log::info!("mDNS: peer removed '{fullname}'");
                        peers_clone.lock().unwrap_or_else(|e| e.into_inner()).remove(&fullname);
                    }
                    _ => {}
                }
            }
        });

        Ok(Self { daemon, peers })
    }

    /// Get all currently discovered peers.
    pub fn peers(&self) -> Vec<LocalPeer> {
        self.peers.lock().unwrap_or_else(|e| e.into_inner()).values().cloned().collect()
    }

    /// Stop browsing.
    pub fn stop(&self) {
        if let Err(e) = self.daemon.stop_browse(SERVICE_TYPE) {
            log::warn!("mDNS stop browse failed: {e}");
        }
    }
}

impl Drop for LocalBrowser {
    fn drop(&mut self) {
        self.stop();
    }
}

/// System hostname via POSIX gethostname.
fn simple_hostname() -> String {
    let mut buf = [0u8; 256];
    let result = unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) };
    if result == 0 {
        let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        String::from_utf8_lossy(&buf[..len]).to_string()
    } else {
        // Fallback: environment variables, then a unique default.
        std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| format!("omnidea-{}", std::process::id()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_peer_ws_url() {
        let peer = LocalPeer {
            name: "test".into(),
            addresses: vec!["192.168.1.5".parse().unwrap()],
            port: 8080,
            pubkey_hex: None,
        };
        assert_eq!(peer.ws_url(), Some("ws://192.168.1.5:8080".into()));
    }

    #[test]
    fn local_peer_ws_url_empty() {
        let peer = LocalPeer {
            name: "test".into(),
            addresses: vec![],
            port: 8080,
            pubkey_hex: None,
        };
        assert_eq!(peer.ws_url(), None);
    }

    #[test]
    fn hostname_returns_something() {
        let h = simple_hostname();
        assert!(!h.is_empty());
    }
}
