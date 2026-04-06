use serde::{Deserialize, Serialize};

/// What kind of device is running this Omny.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceType {
    /// Desktop/laptop — always-on backbone node.
    Desktop,
    /// Phone — intermittent, battery-constrained.
    Mobile,
    /// Community anchor — dedicated serving node.
    CommunityAnchor,
}

/// Current conditions affecting relay behavior.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceCondition {
    /// Battery level (0-100). `None` if plugged in / desktop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery_percent: Option<u8>,
    /// Whether the device is on WiFi, cellular, or ethernet.
    pub connection: ConnectionType,
    /// Whether the device is currently charging.
    pub charging: bool,
    /// Available storage in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_available: Option<u64>,
}

/// Network connection type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionType {
    Ethernet,
    WiFi,
    Cellular,
    Unknown,
}

/// How aggressively this device should serve content.
///
/// Computed from DeviceType + DeviceCondition. The Omny runtime
/// evaluates this periodically and adjusts relay behavior.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceProfile {
    /// Device type (static, set at install).
    pub device_type: DeviceType,
    /// Human-readable device name.
    pub device_name: String,
    /// Current serving policy.
    pub policy: ServingPolicy,
}

/// Relay serving policy — how much this device contributes to the network.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServingPolicy {
    /// Full serving: accept connections, cache content, forward events.
    /// Desktop on Ethernet, Community Anchors always.
    Full,
    /// Normal serving: accept connections, limited caching.
    /// Desktop on WiFi, Mobile on WiFi with good battery.
    Normal,
    /// Minimal serving: maintain existing connections, no new ones.
    /// Mobile on cellular, or low battery.
    Minimal,
    /// Dormant: only maintain connection to own desktop Omny.
    /// Below 20% battery, or cellular with low data.
    Dormant,
}

impl DeviceProfile {
    /// Compute serving policy from device type and current conditions.
    pub fn compute_policy(device_type: DeviceType, condition: &DeviceCondition) -> ServingPolicy {
        match device_type {
            DeviceType::CommunityAnchor => ServingPolicy::Full,
            DeviceType::Desktop => match condition.connection {
                ConnectionType::Ethernet => ServingPolicy::Full,
                ConnectionType::WiFi => ServingPolicy::Normal,
                _ => ServingPolicy::Minimal,
            },
            DeviceType::Mobile => {
                // Battery check first.
                if let Some(battery) = condition.battery_percent {
                    if battery < 20 && !condition.charging {
                        return ServingPolicy::Dormant;
                    }
                }
                match condition.connection {
                    ConnectionType::WiFi => {
                        if condition.charging {
                            ServingPolicy::Normal
                        } else {
                            ServingPolicy::Minimal
                        }
                    }
                    ConnectionType::Cellular => ServingPolicy::Dormant,
                    _ => ServingPolicy::Minimal,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn community_anchor_always_full() {
        let condition = DeviceCondition {
            battery_percent: None,
            connection: ConnectionType::Ethernet,
            charging: true,
            storage_available: None,
        };
        assert_eq!(
            DeviceProfile::compute_policy(DeviceType::CommunityAnchor, &condition),
            ServingPolicy::Full
        );
    }

    #[test]
    fn desktop_ethernet_full() {
        let condition = DeviceCondition {
            battery_percent: None,
            connection: ConnectionType::Ethernet,
            charging: true,
            storage_available: None,
        };
        assert_eq!(
            DeviceProfile::compute_policy(DeviceType::Desktop, &condition),
            ServingPolicy::Full
        );
    }

    #[test]
    fn desktop_wifi_normal() {
        let condition = DeviceCondition {
            battery_percent: None,
            connection: ConnectionType::WiFi,
            charging: true,
            storage_available: None,
        };
        assert_eq!(
            DeviceProfile::compute_policy(DeviceType::Desktop, &condition),
            ServingPolicy::Normal
        );
    }

    #[test]
    fn mobile_wifi_charging_normal() {
        let condition = DeviceCondition {
            battery_percent: Some(80),
            connection: ConnectionType::WiFi,
            charging: true,
            storage_available: None,
        };
        assert_eq!(
            DeviceProfile::compute_policy(DeviceType::Mobile, &condition),
            ServingPolicy::Normal
        );
    }

    #[test]
    fn mobile_wifi_not_charging_minimal() {
        let condition = DeviceCondition {
            battery_percent: Some(80),
            connection: ConnectionType::WiFi,
            charging: false,
            storage_available: None,
        };
        assert_eq!(
            DeviceProfile::compute_policy(DeviceType::Mobile, &condition),
            ServingPolicy::Minimal
        );
    }

    #[test]
    fn mobile_cellular_dormant() {
        let condition = DeviceCondition {
            battery_percent: Some(80),
            connection: ConnectionType::Cellular,
            charging: false,
            storage_available: None,
        };
        assert_eq!(
            DeviceProfile::compute_policy(DeviceType::Mobile, &condition),
            ServingPolicy::Dormant
        );
    }

    #[test]
    fn mobile_low_battery_dormant() {
        let condition = DeviceCondition {
            battery_percent: Some(15),
            connection: ConnectionType::WiFi,
            charging: false,
            storage_available: None,
        };
        assert_eq!(
            DeviceProfile::compute_policy(DeviceType::Mobile, &condition),
            ServingPolicy::Dormant
        );
    }

    #[test]
    fn mobile_low_battery_but_charging_not_dormant() {
        let condition = DeviceCondition {
            battery_percent: Some(15),
            connection: ConnectionType::WiFi,
            charging: true,
            storage_available: None,
        };
        // Charging overrides low battery — gets Normal (WiFi + charging).
        assert_eq!(
            DeviceProfile::compute_policy(DeviceType::Mobile, &condition),
            ServingPolicy::Normal
        );
    }

    #[test]
    fn device_profile_serde() {
        let profile = DeviceProfile {
            device_type: DeviceType::Desktop,
            device_name: "Sam's MacBook Pro".into(),
            policy: ServingPolicy::Full,
        };
        let json = serde_json::to_string(&profile).unwrap();
        let loaded: DeviceProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.device_type, DeviceType::Desktop);
        assert_eq!(loaded.policy, ServingPolicy::Full);
    }

    #[test]
    fn device_type_serde_camel_case() {
        assert_eq!(
            serde_json::to_string(&DeviceType::CommunityAnchor).unwrap(),
            "\"communityAnchor\""
        );
    }

    #[test]
    fn connection_type_serde() {
        for ct in [
            ConnectionType::Ethernet,
            ConnectionType::WiFi,
            ConnectionType::Cellular,
            ConnectionType::Unknown,
        ] {
            let json = serde_json::to_string(&ct).unwrap();
            let loaded: ConnectionType = serde_json::from_str(&json).unwrap();
            assert_eq!(loaded, ct);
        }
    }
}
