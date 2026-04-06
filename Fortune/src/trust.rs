use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A commons trust — collective stewardship of shared resources.
///
/// From Conjunction Art. 6 §1: "This Covenant hereby abolishes the doctrine
/// of absolute ownership. In its place, it affirms Stewardship as the lawful
/// mode of relation between Persons and possessions."
///
/// From Consortium Art. 5 §1: "Consortia shall make use of material, biological,
/// informational, and energetic resources only in ways that uphold the regenerative,
/// collective, and interdependent ethos of this Covenant."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommonsTrust {
    pub id: Uuid,
    pub name: String,
    pub trust_type: TrustType,
    pub stewards: Vec<String>,
    pub assets: Vec<TrustAsset>,
    pub stewardship_records: Vec<StewardshipRecord>,
    pub created_at: DateTime<Utc>,
}

impl CommonsTrust {
    /// Create a new commons trust with no stewards or assets.
    pub fn new(name: impl Into<String>, trust_type: TrustType) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            trust_type,
            stewards: Vec::new(),
            assets: Vec::new(),
            stewardship_records: Vec::new(),
            created_at: Utc::now(),
        }
    }

    /// Add a steward to this trust. Duplicates are ignored.
    pub fn add_steward(&mut self, pubkey: impl Into<String>) {
        let pubkey = pubkey.into();
        if !self.stewards.contains(&pubkey) {
            self.stewards.push(pubkey);
        }
    }

    /// Remove a steward from this trust.
    pub fn remove_steward(&mut self, pubkey: &str) {
        self.stewards.retain(|s| s != pubkey);
    }

    /// Check if a person is a steward of this trust.
    pub fn is_steward(&self, pubkey: &str) -> bool {
        self.stewards.contains(&pubkey.to_string())
    }

    /// Add an asset to this trust's holdings.
    pub fn add_asset(&mut self, asset: TrustAsset) {
        self.assets.push(asset);
    }

    /// Record a stewardship action (maintenance, improvement, etc.).
    pub fn record_stewardship(&mut self, record: StewardshipRecord) {
        self.stewardship_records.push(record);
    }

    /// Number of stewards managing this trust.
    pub fn steward_count(&self) -> usize {
        self.stewards.len()
    }

    /// Number of assets held in this trust.
    pub fn asset_count(&self) -> usize {
        self.assets.len()
    }
}

/// What kind of commons the trust manages.
///
/// From Conjunction Art. 6 §4: "Creative and intellectual works shall be
/// honored through attribution and access. The Commons holds the memory
/// of the world. No one shall fence it off."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TrustType {
    /// Land, water, habitat.
    Land,
    /// Shared tools, equipment, machinery.
    Tools,
    /// Shared knowledge, curricula, research.
    Knowledge,
    /// Roads, buildings, networks, energy.
    Infrastructure,
    /// Art, music, language, history.
    Cultural,
}

/// An asset held in trust.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustAsset {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub asset_type: String,
    pub added_at: DateTime<Utc>,
    pub added_by: String,
}

impl TrustAsset {
    /// Create a new trust asset with a name, description, type, and the person who added it.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        asset_type: impl Into<String>,
        added_by: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: description.into(),
            asset_type: asset_type.into(),
            added_at: Utc::now(),
            added_by: added_by.into(),
        }
    }
}

/// A record of stewardship activity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StewardshipRecord {
    pub id: Uuid,
    pub steward: String,
    pub action: String,
    pub asset_id: Option<Uuid>,
    pub occurred_at: DateTime<Utc>,
}

impl StewardshipRecord {
    /// Create a new stewardship record for an action taken by a steward.
    pub fn new(
        steward: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            steward: steward.into(),
            action: action.into(),
            asset_id: None,
            occurred_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_commons_trust() {
        let trust = CommonsTrust::new("Community Garden", TrustType::Land);
        assert_eq!(trust.name, "Community Garden");
        assert_eq!(trust.trust_type, TrustType::Land);
        assert_eq!(trust.steward_count(), 0);
    }

    #[test]
    fn steward_management() {
        let mut trust = CommonsTrust::new("Tool Library", TrustType::Tools);
        trust.add_steward("alice");
        trust.add_steward("bob");
        trust.add_steward("alice"); // duplicate ignored

        assert_eq!(trust.steward_count(), 2);
        assert!(trust.is_steward("alice"));

        trust.remove_steward("alice");
        assert!(!trust.is_steward("alice"));
        assert_eq!(trust.steward_count(), 1);
    }

    #[test]
    fn asset_tracking() {
        let mut trust = CommonsTrust::new("Knowledge Commons", TrustType::Knowledge);
        trust.add_asset(TrustAsset::new(
            "Permaculture Guide",
            "Comprehensive bioregional planting guide",
            "document",
            "alice",
        ));
        trust.add_asset(TrustAsset::new(
            "Water Systems Manual",
            "Greywater and rainwater collection designs",
            "document",
            "bob",
        ));
        assert_eq!(trust.asset_count(), 2);
    }

    #[test]
    fn stewardship_recording() {
        let mut trust = CommonsTrust::new("Solar Grid", TrustType::Infrastructure);
        trust.add_steward("alice");
        trust.record_stewardship(StewardshipRecord::new(
            "alice",
            "Performed quarterly maintenance on Panel Array A",
        ));
        assert_eq!(trust.stewardship_records.len(), 1);
    }

    #[test]
    fn all_trust_types() {
        let types = [
            TrustType::Land,
            TrustType::Tools,
            TrustType::Knowledge,
            TrustType::Infrastructure,
            TrustType::Cultural,
        ];
        assert_eq!(types.len(), 5);
    }
}
