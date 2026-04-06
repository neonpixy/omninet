//! Device fleet management — tracks all paired devices and their status.
//!
//! A [`DeviceFleet`] is the in-memory registry of all devices that have
//! been paired with this identity. Each device is tracked as a
//! [`FleetEntry`] with its pairing data, optional profile, and current
//! [`DeviceStatus`].

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use globe::discovery::pairing::DevicePair;
use globe::discovery::profile::DeviceProfile;
use serde::{Deserialize, Serialize};

use crate::error::DeviceManagerError;

/// A single device in the fleet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FleetEntry {
    /// Unique device identifier (Crown ID from pairing).
    pub device_crown_id: String,
    /// Human-readable device name.
    pub device_name: String,
    /// The verified pairing record.
    pub pair: DevicePair,
    /// Optional device profile (type, serving policy, etc.).
    pub profile: Option<DeviceProfile>,
    /// Current device status.
    pub status: DeviceStatus,
    /// When this device was paired.
    pub paired_at: DateTime<Utc>,
}

/// Current status of a paired device.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceStatus {
    /// When this device was last seen.
    pub last_seen: DateTime<Utc>,
    /// Whether the device is currently online.
    pub online: bool,
    /// Current serving policy name (from `ServingPolicy`), if known.
    pub serving_policy: Option<String>,
    /// Current connection type name (from `ConnectionType`), if known.
    pub connection_type: Option<String>,
    /// Battery percentage (0-100), if available.
    pub battery_percent: Option<u8>,
}

/// Aggregate health summary of the device fleet.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FleetHealth {
    /// Total number of paired devices.
    pub total_devices: usize,
    /// Number of devices currently online.
    pub online_devices: usize,
    /// Number of devices currently offline.
    pub offline_devices: usize,
    /// Whether all devices are synced (determined externally via SyncTracker).
    pub all_synced: bool,
}

/// Registry of all paired devices.
///
/// Devices are keyed by their Crown ID. The fleet is an in-memory
/// data structure — persistence is handled by the caller (Vault).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DeviceFleet {
    devices: HashMap<String, FleetEntry>,
}

impl DeviceFleet {
    /// Create an empty fleet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a device to the fleet.
    ///
    /// # Errors
    ///
    /// Returns [`DeviceManagerError::AlreadyPaired`] if a device with
    /// the same crown_id already exists in the fleet.
    pub fn add(&mut self, entry: FleetEntry) -> Result<(), DeviceManagerError> {
        if self.devices.contains_key(&entry.device_crown_id) {
            return Err(DeviceManagerError::AlreadyPaired(
                entry.device_crown_id.clone(),
            ));
        }
        self.devices.insert(entry.device_crown_id.clone(), entry);
        Ok(())
    }

    /// Remove a device from the fleet, returning it if it existed.
    pub fn remove(&mut self, device_crown_id: &str) -> Option<FleetEntry> {
        self.devices.remove(device_crown_id)
    }

    /// Get an immutable reference to a device by crown_id.
    pub fn get(&self, device_crown_id: &str) -> Option<&FleetEntry> {
        self.devices.get(device_crown_id)
    }

    /// Get a mutable reference to a device by crown_id.
    pub fn get_mut(&mut self, device_crown_id: &str) -> Option<&mut FleetEntry> {
        self.devices.get_mut(device_crown_id)
    }

    /// List all devices in the fleet (unordered).
    pub fn list(&self) -> Vec<&FleetEntry> {
        self.devices.values().collect()
    }

    /// Update the status of a device.
    ///
    /// # Errors
    ///
    /// Returns [`DeviceManagerError::DeviceNotFound`] if no device with
    /// the given crown_id exists in the fleet.
    pub fn update_status(
        &mut self,
        device_crown_id: &str,
        status: DeviceStatus,
    ) -> Result<(), DeviceManagerError> {
        let entry = self
            .devices
            .get_mut(device_crown_id)
            .ok_or_else(|| DeviceManagerError::DeviceNotFound(device_crown_id.to_string()))?;
        entry.status = status;
        Ok(())
    }

    /// Compute the aggregate health of the fleet.
    pub fn health(&self) -> FleetHealth {
        let total_devices = self.devices.len();
        let online_devices = self.devices.values().filter(|e| e.status.online).count();
        let offline_devices = total_devices - online_devices;

        FleetHealth {
            total_devices,
            online_devices,
            offline_devices,
            // Sync state is tracked externally by SyncTracker; we default
            // to true for an empty fleet and false otherwise. The caller
            // should populate this from SyncTracker.all_synced().
            all_synced: total_devices == 0,
        }
    }

    /// Number of devices in the fleet.
    pub fn count(&self) -> usize {
        self.devices.len()
    }

    /// Whether the fleet has no devices.
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pair(name: &str) -> DevicePair {
        DevicePair {
            identity: format!("cpub_{name}"),
            local_device: "Test Desktop".into(),
            remote_device: name.into(),
            remote_relay_url: "ws://localhost:8080".into(),
            paired_at: Utc::now().timestamp(),
            active: true,
        }
    }

    fn make_status(online: bool) -> DeviceStatus {
        DeviceStatus {
            last_seen: Utc::now(),
            online,
            serving_policy: None,
            connection_type: None,
            battery_percent: None,
        }
    }

    fn make_entry(crown_id: &str, name: &str, online: bool) -> FleetEntry {
        FleetEntry {
            device_crown_id: crown_id.to_string(),
            device_name: name.to_string(),
            pair: make_pair(name),
            profile: None,
            status: make_status(online),
            paired_at: Utc::now(),
        }
    }

    #[test]
    fn new_fleet_is_empty() {
        let fleet = DeviceFleet::new();
        assert!(fleet.is_empty());
        assert_eq!(fleet.count(), 0);
    }

    #[test]
    fn add_and_get() {
        let mut fleet = DeviceFleet::new();
        let entry = make_entry("cpub_phone", "Phone", true);
        fleet.add(entry).expect("first add should succeed");

        assert_eq!(fleet.count(), 1);
        assert!(!fleet.is_empty());

        let got = fleet.get("cpub_phone").expect("device should exist");
        assert_eq!(got.device_name, "Phone");
    }

    #[test]
    fn add_duplicate_fails() {
        let mut fleet = DeviceFleet::new();
        let entry1 = make_entry("cpub_phone", "Phone", true);
        let entry2 = make_entry("cpub_phone", "Phone v2", true);

        fleet.add(entry1).expect("first add should succeed");
        let result = fleet.add(entry2);

        assert!(matches!(
            result,
            Err(DeviceManagerError::AlreadyPaired(ref crown_id)) if crown_id == "cpub_phone"
        ));
    }

    #[test]
    fn remove_device() {
        let mut fleet = DeviceFleet::new();
        fleet
            .add(make_entry("cpub_phone", "Phone", true))
            .unwrap();

        let removed = fleet.remove("cpub_phone");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().device_name, "Phone");
        assert!(fleet.is_empty());

        // Removing again returns None.
        assert!(fleet.remove("cpub_phone").is_none());
    }

    #[test]
    fn list_all_devices() {
        let mut fleet = DeviceFleet::new();
        fleet
            .add(make_entry("cpub_phone", "Phone", true))
            .unwrap();
        fleet
            .add(make_entry("cpub_tablet", "Tablet", false))
            .unwrap();

        let entries = fleet.list();
        assert_eq!(entries.len(), 2);

        let names: Vec<&str> = entries.iter().map(|e| e.device_name.as_str()).collect();
        assert!(names.contains(&"Phone"));
        assert!(names.contains(&"Tablet"));
    }

    #[test]
    fn update_status() {
        let mut fleet = DeviceFleet::new();
        fleet
            .add(make_entry("cpub_phone", "Phone", false))
            .unwrap();

        assert!(!fleet.get("cpub_phone").unwrap().status.online);

        fleet
            .update_status("cpub_phone", make_status(true))
            .expect("update should succeed");

        assert!(fleet.get("cpub_phone").unwrap().status.online);
    }

    #[test]
    fn update_status_not_found() {
        let mut fleet = DeviceFleet::new();
        let result = fleet.update_status("cpub_ghost", make_status(true));

        assert!(matches!(
            result,
            Err(DeviceManagerError::DeviceNotFound(ref crown_id)) if crown_id == "cpub_ghost"
        ));
    }

    #[test]
    fn health_empty_fleet() {
        let fleet = DeviceFleet::new();
        let health = fleet.health();

        assert_eq!(
            health,
            FleetHealth {
                total_devices: 0,
                online_devices: 0,
                offline_devices: 0,
                all_synced: true,
            }
        );
    }

    #[test]
    fn health_mixed_fleet() {
        let mut fleet = DeviceFleet::new();
        fleet
            .add(make_entry("cpub_phone", "Phone", true))
            .unwrap();
        fleet
            .add(make_entry("cpub_tablet", "Tablet", false))
            .unwrap();
        fleet
            .add(make_entry("cpub_desktop", "Desktop", true))
            .unwrap();

        let health = fleet.health();

        assert_eq!(health.total_devices, 3);
        assert_eq!(health.online_devices, 2);
        assert_eq!(health.offline_devices, 1);
        assert!(!health.all_synced);
    }

    #[test]
    fn get_mut_allows_modification() {
        let mut fleet = DeviceFleet::new();
        fleet
            .add(make_entry("cpub_phone", "Phone", false))
            .unwrap();

        let entry = fleet.get_mut("cpub_phone").expect("device should exist");
        entry.device_name = "Updated Phone".to_string();

        assert_eq!(
            fleet.get("cpub_phone").unwrap().device_name,
            "Updated Phone"
        );
    }

    #[test]
    fn fleet_serde_round_trip() {
        let mut fleet = DeviceFleet::new();
        fleet
            .add(make_entry("cpub_phone", "Phone", true))
            .unwrap();
        fleet
            .add(make_entry("cpub_tablet", "Tablet", false))
            .unwrap();

        let json = serde_json::to_string(&fleet).expect("serialize should succeed");
        let loaded: DeviceFleet =
            serde_json::from_str(&json).expect("deserialize should succeed");

        assert_eq!(loaded.count(), 2);
        assert!(loaded.get("cpub_phone").is_some());
        assert!(loaded.get("cpub_tablet").is_some());
    }
}
