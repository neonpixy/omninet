use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A constitutional duty — a binding obligation that arises from consciousness and kinship.
///
/// From Covenant Core Art. 4: "The duty of the people to uphold the dignity of every person
/// and act in accordance with justice shall be binding."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Duty {
    pub id: Uuid,
    pub category: DutyCategory,
    pub name: String,
    pub description: String,
    pub binding_level: BindingLevel,
    pub applies_to: DutyScope,
    pub is_immutable: bool,
    pub source: String,
    pub created_at: DateTime<Utc>,
}

impl Duty {
    /// Create a new duty with a fresh UUID and the current timestamp.
    pub fn new(
        category: DutyCategory,
        name: impl Into<String>,
        description: impl Into<String>,
        binding_level: BindingLevel,
        applies_to: DutyScope,
        is_immutable: bool,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            category,
            name: name.into(),
            description: description.into(),
            binding_level,
            applies_to,
            is_immutable,
            source: source.into(),
            created_at: Utc::now(),
        }
    }
}

/// Categories of duties, derived from Covenant Core Art. 4 and 7.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DutyCategory {
    /// Core Art. 4 Section 1 — uphold dignity and justice
    UpholdDignity,
    /// Core Art. 4 Section 2 — carry forward memory, preserve language and lineage
    Remember,
    /// Core Art. 4 Section 3 — protect Earth and living systems
    Steward,
    /// Core Art. 4 Section 4 — refuse unlawful governance, reconstitute just structures
    RefuseAndReconstitute,
    /// Core Art. 7 Section 1 — communities uphold the Covenant
    CommunityFidelity,
    /// Core Art. 7 Section 2 — mutual aid and solidarity
    MutualAid,
    /// Core Art. 7 Section 3 — ecological stewardship by communities
    EcologicalStewardship,
    /// Core Art. 7 Section 4 — challenge breaches, reconstitute where harm occurred
    PublicChallenge,
}

/// How strongly a duty binds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BindingLevel {
    /// Aspirational — guides behavior but not enforceable
    Aspirational,
    /// Obligatory — enforceable within community context
    Obligatory,
    /// Absolute — binding without exception, violation is breach
    Absolute,
}

/// Who a duty applies to.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DutyScope {
    /// Every person individually
    AllPersons,
    /// Communities as collective bodies
    AllCommunities,
    /// Both persons and communities
    Universal,
}

/// The registry of all constitutional duties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DutiesRegistry {
    duties: HashMap<Uuid, Duty>,
    by_category: HashMap<DutyCategory, Vec<Uuid>>,
}

impl DutiesRegistry {
    /// Create an empty duties registry with no pre-populated duties.
    pub fn new() -> Self {
        Self {
            duties: HashMap::new(),
            by_category: HashMap::new(),
        }
    }

    /// Create a registry pre-populated with Covenant duties.
    pub fn with_covenant_duties() -> Self {
        let mut registry = Self::new();
        for duty in covenant_duties() {
            let _ = registry.register(duty);
        }
        registry
    }

    /// Register a new duty. Returns an error if a duty with the same name and category already exists.
    pub fn register(&mut self, duty: Duty) -> Result<Uuid, crate::PolityError> {
        let id = duty.id;
        if self.duties.values().any(|d| d.name == duty.name && d.category == duty.category) {
            return Err(crate::PolityError::DuplicateDuty(duty.name));
        }
        self.by_category.entry(duty.category).or_default().push(id);
        self.duties.insert(id, duty);
        Ok(id)
    }

    /// Look up a duty by ID.
    pub fn get(&self, id: &Uuid) -> Option<&Duty> {
        self.duties.get(id)
    }

    /// Find duties by category.
    pub fn by_category(&self, category: DutyCategory) -> Vec<&Duty> {
        self.by_category
            .get(&category)
            .map(|ids| ids.iter().filter_map(|id| self.duties.get(id)).collect())
            .unwrap_or_default()
    }

    /// Find a duty by name (case-insensitive).
    pub fn find_by_name(&self, name: &str) -> Option<&Duty> {
        let lower = name.to_lowercase();
        self.duties.values().find(|d| d.name.to_lowercase() == lower)
    }

    /// All duties in the registry.
    pub fn all(&self) -> Vec<&Duty> {
        self.duties.values().collect()
    }

    /// All duties with `BindingLevel::Absolute` — violation is always a breach.
    pub fn absolute(&self) -> Vec<&Duty> {
        self.duties
            .values()
            .filter(|d| d.binding_level == BindingLevel::Absolute)
            .collect()
    }

    /// Number of registered duties.
    pub fn len(&self) -> usize {
        self.duties.len()
    }

    /// Whether the registry contains no duties.
    pub fn is_empty(&self) -> bool {
        self.duties.is_empty()
    }

    /// Remove a duty (only if not immutable). Covenant duties cannot be removed.
    pub fn remove(&mut self, id: &Uuid) -> Result<Duty, crate::PolityError> {
        let duty = self
            .duties
            .get(id)
            .ok_or_else(|| crate::PolityError::DutyNotFound(id.to_string()))?;

        if duty.is_immutable {
            return Err(crate::PolityError::ImmutableViolation(duty.name.clone()));
        }

        let duty = self
            .duties
            .remove(id)
            .expect("key verified to exist by .get() above");
        if let Some(ids) = self.by_category.get_mut(&duty.category) {
            ids.retain(|i| i != id);
        }
        Ok(duty)
    }
}

impl Default for DutiesRegistry {
    fn default() -> Self {
        Self::with_covenant_duties()
    }
}

/// The foundational duties from Covenant Core Art. 4 and 7.
fn covenant_duties() -> Vec<Duty> {
    vec![
        Duty::new(
            DutyCategory::UpholdDignity,
            "Duty to Uphold Dignity and Justice",
            "The duty of the people to uphold the dignity of every person and act in accordance with justice shall be binding. This duty applies in private and public life.",
            BindingLevel::Absolute,
            DutyScope::Universal,
            true,
            "Core Art. 4 Section 1",
        ),
        Duty::new(
            DutyCategory::Remember,
            "Duty of Remembrance",
            "The duty of the people to remember shall be binding. This includes the duty to carry forward memory of those displaced, harmed, erased, or enslaved.",
            BindingLevel::Absolute,
            DutyScope::Universal,
            true,
            "Core Art. 4 Section 2",
        ),
        Duty::new(
            DutyCategory::Steward,
            "Duty of Stewardship",
            "The duty of the people to protect the Earth and all living systems shall be binding. This includes the duty to restore what has been damaged and to end extractive practices.",
            BindingLevel::Absolute,
            DutyScope::Universal,
            true,
            "Core Art. 4 Section 3",
        ),
        Duty::new(
            DutyCategory::RefuseAndReconstitute,
            "Duty to Refuse and Reconstitute",
            "The duty of the people to refuse unlawful governance and to reconstitute just structures shall be binding.",
            BindingLevel::Absolute,
            DutyScope::Universal,
            true,
            "Core Art. 4 Section 4",
        ),
        Duty::new(
            DutyCategory::CommunityFidelity,
            "Duty to Uphold the Covenant",
            "Communities shall hold the duty to uphold the rights, principles, and regenerative commitments of this Covenant.",
            BindingLevel::Absolute,
            DutyScope::AllCommunities,
            true,
            "Core Art. 7 Section 1",
        ),
        Duty::new(
            DutyCategory::MutualAid,
            "Duty of Mutual Aid and Solidarity",
            "Communities shall hold the duty to offer mutual aid, protect the vulnerable, and act in solidarity with others in times of need. Communities are not fortresses. They are nodes of care.",
            BindingLevel::Obligatory,
            DutyScope::AllCommunities,
            true,
            "Core Art. 7 Section 2",
        ),
        Duty::new(
            DutyCategory::EcologicalStewardship,
            "Duty of Ecological Stewardship",
            "Communities shall hold the duty to live in right relation with Earth and act as stewards of their local ecologies.",
            BindingLevel::Absolute,
            DutyScope::AllCommunities,
            true,
            "Core Art. 7 Section 3",
        ),
        Duty::new(
            DutyCategory::PublicChallenge,
            "Duty of Public Challenge and Reconstitution",
            "Communities shall hold the duty to challenge breaches of this Covenant and reconstitute relation where harm has occurred.",
            BindingLevel::Obligatory,
            DutyScope::AllCommunities,
            true,
            "Core Art. 7 Section 4",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covenant_duties_populated() {
        let registry = DutiesRegistry::with_covenant_duties();
        assert_eq!(registry.len(), 8);
        assert!(registry.all().iter().all(|d| d.is_immutable));
    }

    #[test]
    fn absolute_duties() {
        let registry = DutiesRegistry::default();
        let absolute = registry.absolute();
        // 6 absolute (mutual aid and public challenge are obligatory)
        assert_eq!(absolute.len(), 6);
        assert!(absolute.iter().all(|d| d.binding_level == BindingLevel::Absolute));
    }

    #[test]
    fn binding_level_ordering() {
        assert!(BindingLevel::Aspirational < BindingLevel::Obligatory);
        assert!(BindingLevel::Obligatory < BindingLevel::Absolute);
    }

    #[test]
    fn find_by_category() {
        let registry = DutiesRegistry::default();
        let steward = registry.by_category(DutyCategory::Steward);
        assert_eq!(steward.len(), 1);
        assert!(steward[0].name.contains("Stewardship"));
    }

    #[test]
    fn cannot_remove_immutable_duty() {
        let mut registry = DutiesRegistry::default();
        let duty = registry.find_by_name("Duty of Remembrance").unwrap();
        let id = duty.id;
        let result = registry.remove(&id);
        assert!(matches!(result, Err(crate::PolityError::ImmutableViolation(_))));
    }

    #[test]
    fn can_register_custom_duty() {
        let mut registry = DutiesRegistry::default();
        let duty = Duty::new(
            DutyCategory::MutualAid,
            "Duty of Seed Sharing",
            "Members shall share surplus seeds with neighboring communities each spring.",
            BindingLevel::Aspirational,
            DutyScope::AllCommunities,
            false,
            "Bioregional Compact Section 12",
        );
        registry.register(duty).unwrap();
        assert_eq!(registry.len(), 9);
    }

    #[test]
    fn duty_serialization_roundtrip() {
        let duty = Duty::new(
            DutyCategory::Steward,
            "Duty of Watershed Care",
            "Protect local watersheds.",
            BindingLevel::Obligatory,
            DutyScope::AllCommunities,
            false,
            "River Council Charter",
        );
        let json = serde_json::to_string(&duty).unwrap();
        let restored: Duty = serde_json::from_str(&json).unwrap();
        assert_eq!(duty.name, restored.name);
        assert_eq!(duty.binding_level, restored.binding_level);
    }
}
