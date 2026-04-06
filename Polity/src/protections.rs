use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A constitutional protection — an active prohibition against specific forms of harm.
///
/// From Covenant Core Art. 5: Protections against harm are not passive rights; they are
/// active barriers. The Covenant does not merely say "you have the right to be safe" — it
/// says "these specific harms are prohibited."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Protection {
    pub id: Uuid,
    pub prohibition_type: ProhibitionType,
    pub name: String,
    pub description: String,
    pub is_absolute: bool,
    pub is_immutable: bool,
    pub source: String,
    pub created_at: DateTime<Utc>,
}

impl Protection {
    /// Create a new protection with a fresh UUID. All Covenant protections default to immutable.
    pub fn new(
        prohibition_type: ProhibitionType,
        name: impl Into<String>,
        description: impl Into<String>,
        is_absolute: bool,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            prohibition_type,
            name: name.into(),
            description: description.into(),
            is_absolute,
            is_immutable: true, // all Covenant protections are immutable
            source: source.into(),
            created_at: Utc::now(),
        }
    }
}

/// Types of prohibitions, derived from Covenant Core Art. 5.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProhibitionType {
    /// Core Art. 5 Section 1 — no domination by force, status, wealth, lineage, or identity
    Domination,
    /// Core Art. 5 Section 1 — no discrimination on any basis
    Discrimination,
    /// Core Art. 5 Section 2 — no surveillance, behavioral manipulation, unwarranted intrusion
    Surveillance,
    /// Core Art. 5 Section 3 — no extraction, manufactured scarcity, debt bondage
    Exploitation,
    /// Core Art. 5 Section 4 — no torture, disappearance, depersonalization
    Cruelty,
    /// Core Art. 3 Section 2 — no permanent ecological harm, extinction, collapse
    Ecocide,
    /// Conjunction Art. 3 Section 4 — no industrial violence, commodification of sentient life
    IndustrialCruelty,
    /// Core Art. 8 Section 2 — conditions that constitute systemic breach
    SystemicBreach,
}

/// The registry of all constitutional protections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectionsRegistry {
    protections: HashMap<Uuid, Protection>,
    by_type: HashMap<ProhibitionType, Vec<Uuid>>,
}

impl ProtectionsRegistry {
    /// Create an empty protections registry with no pre-populated protections.
    pub fn new() -> Self {
        Self {
            protections: HashMap::new(),
            by_type: HashMap::new(),
        }
    }

    /// Create a registry pre-populated with Covenant protections.
    pub fn with_covenant_protections() -> Self {
        let mut registry = Self::new();
        for protection in covenant_protections() {
            let _ = registry.register(protection);
        }
        registry
    }

    /// Register a new protection. Returns an error if one with the same name and type already exists.
    pub fn register(&mut self, protection: Protection) -> Result<Uuid, crate::PolityError> {
        let id = protection.id;
        if self
            .protections
            .values()
            .any(|p| p.name == protection.name && p.prohibition_type == protection.prohibition_type)
        {
            return Err(crate::PolityError::DuplicateProtection(protection.name));
        }
        self.by_type
            .entry(protection.prohibition_type)
            .or_default()
            .push(id);
        self.protections.insert(id, protection);
        Ok(id)
    }

    /// Look up a protection by ID.
    pub fn get(&self, id: &Uuid) -> Option<&Protection> {
        self.protections.get(id)
    }

    /// Find protections by prohibition type.
    pub fn by_type(&self, prohibition_type: ProhibitionType) -> Vec<&Protection> {
        self.by_type
            .get(&prohibition_type)
            .map(|ids| ids.iter().filter_map(|id| self.protections.get(id)).collect())
            .unwrap_or_default()
    }

    /// Find a protection by name (case-insensitive).
    pub fn find_by_name(&self, name: &str) -> Option<&Protection> {
        let lower = name.to_lowercase();
        self.protections
            .values()
            .find(|p| p.name.to_lowercase() == lower)
    }

    /// All protections in the registry.
    pub fn all(&self) -> Vec<&Protection> {
        self.protections.values().collect()
    }

    /// All absolute protections — harm types that cannot be suspended under any circumstance.
    pub fn absolute(&self) -> Vec<&Protection> {
        self.protections
            .values()
            .filter(|p| p.is_absolute)
            .collect()
    }

    /// Number of registered protections.
    pub fn len(&self) -> usize {
        self.protections.len()
    }

    /// Whether the registry contains no protections.
    pub fn is_empty(&self) -> bool {
        self.protections.is_empty()
    }

    /// Check whether a described action violates any protection.
    pub fn check_violation(&self, action: &ActionDescription) -> Vec<&Protection> {
        self.protections
            .values()
            .filter(|p| action.violates.contains(&p.prohibition_type))
            .collect()
    }

    /// Remove a protection (only if not immutable). Covenant protections cannot be removed.
    pub fn remove(&mut self, id: &Uuid) -> Result<Protection, crate::PolityError> {
        let protection = self
            .protections
            .get(id)
            .ok_or_else(|| crate::PolityError::ProtectionNotFound(id.to_string()))?;

        if protection.is_immutable {
            return Err(crate::PolityError::ImmutableViolation(
                protection.name.clone(),
            ));
        }

        let protection = self
            .protections
            .remove(id)
            .expect("key verified to exist by .get() above");
        if let Some(ids) = self.by_type.get_mut(&protection.prohibition_type) {
            ids.retain(|i| i != id);
        }
        Ok(protection)
    }
}

impl Default for ProtectionsRegistry {
    fn default() -> Self {
        Self::with_covenant_protections()
    }
}

/// Describes an action to be checked against protections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDescription {
    pub description: String,
    pub actor: String,
    pub violates: Vec<ProhibitionType>,
}

/// Covenant Core Art. 5 protections — these are the hard barriers.
fn covenant_protections() -> Vec<Protection> {
    vec![
        Protection::new(
            ProhibitionType::Domination,
            "Prohibition of Domination",
            "No person or system shall impose domination over another by force, status, wealth, lineage, or identity. All hierarchies that deny equal dignity shall be dissolved.",
            true,
            "Core Art. 5 Section 1",
        ),
        Protection::new(
            ProhibitionType::Discrimination,
            "Prohibition of Discrimination",
            "Discrimination on the basis of race, gender, class, disability, religion, culture, language, sexual identity, age, origin, belief, or body shall be prohibited.",
            true,
            "Core Art. 5 Section 1",
        ),
        Protection::new(
            ProhibitionType::Surveillance,
            "Prohibition of Surveillance and Intrusion",
            "No person shall be subjected to surveillance, behavioral manipulation, or unwarranted intrusion. Monitoring, targeting, data harvesting, or behavior modification without meaningful and freely given consent shall be prohibited.",
            true,
            "Core Art. 5 Section 2",
        ),
        Protection::new(
            ProhibitionType::Exploitation,
            "Prohibition of Exploitation and Extraction",
            "No system that relies on domination, violence, secrecy, manufactured scarcity, surveillance, debt, or extractive accumulation shall hold lawful standing. No person shall be commodified.",
            true,
            "Core Art. 5 Section 3",
        ),
        Protection::new(
            ProhibitionType::Cruelty,
            "Prohibition of Torture and Depersonalization",
            "No person shall be subjected to torture, degrading treatment, disappearance, or permanent dislocation from community or land. Dignity shall not be suspended. No cause shall be allowed to erase the person.",
            true,
            "Core Art. 5 Section 4",
        ),
        Protection::new(
            ProhibitionType::Ecocide,
            "Protection from Ecological Harm",
            "No system shall hold standing where it causes permanent ecological harm, extinction, or collapse. Every law, practice, and institution shall support regeneration and renewal.",
            true,
            "Core Art. 3 Section 2",
        ),
        Protection::new(
            ProhibitionType::IndustrialCruelty,
            "Prohibition of Industrial Cruelty",
            "Systems of industrial violence, mechanized torment, and commodification of sentient life are abolished. A society that requires cruelty to function shall not stand.",
            true,
            "Conjunction Art. 3 Section 4",
        ),
        Protection::new(
            ProhibitionType::SystemicBreach,
            "Conditions of Systemic Breach",
            "A breach occurs when a governing body, economic system, technological platform, or community structure violates rights, breaks duties, refuses challenge or correction, or perpetuates domination through concealment, coercion, or systemic exclusion.",
            true,
            "Core Art. 8 Section 2",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covenant_protections_populated() {
        let registry = ProtectionsRegistry::with_covenant_protections();
        assert_eq!(registry.len(), 8);
        assert!(registry.all().iter().all(|p| p.is_immutable));
        assert!(registry.all().iter().all(|p| p.is_absolute));
    }

    #[test]
    fn find_by_type() {
        let registry = ProtectionsRegistry::default();
        let surveillance = registry.by_type(ProhibitionType::Surveillance);
        assert_eq!(surveillance.len(), 1);
        assert!(surveillance[0].name.contains("Surveillance"));
    }

    #[test]
    fn check_violation_detects_harm() {
        let registry = ProtectionsRegistry::default();
        let action = ActionDescription {
            description: "Platform harvests user behavioral data for ad targeting".into(),
            actor: "megacorp".into(),
            violates: vec![ProhibitionType::Surveillance, ProhibitionType::Exploitation],
        };
        let violations = registry.check_violation(&action);
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn check_violation_passes_clean_action() {
        let registry = ProtectionsRegistry::default();
        let action = ActionDescription {
            description: "Community votes on garden layout".into(),
            actor: "garden_collective".into(),
            violates: vec![],
        };
        let violations = registry.check_violation(&action);
        assert!(violations.is_empty());
    }

    #[test]
    fn cannot_remove_immutable_protection() {
        let mut registry = ProtectionsRegistry::default();
        let protection = registry.find_by_name("Prohibition of Domination").unwrap();
        let id = protection.id;
        let result = registry.remove(&id);
        assert!(matches!(result, Err(crate::PolityError::ImmutableViolation(_))));
    }

    #[test]
    fn absolute_protections() {
        let registry = ProtectionsRegistry::default();
        let absolute = registry.absolute();
        assert_eq!(absolute.len(), 8); // all Covenant protections are absolute
    }

    #[test]
    fn protection_serialization_roundtrip() {
        let protection = Protection::new(
            ProhibitionType::Domination,
            "No Algorithmic Domination",
            "Algorithms shall not determine human worth.",
            true,
            "Digital Rights Extension",
        );
        let json = serde_json::to_string(&protection).unwrap();
        let restored: Protection = serde_json::from_str(&json).unwrap();
        assert_eq!(protection.name, restored.name);
        assert_eq!(protection.prohibition_type, restored.prohibition_type);
    }
}
