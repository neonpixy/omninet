use serde::{Deserialize, Serialize};

/// A device's network addresses — what it knows about how to reach itself.
///
/// Collected automatically by the Omny runtime. Local IPs from the OS,
/// public IP from UPnP (if available), relay URLs from configuration.
/// Before publishing as relay hints, all addresses are encrypted with
/// the Network Key so they're readable only by Omnidea participants.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AddressInfo {
    /// Local network addresses (e.g. "192.168.1.42:8080").
    /// Discovered via OS network interfaces.
    pub local: Vec<String>,
    /// Public address from UPnP port mapping (e.g. "73.45.123.89:8080").
    /// Only present if UPnP succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<String>,
    /// Relay URLs this device connects to (e.g. "ws://relay.example.com").
    pub relay_urls: Vec<String>,
}

impl AddressInfo {
    /// All addresses combined (local + public + relay URLs).
    pub fn all_addresses(&self) -> Vec<String> {
        let mut addrs = self.local.clone();
        if let Some(public) = &self.public {
            addrs.push(public.clone());
        }
        addrs.extend(self.relay_urls.iter().cloned());
        addrs
    }

    /// Whether this device has any reachable address.
    pub fn is_reachable(&self) -> bool {
        !self.local.is_empty() || self.public.is_some() || !self.relay_urls.is_empty()
    }
}

/// Encrypted address payload for relay hints.
///
/// The `ciphertext` contains an AddressInfo encrypted with the Network Key
/// (AES-256-GCM via Sentinal) and optionally babelized (Lingo).
/// The `key_version` tells the reader which Network Key to use.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedAddresses {
    /// Network Key version used for encryption.
    pub key_version: u32,
    /// Encrypted + babelized address data (base64).
    pub ciphertext: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_info_defaults() {
        let info = AddressInfo::default();
        assert!(info.local.is_empty());
        assert!(info.public.is_none());
        assert!(info.relay_urls.is_empty());
        assert!(!info.is_reachable());
    }

    #[test]
    fn address_info_with_local() {
        let info = AddressInfo {
            local: vec!["192.168.1.42:8080".into()],
            ..Default::default()
        };
        assert!(info.is_reachable());
        assert_eq!(info.all_addresses().len(), 1);
    }

    #[test]
    fn address_info_all_addresses() {
        let info = AddressInfo {
            local: vec!["192.168.1.42:8080".into(), "10.0.0.5:8080".into()],
            public: Some("73.45.123.89:8080".into()),
            relay_urls: vec!["ws://relay.example.com".into()],
        };
        let all = info.all_addresses();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&"73.45.123.89:8080".to_string()));
        assert!(all.contains(&"ws://relay.example.com".to_string()));
    }

    #[test]
    fn address_info_serde() {
        let info = AddressInfo {
            local: vec!["192.168.1.42:8080".into()],
            public: Some("73.45.123.89:8080".into()),
            relay_urls: vec!["ws://relay.example.com".into()],
        };
        let json = serde_json::to_string(&info).unwrap();
        let loaded: AddressInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.local, info.local);
        assert_eq!(loaded.public, info.public);
    }

    #[test]
    fn address_info_skips_none_public() {
        let info = AddressInfo {
            local: vec!["192.168.1.42:8080".into()],
            public: None,
            relay_urls: vec![],
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(!json.contains("public"));
    }

    #[test]
    fn encrypted_addresses_serde() {
        let enc = EncryptedAddresses {
            key_version: 1,
            ciphertext: "base64encrypteddata".into(),
        };
        let json = serde_json::to_string(&enc).unwrap();
        let loaded: EncryptedAddresses = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.key_version, 1);
        assert_eq!(loaded.ciphertext, "base64encrypteddata");
    }

    #[test]
    fn reachable_via_public_only() {
        let info = AddressInfo {
            local: vec![],
            public: Some("73.45.123.89:8080".into()),
            relay_urls: vec![],
        };
        assert!(info.is_reachable());
    }

    #[test]
    fn reachable_via_relay_only() {
        let info = AddressInfo {
            local: vec![],
            public: None,
            relay_urls: vec!["ws://relay.example.com".into()],
        };
        assert!(info.is_reachable());
    }
}
