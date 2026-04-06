//! Decentralized tracker protocol.
//!
//! An OmniTag is a Crown keypair on a chip. It broadcasts its presence via BLE,
//! and any Omnibus node that detects it can report a sighting. The sighting's
//! location is Sentinal-encrypted (opaque bytes here) so only the tag owner and
//! authorized viewers can read it. TagStream provides a capacity-managed history
//! of decrypted sighting locations for authorized consumers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use x::GeoCoordinate;

// ---------------------------------------------------------------------------
// MARK: - TrackerType
// ---------------------------------------------------------------------------

/// The kind of physical tracker device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrackerType {
    OmniTag,
    AppleAirTag,
    Tile,
    SmartTag,
    Custom(String),
}

// ---------------------------------------------------------------------------
// MARK: - OmniTagIdentity
// ---------------------------------------------------------------------------

/// Identity record for a registered OmniTag.
///
/// The tag has its own Crown keypair burned into the chip. The `owner` is the
/// crown_id of the person who registered it; `tag_pubkey` is the crown_id of the tag
/// itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OmniTagIdentity {
    pub id: Uuid,
    pub name: Option<String>,
    /// Owner's crown_id.
    pub owner: String,
    /// The tag's own Crown ID (keypair on chip).
    pub tag_pubkey: String,
    pub created_at: DateTime<Utc>,
    pub active: bool,
}

impl OmniTagIdentity {
    /// Create a new active tag identity with a generated UUID.
    pub fn new(owner: impl Into<String>, tag_pubkey: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: None,
            owner: owner.into(),
            tag_pubkey: tag_pubkey.into(),
            created_at: Utc::now(),
            active: true,
        }
    }

    /// Set a human-readable name for the tag.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Deactivate the tag (e.g. lost, retired).
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Re-activate a previously deactivated tag.
    pub fn activate(&mut self) {
        self.active = true;
    }
}

// ---------------------------------------------------------------------------
// MARK: - TagSighting
// ---------------------------------------------------------------------------

/// A single sighting of a tag by an Omnibus node.
///
/// The location is Sentinal-encrypted — this struct carries the opaque
/// ciphertext. Only the tag owner (and authorized viewers) can decrypt it
/// into a `GeoCoordinate` via Sentinal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TagSighting {
    /// The tag's Crown ID.
    pub tag_pubkey: String,
    /// Sentinal-encrypted location (opaque ciphertext).
    pub encrypted_location: Vec<u8>,
    /// The crown_id of the Omnibus node that detected the tag.
    pub sighter: String,
    pub sighted_at: DateTime<Utc>,
    /// BLE RSSI value (negative dBm). Closer to 0 = stronger signal.
    pub signal_strength: Option<i32>,
}

impl TagSighting {
    /// Create a new sighting with the current timestamp.
    pub fn new(
        tag_pubkey: impl Into<String>,
        encrypted_location: Vec<u8>,
        sighter: impl Into<String>,
    ) -> Self {
        Self {
            tag_pubkey: tag_pubkey.into(),
            encrypted_location,
            sighter: sighter.into(),
            sighted_at: Utc::now(),
            signal_strength: None,
        }
    }

    /// Attach a BLE signal strength reading.
    pub fn with_signal_strength(mut self, rssi: i32) -> Self {
        self.signal_strength = Some(rssi);
        self
    }
}

// ---------------------------------------------------------------------------
// MARK: - TagStreamEntry
// ---------------------------------------------------------------------------

/// A single decrypted location entry in a tag's history stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TagStreamEntry {
    pub location: GeoCoordinate,
    pub sighted_at: DateTime<Utc>,
    /// The crown_id of the node that reported this sighting (if known).
    pub sighter: Option<String>,
}

// ---------------------------------------------------------------------------
// MARK: - TagStream
// ---------------------------------------------------------------------------

/// Capacity-managed history of a tag's decrypted locations.
///
/// Only the tag owner and authorized viewers should populate this from
/// decrypted `TagSighting` data. Entries are stored FIFO and trimmed to
/// `max_entries`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TagStream {
    pub tag_id: Uuid,
    /// Owner's crown_id.
    pub owner: String,
    /// Crown IDs authorized to view this stream (in addition to the owner).
    pub authorized_viewers: Vec<String>,
    pub entries: Vec<TagStreamEntry>,
    pub max_entries: usize,
}

impl TagStream {
    /// Create a new empty stream with default capacity (100 entries).
    pub fn new(tag_id: Uuid, owner: impl Into<String>) -> Self {
        Self {
            tag_id,
            owner: owner.into(),
            authorized_viewers: Vec::new(),
            entries: Vec::new(),
            max_entries: 100,
        }
    }

    /// Append a location entry. If the stream exceeds `max_entries`, the
    /// oldest entry is removed (FIFO).
    pub fn add_entry(&mut self, entry: TagStreamEntry) {
        self.entries.push(entry);
        while self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    /// Grant view access to an crown_id.
    pub fn add_viewer(&mut self, crown_id: impl Into<String>) {
        let crown_id = crown_id.into();
        if !self.authorized_viewers.contains(&crown_id) {
            self.authorized_viewers.push(crown_id);
        }
    }

    /// Revoke view access from an crown_id.
    pub fn remove_viewer(&mut self, crown_id: &str) {
        self.authorized_viewers.retain(|v| v != crown_id);
    }

    /// Check whether `viewer` is authorized. The owner is always authorized.
    pub fn is_authorized(&self, viewer: &str) -> bool {
        self.owner == viewer || self.authorized_viewers.iter().any(|v| v == viewer)
    }

    /// The most recent entry, if any.
    pub fn latest(&self) -> Option<&TagStreamEntry> {
        self.entries.last()
    }

    /// All entries sighted at or after `since`.
    pub fn entries_since(&self, since: DateTime<Utc>) -> Vec<&TagStreamEntry> {
        self.entries
            .iter()
            .filter(|e| e.sighted_at >= since)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// MARK: - Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    // -- OmniTagIdentity ----------------------------------------------------

    #[test]
    fn new_tag_is_active() {
        let tag = OmniTagIdentity::new("cpub1owner", "cpub1tag");
        assert!(tag.active);
        assert!(tag.name.is_none());
        assert_eq!(tag.owner, "cpub1owner");
        assert_eq!(tag.tag_pubkey, "cpub1tag");
    }

    #[test]
    fn with_name_sets_name() {
        let tag = OmniTagIdentity::new("cpub1owner", "cpub1tag").with_name("Backpack");
        assert_eq!(tag.name.as_deref(), Some("Backpack"));
    }

    #[test]
    fn deactivate_and_activate() {
        let mut tag = OmniTagIdentity::new("cpub1owner", "cpub1tag");
        tag.deactivate();
        assert!(!tag.active);
        tag.activate();
        assert!(tag.active);
    }

    #[test]
    fn tag_identity_uuid_is_v4() {
        let tag = OmniTagIdentity::new("cpub1owner", "cpub1tag");
        assert_eq!(tag.id.get_version_num(), 4);
    }

    #[test]
    fn tag_identity_serde_round_trip() {
        let tag = OmniTagIdentity::new("cpub1owner", "cpub1tag").with_name("Keys");
        let json = serde_json::to_string(&tag).unwrap();
        let parsed: OmniTagIdentity = serde_json::from_str(&json).unwrap();
        assert_eq!(tag, parsed);
    }

    // -- TagSighting --------------------------------------------------------

    #[test]
    fn new_sighting_auto_timestamp() {
        let before = Utc::now();
        let s = TagSighting::new("cpub1tag", vec![1, 2, 3], "cpub1sighter");
        let after = Utc::now();
        assert!(s.sighted_at >= before && s.sighted_at <= after);
        assert_eq!(s.tag_pubkey, "cpub1tag");
        assert_eq!(s.encrypted_location, vec![1, 2, 3]);
        assert_eq!(s.sighter, "cpub1sighter");
        assert!(s.signal_strength.is_none());
    }

    #[test]
    fn sighting_with_signal_strength() {
        let s = TagSighting::new("cpub1tag", vec![0xAB], "cpub1node").with_signal_strength(-72);
        assert_eq!(s.signal_strength, Some(-72));
    }

    #[test]
    fn sighting_serde_round_trip() {
        let s = TagSighting::new("cpub1tag", vec![10, 20, 30], "cpub1node")
            .with_signal_strength(-55);
        let json = serde_json::to_string(&s).unwrap();
        let parsed: TagSighting = serde_json::from_str(&json).unwrap();
        assert_eq!(s, parsed);
    }

    // -- TagStream ----------------------------------------------------------

    #[test]
    fn new_stream_is_empty() {
        let tag_id = Uuid::new_v4();
        let stream = TagStream::new(tag_id, "cpub1owner");
        assert_eq!(stream.tag_id, tag_id);
        assert_eq!(stream.owner, "cpub1owner");
        assert!(stream.entries.is_empty());
        assert!(stream.authorized_viewers.is_empty());
        assert_eq!(stream.max_entries, 100);
    }

    #[test]
    fn add_entry_and_latest() {
        let mut stream = TagStream::new(Uuid::new_v4(), "cpub1owner");
        let coord = GeoCoordinate::new(39.7392, -104.9903).unwrap();
        let entry = TagStreamEntry {
            location: coord,
            sighted_at: Utc::now(),
            sighter: Some("cpub1node".into()),
        };
        stream.add_entry(entry.clone());
        assert_eq!(stream.entries.len(), 1);
        assert_eq!(stream.latest().unwrap().location, coord);
    }

    #[test]
    fn fifo_eviction_at_max_entries() {
        let mut stream = TagStream::new(Uuid::new_v4(), "cpub1owner");
        stream.max_entries = 3;

        for i in 0..5 {
            let coord = GeoCoordinate::new(i as f64, 0.0).unwrap();
            stream.add_entry(TagStreamEntry {
                location: coord,
                sighted_at: Utc::now(),
                sighter: None,
            });
        }

        assert_eq!(stream.entries.len(), 3);
        // Oldest (lat=0, lat=1) should be evicted; lat=2,3,4 remain
        assert_eq!(stream.entries[0].location.latitude, 2.0);
        assert_eq!(stream.entries[1].location.latitude, 3.0);
        assert_eq!(stream.entries[2].location.latitude, 4.0);
    }

    #[test]
    fn viewer_authorization() {
        let mut stream = TagStream::new(Uuid::new_v4(), "cpub1owner");

        // Owner is always authorized
        assert!(stream.is_authorized("cpub1owner"));
        assert!(!stream.is_authorized("cpub1alice"));

        stream.add_viewer("cpub1alice");
        assert!(stream.is_authorized("cpub1alice"));

        // Duplicate add is a no-op
        stream.add_viewer("cpub1alice");
        assert_eq!(stream.authorized_viewers.len(), 1);

        stream.remove_viewer("cpub1alice");
        assert!(!stream.is_authorized("cpub1alice"));
    }

    #[test]
    fn entries_since_filters_correctly() {
        let mut stream = TagStream::new(Uuid::new_v4(), "cpub1owner");
        let now = Utc::now();
        let old_time = now - Duration::hours(2);
        let recent_time = now - Duration::minutes(30);

        let old_entry = TagStreamEntry {
            location: GeoCoordinate::new(39.0, -105.0).unwrap(),
            sighted_at: old_time,
            sighter: None,
        };
        let recent_entry = TagStreamEntry {
            location: GeoCoordinate::new(40.0, -105.0).unwrap(),
            sighted_at: recent_time,
            sighter: None,
        };

        stream.add_entry(old_entry);
        stream.add_entry(recent_entry);

        let cutoff = now - Duration::hours(1);
        let recent = stream.entries_since(cutoff);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].location.latitude, 40.0);

        // All entries since the beginning
        let all = stream.entries_since(old_time);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn latest_on_empty_stream_is_none() {
        let stream = TagStream::new(Uuid::new_v4(), "cpub1owner");
        assert!(stream.latest().is_none());
    }

    #[test]
    fn stream_serde_round_trip() {
        let mut stream = TagStream::new(Uuid::new_v4(), "cpub1owner");
        stream.add_viewer("cpub1alice");
        stream.add_entry(TagStreamEntry {
            location: GeoCoordinate::new(39.7392, -104.9903).unwrap(),
            sighted_at: Utc::now(),
            sighter: Some("cpub1node".into()),
        });

        let json = serde_json::to_string(&stream).unwrap();
        let parsed: TagStream = serde_json::from_str(&json).unwrap();
        assert_eq!(stream, parsed);
    }

    // -- TrackerType --------------------------------------------------------

    #[test]
    fn tracker_type_serde_round_trip() {
        let types = vec![
            TrackerType::OmniTag,
            TrackerType::AppleAirTag,
            TrackerType::Tile,
            TrackerType::SmartTag,
            TrackerType::Custom("Chipolo".into()),
        ];
        for t in &types {
            let json = serde_json::to_string(t).unwrap();
            let parsed: TrackerType = serde_json::from_str(&json).unwrap();
            assert_eq!(*t, parsed);
        }
    }
}
