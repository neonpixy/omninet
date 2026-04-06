use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::protections::ProhibitionType;
use crate::rights::RightCategory;

/// A detected or declared violation of the Covenant.
///
/// From Covenant Core Art. 8 Section 2: "A breach shall be held to occur when a governing
/// body, economic system, technological platform, or community structure violates the
/// rights of persons, peoples, or Earth."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Breach {
    pub id: Uuid,
    pub violation_type: ViolationType,
    pub severity: BreachSeverity,
    pub description: String,
    /// Which right categories are affected by this breach.
    pub affected_rights: Vec<RightCategory>,
    /// Which prohibition types were violated.
    pub violated_prohibitions: Vec<ProhibitionType>,
    /// The entity responsible for the breach.
    pub actor: String,
    pub detected_at: DateTime<Utc>,
    /// Who reported this breach (pubkey or identifier), if known.
    pub reported_by: Option<String>,
    pub status: BreachStatus,
    /// Freeform key-value metadata (e.g., "platform" -> "SocialNet").
    pub context: HashMap<String, String>,
}

impl Breach {
    /// Create a new breach record. Starts in `Detected` status with no affected rights or prohibitions.
    /// Use builder methods (`with_rights`, `with_prohibitions`, etc.) to add details.
    pub fn new(
        violation_type: ViolationType,
        severity: BreachSeverity,
        description: impl Into<String>,
        actor: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            violation_type,
            severity,
            description: description.into(),
            affected_rights: Vec::new(),
            violated_prohibitions: Vec::new(),
            actor: actor.into(),
            detected_at: Utc::now(),
            reported_by: None,
            status: BreachStatus::Detected,
            context: HashMap::new(),
        }
    }

    /// Attach affected right categories to this breach.
    pub fn with_rights(mut self, rights: Vec<RightCategory>) -> Self {
        self.affected_rights = rights;
        self
    }

    /// Attach violated prohibition types to this breach.
    pub fn with_prohibitions(mut self, prohibitions: Vec<ProhibitionType>) -> Self {
        self.violated_prohibitions = prohibitions;
        self
    }

    /// Record who reported this breach.
    pub fn with_reporter(mut self, reporter: impl Into<String>) -> Self {
        self.reported_by = Some(reporter.into());
        self
    }

    /// Add a key-value context entry (e.g., "platform" -> "SocialNet").
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// Whether this breach involves immutable foundations.
    pub fn is_foundational(&self) -> bool {
        use crate::immutable::ImmutableFoundation;
        self.affected_rights
            .iter()
            .any(ImmutableFoundation::is_right_immutable)
            || self
                .violated_prohibitions
                .iter()
                .any(ImmutableFoundation::is_prohibition_absolute)
    }
}

/// How a breach was identified.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ViolationType {
    /// A right was infringed
    RightViolation,
    /// A duty was neglected
    DutyNeglect,
    /// A protection was breached
    ProtectionBreach,
    /// Consent was absent or coerced
    ConsentViolation,
    /// Systemic pattern of harm
    SystemicBreach,
}

/// The gravity of a breach.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BreachSeverity {
    /// Individual instance, limited harm
    Minor,
    /// Pattern emerging, multiple affected
    Significant,
    /// Widespread harm, structural cause
    Grave,
    /// Foundational — threatens the Core or Commons
    Existential,
}

/// Lifecycle of a breach.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BreachStatus {
    /// Just identified
    Detected,
    /// Under investigation
    Investigating,
    /// Confirmed as breach
    Confirmed,
    /// Remediation in progress
    Remediating,
    /// Resolved through repair
    Resolved,
    /// Dismissed (not a breach)
    Dismissed,
}

/// Tracks all breaches in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreachRegistry {
    breaches: HashMap<Uuid, Breach>,
}

impl BreachRegistry {
    /// Create an empty breach registry.
    pub fn new() -> Self {
        Self {
            breaches: HashMap::new(),
        }
    }

    /// Record a new breach and return its ID.
    pub fn record(&mut self, breach: Breach) -> Uuid {
        let id = breach.id;
        self.breaches.insert(id, breach);
        id
    }

    /// Look up a breach by ID.
    pub fn get(&self, id: &Uuid) -> Option<&Breach> {
        self.breaches.get(id)
    }

    /// Get a mutable reference to a breach by ID, for updating status or context.
    pub fn get_mut(&mut self, id: &Uuid) -> Option<&mut Breach> {
        self.breaches.get_mut(id)
    }

    /// Advance a breach through its lifecycle (Detected -> Investigating -> Confirmed -> etc.).
    pub fn update_status(&mut self, id: &Uuid, status: BreachStatus) -> Result<(), crate::PolityError> {
        let breach = self
            .breaches
            .get_mut(id)
            .ok_or_else(|| crate::PolityError::BreachNotFound(id.to_string()))?;
        breach.status = status;
        Ok(())
    }

    /// Find all breaches attributed to a given actor.
    pub fn by_actor(&self, actor: &str) -> Vec<&Breach> {
        self.breaches
            .values()
            .filter(|b| b.actor == actor)
            .collect()
    }

    /// Find all breaches at a specific severity level.
    pub fn by_severity(&self, severity: BreachSeverity) -> Vec<&Breach> {
        self.breaches
            .values()
            .filter(|b| b.severity == severity)
            .collect()
    }

    /// All breaches that are not yet Resolved or Dismissed.
    pub fn active(&self) -> Vec<&Breach> {
        self.breaches
            .values()
            .filter(|b| !matches!(b.status, BreachStatus::Resolved | BreachStatus::Dismissed))
            .collect()
    }

    /// All breaches that touch immutable foundations (Core rights or absolute prohibitions).
    pub fn foundational(&self) -> Vec<&Breach> {
        self.breaches
            .values()
            .filter(|b| b.is_foundational())
            .collect()
    }

    /// Total number of recorded breaches (all statuses).
    pub fn len(&self) -> usize {
        self.breaches.len()
    }

    /// Whether the registry contains no breaches.
    pub fn is_empty(&self) -> bool {
        self.breaches.is_empty()
    }
}

impl Default for BreachRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_record_breach() {
        let mut registry = BreachRegistry::new();
        let breach = Breach::new(
            ViolationType::ProtectionBreach,
            BreachSeverity::Grave,
            "Platform harvested user data without consent",
            "megacorp_platform",
        )
        .with_prohibitions(vec![ProhibitionType::Surveillance, ProhibitionType::Exploitation])
        .with_rights(vec![RightCategory::Privacy])
        .with_reporter("user_alice")
        .with_context("platform", "SocialNet");

        let id = registry.record(breach);
        let stored = registry.get(&id).unwrap();
        assert_eq!(stored.severity, BreachSeverity::Grave);
        assert_eq!(stored.violated_prohibitions.len(), 2);
        assert_eq!(stored.affected_rights.len(), 1);
        assert_eq!(stored.reported_by.as_deref(), Some("user_alice"));
        assert_eq!(stored.context.get("platform").unwrap(), "SocialNet");
    }

    #[test]
    fn breach_is_foundational() {
        let breach = Breach::new(
            ViolationType::RightViolation,
            BreachSeverity::Existential,
            "Dignity of persons denied",
            "authoritarian_regime",
        )
        .with_rights(vec![RightCategory::Dignity]);

        assert!(breach.is_foundational());
    }

    #[test]
    fn breach_is_not_foundational_for_custom_rights() {
        let breach = Breach::new(
            ViolationType::RightViolation,
            BreachSeverity::Minor,
            "Union ceremony not recorded",
            "lazy_registrar",
        )
        .with_rights(vec![RightCategory::Union]);

        assert!(!breach.is_foundational());
    }

    #[test]
    fn severity_ordering() {
        assert!(BreachSeverity::Minor < BreachSeverity::Significant);
        assert!(BreachSeverity::Significant < BreachSeverity::Grave);
        assert!(BreachSeverity::Grave < BreachSeverity::Existential);
    }

    #[test]
    fn update_breach_status() {
        let mut registry = BreachRegistry::new();
        let breach = Breach::new(
            ViolationType::ConsentViolation,
            BreachSeverity::Significant,
            "Consent assumed rather than obtained",
            "community_x",
        );
        let id = registry.record(breach);

        assert_eq!(registry.get(&id).unwrap().status, BreachStatus::Detected);
        registry.update_status(&id, BreachStatus::Investigating).unwrap();
        assert_eq!(registry.get(&id).unwrap().status, BreachStatus::Investigating);
        registry.update_status(&id, BreachStatus::Resolved).unwrap();
        assert_eq!(registry.get(&id).unwrap().status, BreachStatus::Resolved);
    }

    #[test]
    fn query_active_breaches() {
        let mut registry = BreachRegistry::new();

        let b1 = Breach::new(ViolationType::RightViolation, BreachSeverity::Minor, "minor issue", "a");
        let b2 = Breach::new(ViolationType::DutyNeglect, BreachSeverity::Significant, "neglect", "b");
        let id1 = registry.record(b1);
        let _id2 = registry.record(b2);

        assert_eq!(registry.active().len(), 2);
        registry.update_status(&id1, BreachStatus::Resolved).unwrap();
        assert_eq!(registry.active().len(), 1);
    }

    #[test]
    fn query_by_actor() {
        let mut registry = BreachRegistry::new();
        registry.record(Breach::new(ViolationType::RightViolation, BreachSeverity::Minor, "a", "corp_x"));
        registry.record(Breach::new(ViolationType::DutyNeglect, BreachSeverity::Minor, "b", "corp_x"));
        registry.record(Breach::new(ViolationType::RightViolation, BreachSeverity::Minor, "c", "corp_y"));

        assert_eq!(registry.by_actor("corp_x").len(), 2);
        assert_eq!(registry.by_actor("corp_y").len(), 1);
        assert_eq!(registry.by_actor("corp_z").len(), 0);
    }

    #[test]
    fn breach_serialization_roundtrip() {
        let breach = Breach::new(
            ViolationType::SystemicBreach,
            BreachSeverity::Existential,
            "System perpetuates domination through concealment",
            "shadow_corp",
        )
        .with_prohibitions(vec![ProhibitionType::Domination])
        .with_context("evidence", "audit_report_2026");

        let json = serde_json::to_string(&breach).unwrap();
        let restored: Breach = serde_json::from_str(&json).unwrap();
        assert_eq!(breach.description, restored.description);
        assert_eq!(breach.severity, restored.severity);
        assert_eq!(breach.context, restored.context);
    }
}
