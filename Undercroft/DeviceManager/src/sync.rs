//! Sync coordination types — tracks which device is "home" for each
//! data type, and the sync state between devices.
//!
//! These are pure data structures for tracking sync progress. The
//! actual sync mechanism (over Globe events) is external to this
//! module.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Maps data types to their "home" device (the authoritative source).
///
/// For example, "contacts" might have its home on the phone, while
/// "documents" might live on the desktop. When conflicts arise, the
/// home device's version takes precedence.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SyncPriority {
    /// Maps `data_type` -> `device_crown_id` (which device is authoritative).
    homes: HashMap<String, String>,
}

impl SyncPriority {
    /// Create an empty priority map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set which device is home for a given data type.
    pub fn set_home(&mut self, data_type: &str, device_crown_id: &str) {
        self.homes
            .insert(data_type.to_string(), device_crown_id.to_string());
    }

    /// Get the home device for a data type, if one is set.
    pub fn home_for(&self, data_type: &str) -> Option<&str> {
        self.homes.get(data_type).map(|s| s.as_str())
    }

    /// Remove the home assignment for a data type.
    pub fn remove(&mut self, data_type: &str) {
        self.homes.remove(data_type);
    }

    /// All home assignments.
    pub fn all(&self) -> &HashMap<String, String> {
        &self.homes
    }
}

/// The sync state of a particular data type on a particular device.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SyncState {
    /// Fully synced as of the given time.
    Synced {
        /// When synchronization was last confirmed.
        at: DateTime<Utc>,
    },
    /// Sync is pending (changes exist that haven't been pushed/pulled).
    Pending {
        /// When the pending state was first detected.
        since: DateTime<Utc>,
    },
    /// A conflict was detected that requires resolution.
    Conflict {
        /// When the conflict was detected.
        detected_at: DateTime<Utc>,
        /// Human-readable description of the conflict.
        description: String,
    },
    /// Sync state is not yet known (initial state).
    Unknown,
}

/// Tracks sync state per device per data type.
///
/// Keyed as `device_crown_id -> data_type -> SyncState`. This is the
/// central registry for understanding what is synced and what isn't
/// across the fleet.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SyncTracker {
    /// `device_crown_id` -> `data_type` -> `SyncState`
    states: HashMap<String, HashMap<String, SyncState>>,
}

impl SyncTracker {
    /// Create an empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the sync state for a device + data type pair.
    pub fn set_state(&mut self, device_crown_id: &str, data_type: &str, state: SyncState) {
        self.states
            .entry(device_crown_id.to_string())
            .or_default()
            .insert(data_type.to_string(), state);
    }

    /// Get the sync state for a device + data type pair.
    ///
    /// Returns [`SyncState::Unknown`] if no state has been recorded.
    pub fn get_state(&self, device_crown_id: &str, data_type: &str) -> &SyncState {
        static UNKNOWN: SyncState = SyncState::Unknown;
        self.states
            .get(device_crown_id)
            .and_then(|inner| inner.get(data_type))
            .unwrap_or(&UNKNOWN)
    }

    /// Get all sync states for a specific device.
    pub fn states_for_device(
        &self,
        device_crown_id: &str,
    ) -> Option<&HashMap<String, SyncState>> {
        self.states.get(device_crown_id)
    }

    /// Returns `true` if every tracked state is [`SyncState::Synced`].
    ///
    /// An empty tracker (no states recorded) returns `true`.
    pub fn all_synced(&self) -> bool {
        self.states.values().all(|inner| {
            inner
                .values()
                .all(|s| matches!(s, SyncState::Synced { .. }))
        })
    }

    /// Returns all conflict states as `(device_crown_id, data_type, state)` triples.
    pub fn conflicts(&self) -> Vec<(&str, &str, &SyncState)> {
        let mut result = Vec::new();
        for (device, inner) in &self.states {
            for (data_type, state) in inner {
                if matches!(state, SyncState::Conflict { .. }) {
                    result.push((device.as_str(), data_type.as_str(), state));
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- SyncPriority tests --

    #[test]
    fn priority_set_and_get() {
        let mut priority = SyncPriority::new();
        priority.set_home("contacts", "cpub_phone");
        priority.set_home("documents", "cpub_desktop");

        assert_eq!(priority.home_for("contacts"), Some("cpub_phone"));
        assert_eq!(priority.home_for("documents"), Some("cpub_desktop"));
        assert_eq!(priority.home_for("photos"), None);
    }

    #[test]
    fn priority_overwrite() {
        let mut priority = SyncPriority::new();
        priority.set_home("contacts", "cpub_phone");
        priority.set_home("contacts", "cpub_desktop");

        assert_eq!(priority.home_for("contacts"), Some("cpub_desktop"));
    }

    #[test]
    fn priority_remove() {
        let mut priority = SyncPriority::new();
        priority.set_home("contacts", "cpub_phone");
        priority.remove("contacts");

        assert_eq!(priority.home_for("contacts"), None);
    }

    #[test]
    fn priority_all() {
        let mut priority = SyncPriority::new();
        priority.set_home("contacts", "cpub_phone");
        priority.set_home("documents", "cpub_desktop");

        let all = priority.all();
        assert_eq!(all.len(), 2);
        assert_eq!(all.get("contacts").map(|s| s.as_str()), Some("cpub_phone"));
    }

    #[test]
    fn priority_serde_round_trip() {
        let mut priority = SyncPriority::new();
        priority.set_home("contacts", "cpub_phone");
        priority.set_home("documents", "cpub_desktop");

        let json = serde_json::to_string(&priority).unwrap();
        let loaded: SyncPriority = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.home_for("contacts"), Some("cpub_phone"));
        assert_eq!(loaded.home_for("documents"), Some("cpub_desktop"));
    }

    // -- SyncState tests --

    #[test]
    fn sync_state_serde_round_trip() {
        let states = vec![
            SyncState::Synced { at: Utc::now() },
            SyncState::Pending { since: Utc::now() },
            SyncState::Conflict {
                detected_at: Utc::now(),
                description: "version diverged".into(),
            },
            SyncState::Unknown,
        ];

        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            let loaded: SyncState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, loaded);
        }
    }

    // -- SyncTracker tests --

    #[test]
    fn tracker_empty_is_all_synced() {
        let tracker = SyncTracker::new();
        assert!(tracker.all_synced());
        assert!(tracker.conflicts().is_empty());
    }

    #[test]
    fn tracker_set_and_get() {
        let mut tracker = SyncTracker::new();
        let now = Utc::now();

        tracker.set_state("cpub_phone", "contacts", SyncState::Synced { at: now });

        assert!(matches!(
            tracker.get_state("cpub_phone", "contacts"),
            SyncState::Synced { .. }
        ));

        // Unknown for untracked entries.
        assert!(matches!(
            tracker.get_state("cpub_phone", "documents"),
            SyncState::Unknown
        ));
        assert!(matches!(
            tracker.get_state("cpub_ghost", "contacts"),
            SyncState::Unknown
        ));
    }

    #[test]
    fn tracker_all_synced() {
        let mut tracker = SyncTracker::new();
        let now = Utc::now();

        tracker.set_state("cpub_phone", "contacts", SyncState::Synced { at: now });
        tracker.set_state("cpub_phone", "documents", SyncState::Synced { at: now });
        tracker.set_state("cpub_desktop", "contacts", SyncState::Synced { at: now });

        assert!(tracker.all_synced());
    }

    #[test]
    fn tracker_not_all_synced_with_pending() {
        let mut tracker = SyncTracker::new();
        let now = Utc::now();

        tracker.set_state("cpub_phone", "contacts", SyncState::Synced { at: now });
        tracker.set_state(
            "cpub_phone",
            "documents",
            SyncState::Pending { since: now },
        );

        assert!(!tracker.all_synced());
    }

    #[test]
    fn tracker_conflicts_detection() {
        let mut tracker = SyncTracker::new();
        let now = Utc::now();

        tracker.set_state("cpub_phone", "contacts", SyncState::Synced { at: now });
        tracker.set_state(
            "cpub_phone",
            "documents",
            SyncState::Conflict {
                detected_at: now,
                description: "merge conflict".into(),
            },
        );
        tracker.set_state(
            "cpub_desktop",
            "settings",
            SyncState::Conflict {
                detected_at: now,
                description: "concurrent edits".into(),
            },
        );

        let conflicts = tracker.conflicts();
        assert_eq!(conflicts.len(), 2);

        // Verify that all returned states are Conflict variants.
        for (_, _, state) in &conflicts {
            assert!(matches!(state, SyncState::Conflict { .. }));
        }
    }

    #[test]
    fn tracker_states_for_device() {
        let mut tracker = SyncTracker::new();
        let now = Utc::now();

        tracker.set_state("cpub_phone", "contacts", SyncState::Synced { at: now });
        tracker.set_state(
            "cpub_phone",
            "documents",
            SyncState::Pending { since: now },
        );

        let phone_states = tracker
            .states_for_device("cpub_phone")
            .expect("phone states should exist");
        assert_eq!(phone_states.len(), 2);
        assert!(phone_states.contains_key("contacts"));
        assert!(phone_states.contains_key("documents"));

        assert!(tracker.states_for_device("cpub_ghost").is_none());
    }

    #[test]
    fn tracker_serde_round_trip() {
        let mut tracker = SyncTracker::new();
        let now = Utc::now();

        tracker.set_state("cpub_phone", "contacts", SyncState::Synced { at: now });
        tracker.set_state(
            "cpub_desktop",
            "documents",
            SyncState::Conflict {
                detected_at: now,
                description: "diverged".into(),
            },
        );

        let json = serde_json::to_string(&tracker).unwrap();
        let loaded: SyncTracker = serde_json::from_str(&json).unwrap();

        assert!(matches!(
            loaded.get_state("cpub_phone", "contacts"),
            SyncState::Synced { .. }
        ));
        assert!(matches!(
            loaded.get_state("cpub_desktop", "documents"),
            SyncState::Conflict { .. }
        ));
    }

    #[test]
    fn tracker_overwrite_state() {
        let mut tracker = SyncTracker::new();
        let now = Utc::now();

        tracker.set_state(
            "cpub_phone",
            "contacts",
            SyncState::Pending { since: now },
        );
        assert!(matches!(
            tracker.get_state("cpub_phone", "contacts"),
            SyncState::Pending { .. }
        ));

        tracker.set_state("cpub_phone", "contacts", SyncState::Synced { at: now });
        assert!(matches!(
            tracker.get_state("cpub_phone", "contacts"),
            SyncState::Synced { .. }
        ));
    }
}
