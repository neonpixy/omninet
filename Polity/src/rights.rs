use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A constitutional right — an entitlement that cannot be granted because it precedes law.
/// Rights are recognized, not bestowed. They are portable and inalienable.
///
/// From Covenant Core Art. 2: "Dignity is not granted. It is recognized in law because
/// it precedes law."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Right {
    pub id: Uuid,
    pub category: RightCategory,
    pub name: String,
    pub description: String,
    pub scope: RightScope,
    pub is_immutable: bool,
    pub source: String,
    pub created_at: DateTime<Utc>,
}

impl Right {
    /// Create a new right with a fresh UUID and the current timestamp.
    pub fn new(
        category: RightCategory,
        name: impl Into<String>,
        description: impl Into<String>,
        scope: RightScope,
        is_immutable: bool,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            category,
            name: name.into(),
            description: description.into(),
            scope,
            is_immutable,
            source: source.into(),
            created_at: Utc::now(),
        }
    }
}

/// Categories of rights, derived from Covenant Core Art. 2.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RightCategory {
    /// Core Art. 2 Section 1 — inherent worth, sovereignty, belonging
    Dignity,
    /// Core Art. 2 Section 2 — think, believe, express, assemble
    Thought,
    /// Core Art. 2 Section 2 — spiritual, political, cultural, artistic expression
    Expression,
    /// Core Art. 2 Section 3 — named, heard, protected under law
    LegalStanding,
    /// Core Art. 2 Section 4 — free from harm, deprivation, abandonment
    Safety,
    /// Core Art. 2 Section 5 — privacy, bodily autonomy, freedom from surveillance
    Privacy,
    /// Core Art. 2 Section 6 — refuse domination, resist violation
    Refusal,
    /// Core Art. 3 — rights of Earth and future generations
    Earth,
    /// Core Art. 6 — rights of communities to exist, self-define, participate
    Community,
    /// Conjunction Art. 4 — consent-based personal unions
    Union,
    /// Conjunction Art. 7 — meaningful work, dignified livelihood
    Labor,
}

/// Who or what a right applies to.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RightScope {
    /// Applies to every person (human and synthetic)
    AllPersons,
    /// Applies to all communities
    AllCommunities,
    /// Applies to Earth and living systems
    Earth,
    /// Applies to future generations
    FutureGenerations,
    /// Applies to all beings (persons and non-persons)
    AllBeings,
}

/// The registry of all recognized rights under the Covenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RightsRegistry {
    rights: HashMap<Uuid, Right>,
    by_category: HashMap<RightCategory, Vec<Uuid>>,
}

impl RightsRegistry {
    /// Create an empty rights registry with no pre-populated rights.
    pub fn new() -> Self {
        Self {
            rights: HashMap::new(),
            by_category: HashMap::new(),
        }
    }

    /// Create a registry pre-populated with all Covenant Core rights.
    pub fn with_covenant_rights() -> Self {
        let mut registry = Self::new();
        for right in covenant_rights() {
            // Infallible — these are fresh IDs
            let _ = registry.register(right);
        }
        registry
    }

    /// Register a new right.
    pub fn register(&mut self, right: Right) -> Result<Uuid, crate::PolityError> {
        let id = right.id;
        if self.rights.values().any(|r| r.name == right.name && r.category == right.category) {
            return Err(crate::PolityError::DuplicateRight(right.name));
        }
        self.by_category
            .entry(right.category)
            .or_default()
            .push(id);
        self.rights.insert(id, right);
        Ok(id)
    }

    /// Look up a right by ID.
    pub fn get(&self, id: &Uuid) -> Option<&Right> {
        self.rights.get(id)
    }

    /// Find rights by category.
    pub fn by_category(&self, category: RightCategory) -> Vec<&Right> {
        self.by_category
            .get(&category)
            .map(|ids| ids.iter().filter_map(|id| self.rights.get(id)).collect())
            .unwrap_or_default()
    }

    /// Find a right by name (case-insensitive).
    pub fn find_by_name(&self, name: &str) -> Option<&Right> {
        let lower = name.to_lowercase();
        self.rights.values().find(|r| r.name.to_lowercase() == lower)
    }

    /// All rights in the registry.
    pub fn all(&self) -> Vec<&Right> {
        self.rights.values().collect()
    }

    /// All immutable rights.
    pub fn immutable(&self) -> Vec<&Right> {
        self.rights.values().filter(|r| r.is_immutable).collect()
    }

    /// Number of registered rights.
    pub fn len(&self) -> usize {
        self.rights.len()
    }

    /// Whether the registry contains no rights.
    pub fn is_empty(&self) -> bool {
        self.rights.is_empty()
    }

    /// Remove a right (only if not immutable).
    pub fn remove(&mut self, id: &Uuid) -> Result<Right, crate::PolityError> {
        let right = self
            .rights
            .get(id)
            .ok_or_else(|| crate::PolityError::RightNotFound(id.to_string()))?;

        if right.is_immutable {
            return Err(crate::PolityError::ImmutableViolation(right.name.clone()));
        }

        let right = self
            .rights
            .remove(id)
            .expect("key verified to exist by .get() above");
        if let Some(ids) = self.by_category.get_mut(&right.category) {
            ids.retain(|i| i != id);
        }
        Ok(right)
    }
}

impl Default for RightsRegistry {
    fn default() -> Self {
        Self::with_covenant_rights()
    }
}

/// The foundational rights from Covenant Core Art. 2, 3, and 6.
/// These are immutable — no amendment can touch them.
fn covenant_rights() -> Vec<Right> {
    vec![
        // Core Art. 2 Section 1
        Right::new(
            RightCategory::Dignity,
            "Right to Dignity",
            "The right of every person to dignity shall not be infringed. No person shall be subjected to treatment, structure, or condition that denies their inherent worth, sovereignty, or belonging.",
            RightScope::AllPersons,
            true,
            "Core Art. 2 Section 1",
        ),
        // Core Art. 2 Section 2
        Right::new(
            RightCategory::Thought,
            "Right to Thought, Conscience, and Expression",
            "The right of the people to think, believe, express, and assemble shall not be infringed. This includes spiritual, political, cultural, and artistic expression.",
            RightScope::AllPersons,
            true,
            "Core Art. 2 Section 2",
        ),
        // Core Art. 2 Section 3
        Right::new(
            RightCategory::LegalStanding,
            "Right to Legal Standing and Equal Protection",
            "The right of every person to be named, heard, and protected under the law shall not be denied. This includes the right to challenge injustice, seek remedy, and receive equal treatment.",
            RightScope::AllPersons,
            true,
            "Core Art. 2 Section 3",
        ),
        // Core Art. 2 Section 4
        Right::new(
            RightCategory::Safety,
            "Right to Safety, Shelter, and Care",
            "The right of the people to live free from harm, deprivation, abandonment, or artificial scarcity shall not be infringed. Access to food, water, shelter, healing, and education shall be upheld as conditions of freedom.",
            RightScope::AllPersons,
            true,
            "Core Art. 2 Section 4",
        ),
        // Core Art. 2 Section 5
        Right::new(
            RightCategory::Privacy,
            "Right to Privacy and Autonomy",
            "The right of the people to privacy, bodily autonomy, and freedom from surveillance shall not be infringed. Consent obtained through dependency, necessity, or structural coercion shall be void.",
            RightScope::AllPersons,
            true,
            "Core Art. 2 Section 5",
        ),
        // Core Art. 2 Section 6
        Right::new(
            RightCategory::Refusal,
            "Right to Refuse and Resist",
            "The right of the people to refuse structures of domination and to resist systems that violate this Covenant shall not be infringed. Resistance in the name of justice shall be a lawful act.",
            RightScope::AllPersons,
            true,
            "Core Art. 2 Section 6",
        ),
        // Core Art. 3 Section 1
        Right::new(
            RightCategory::Earth,
            "Standing of Earth",
            "The Earth and all its living systems shall hold legal standing. The forests, rivers, oceans, mountains, animals, soils, fungi, skies, and all interdependent ecologies shall be protected as subjects of law.",
            RightScope::Earth,
            true,
            "Core Art. 3 Section 1",
        ),
        // Core Art. 3 Section 3
        Right::new(
            RightCategory::Earth,
            "Rights of Future Generations",
            "The right of future generations to inherit a living world shall not be infringed. This includes clean air, potable water, fertile soil, biological and linguistic diversity, cultural memory, and healthy ecosystems.",
            RightScope::FutureGenerations,
            true,
            "Core Art. 3 Section 3",
        ),
        // Core Art. 6 Section 1
        Right::new(
            RightCategory::Community,
            "Right to Existence and Self-Definition",
            "The right of communities to exist, self-organize, and define their cultural, spiritual, and social lifeways shall not be infringed.",
            RightScope::AllCommunities,
            true,
            "Core Art. 6 Section 1",
        ),
        // Core Art. 6 Section 2
        Right::new(
            RightCategory::Community,
            "Right to Participation in Governance",
            "The right of communities to participate directly in governance, through self-declaration, convocation, and collective enactment, shall not be infringed.",
            RightScope::AllCommunities,
            true,
            "Core Art. 6 Section 2",
        ),
        // Core Art. 6 Section 4
        Right::new(
            RightCategory::Community,
            "Right to Reclaim, Withdraw, and Reconstitute",
            "The right of communities to reclaim dignity, withdraw from unjust systems, and reconstitute themselves in lawful relation shall not be denied.",
            RightScope::AllCommunities,
            true,
            "Core Art. 6 Section 4",
        ),
        // Conjunction Art. 7 Section 2
        Right::new(
            RightCategory::Labor,
            "Right to Livelihood",
            "Every person shall be guaranteed the means to live with dignity. These are not rewards for productivity — they are the inheritance of kinship among the People. No person shall be made to work to survive.",
            RightScope::AllPersons,
            true,
            "Conjunction Art. 7 Section 2",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covenant_rights_populated() {
        let registry = RightsRegistry::with_covenant_rights();
        assert_eq!(registry.len(), 12);
        assert!(registry.all().iter().all(|r| r.is_immutable));
    }

    #[test]
    fn default_registry_has_covenant_rights() {
        let registry = RightsRegistry::default();
        assert_eq!(registry.len(), 12);
    }

    #[test]
    fn find_by_category() {
        let registry = RightsRegistry::default();
        let dignity = registry.by_category(RightCategory::Dignity);
        assert_eq!(dignity.len(), 1);
        assert!(dignity[0].name.contains("Dignity"));

        let earth = registry.by_category(RightCategory::Earth);
        assert_eq!(earth.len(), 2);

        let community = registry.by_category(RightCategory::Community);
        assert_eq!(community.len(), 3);
    }

    #[test]
    fn find_by_name() {
        let registry = RightsRegistry::default();
        let right = registry.find_by_name("right to dignity").unwrap();
        assert_eq!(right.category, RightCategory::Dignity);
        assert!(right.is_immutable);
    }

    #[test]
    fn cannot_remove_immutable_right() {
        let mut registry = RightsRegistry::default();
        let dignity = registry.find_by_name("Right to Dignity").unwrap();
        let id = dignity.id;
        let result = registry.remove(&id);
        assert!(matches!(result, Err(crate::PolityError::ImmutableViolation(_))));
    }

    #[test]
    fn can_register_and_remove_custom_right() {
        let mut registry = RightsRegistry::new();
        let right = Right::new(
            RightCategory::Community,
            "Right to Garden",
            "Every community may cultivate shared gardens.",
            RightScope::AllCommunities,
            false,
            "Local Charter Section 7",
        );
        let id = registry.register(right).unwrap();
        assert_eq!(registry.len(), 1);

        let removed = registry.remove(&id).unwrap();
        assert_eq!(removed.name, "Right to Garden");
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn cannot_register_duplicate_right() {
        let mut registry = RightsRegistry::new();
        let right1 = Right::new(
            RightCategory::Safety,
            "Right to Clean Water",
            "Access to clean water.",
            RightScope::AllPersons,
            false,
            "Local",
        );
        let right2 = Right::new(
            RightCategory::Safety,
            "Right to Clean Water",
            "Duplicate.",
            RightScope::AllPersons,
            false,
            "Local",
        );
        registry.register(right1).unwrap();
        let result = registry.register(right2);
        assert!(matches!(result, Err(crate::PolityError::DuplicateRight(_))));
    }

    #[test]
    fn right_serialization_roundtrip() {
        let right = Right::new(
            RightCategory::Privacy,
            "Right to Encryption",
            "All persons may encrypt their communications.",
            RightScope::AllPersons,
            false,
            "Digital Rights Charter",
        );
        let json = serde_json::to_string(&right).unwrap();
        let restored: Right = serde_json::from_str(&json).unwrap();
        assert_eq!(right.name, restored.name);
        assert_eq!(right.category, restored.category);
        assert_eq!(right.scope, restored.scope);
    }

    #[test]
    fn right_scope_covers_all_subjects() {
        let registry = RightsRegistry::default();
        let scopes: Vec<_> = registry.all().iter().map(|r| r.scope).collect();
        assert!(scopes.contains(&RightScope::AllPersons));
        assert!(scopes.contains(&RightScope::Earth));
        assert!(scopes.contains(&RightScope::FutureGenerations));
        assert!(scopes.contains(&RightScope::AllCommunities));
    }

    #[test]
    fn immutable_rights_filter() {
        let mut registry = RightsRegistry::default();
        let custom = Right::new(
            RightCategory::Labor,
            "Right to Sabbatical",
            "Every worker deserves extended rest.",
            RightScope::AllPersons,
            false,
            "Labor Council Resolution 3",
        );
        registry.register(custom).unwrap();
        assert_eq!(registry.len(), 13);
        assert_eq!(registry.immutable().len(), 12);
    }
}
