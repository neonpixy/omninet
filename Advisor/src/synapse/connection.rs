use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A weighted connection between two entities in the cognitive graph.
///
/// Synapses strengthen with use and decay over time.
/// Pruned when strength falls to minimum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Synapse {
    pub id: Uuid,
    pub source_type: EntityType,
    pub source_id: Uuid,
    pub target_type: EntityType,
    pub target_id: Uuid,
    pub relationship: RelationshipType,
    pub created_at: DateTime<Utc>,
    pub last_referenced_at: DateTime<Utc>,
    /// Connection strength (clamped to min..=max from config)
    pub strength: f64,
}

/// What kind of entity a synapse endpoint is.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EntityType {
    /// A cognitive session (Home or User).
    Session,
    /// A single thought impulse.
    Thought,
    /// An .idea document.
    Idea,
    /// A stored memory with embedding.
    Memory,
}

/// How two entities are related.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RelationshipType {
    /// This entity discusses the target.
    Discusses,
    /// This entity references the target.
    References,
    /// A general thematic connection between two entities.
    RelatesTo,
    /// This entity was triggered by the target.
    TriggeredBy,
    /// This entity continues a line of reasoning from the target.
    ContinuesFrom,
    /// This entity emerged from reflection on the target.
    EmergedFrom,
    /// This entity explores a topic related to the target.
    Explores,
    /// A user-defined or domain-specific relationship.
    Custom(String),
}

/// A user-defined relationship type with custom behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomRelationship {
    /// Unique identifier (e.g., "contradicts")
    pub id: String,
    /// Display name for UI
    pub display_name: String,
    /// Inverse relationship name (e.g., "contradictedBy")
    pub inverse: Option<String>,
    /// Custom decay rate (overrides default if set)
    pub decay_rate: Option<f64>,
    /// Who registered this relationship type
    pub provider_id: String,
}

impl Synapse {
    /// Create a new synapse with configurable initial strength.
    pub fn new(
        source_type: EntityType,
        source_id: Uuid,
        target_type: EntityType,
        target_id: Uuid,
        relationship: RelationshipType,
        initial_strength: f64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            source_type,
            source_id,
            target_type,
            target_id,
            relationship,
            created_at: now,
            last_referenced_at: now,
            strength: initial_strength.clamp(0.1, 1.0),
        }
    }

    /// Create a "session contains thought" synapse.
    pub fn session_contains_thought(
        session_id: Uuid,
        thought_id: Uuid,
        initial_strength: f64,
    ) -> Self {
        Self::new(
            EntityType::Session,
            session_id,
            EntityType::Thought,
            thought_id,
            RelationshipType::Discusses,
            initial_strength,
        )
    }

    /// Create a "thought relates to thought" synapse.
    pub fn thought_relates(
        source_id: Uuid,
        target_id: Uuid,
        initial_strength: f64,
    ) -> Self {
        Self::new(
            EntityType::Thought,
            source_id,
            EntityType::Thought,
            target_id,
            RelationshipType::RelatesTo,
            initial_strength,
        )
    }

    /// Create a "thought references idea" synapse.
    pub fn thought_references_idea(
        thought_id: Uuid,
        idea_id: Uuid,
        initial_strength: f64,
    ) -> Self {
        Self::new(
            EntityType::Thought,
            thought_id,
            EntityType::Idea,
            idea_id,
            RelationshipType::References,
            initial_strength,
        )
    }

    /// Strengthen the synapse (called when it's referenced).
    pub fn strengthen(&mut self, increment: f64, max: f64) {
        self.strength = (self.strength + increment).min(max);
        self.last_referenced_at = Utc::now();
    }

    /// Apply daily decay.
    pub fn decay(&mut self, daily_rate: f64, min: f64) {
        self.strength = (self.strength - daily_rate).max(min);
    }

    /// Apply decay for multiple days.
    pub fn decay_days(&mut self, days: u32, daily_rate: f64, min: f64) {
        self.strength = (self.strength - daily_rate * f64::from(days)).max(min);
    }

    /// Whether this synapse should be pruned (at minimum strength).
    pub fn should_prune(&self, min_strength: f64) -> bool {
        self.strength <= min_strength
    }

    /// Whether this synapse involves a given entity.
    pub fn involves(&self, entity_type: EntityType, entity_id: Uuid) -> bool {
        (self.source_type == entity_type && self.source_id == entity_id)
            || (self.target_type == entity_type && self.target_id == entity_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synapse_creation() {
        let s = Synapse::new(
            EntityType::Session,
            Uuid::new_v4(),
            EntityType::Thought,
            Uuid::new_v4(),
            RelationshipType::Discusses,
            0.5,
        );
        assert_eq!(s.strength, 0.5);
        assert_eq!(s.source_type, EntityType::Session);
    }

    #[test]
    fn initial_strength_clamped() {
        let s = Synapse::new(
            EntityType::Thought,
            Uuid::new_v4(),
            EntityType::Thought,
            Uuid::new_v4(),
            RelationshipType::RelatesTo,
            5.0,
        );
        assert_eq!(s.strength, 1.0);

        let s2 = Synapse::new(
            EntityType::Thought,
            Uuid::new_v4(),
            EntityType::Thought,
            Uuid::new_v4(),
            RelationshipType::RelatesTo,
            -1.0,
        );
        assert_eq!(s2.strength, 0.1);
    }

    #[test]
    fn strengthen_and_cap() {
        let mut s = Synapse::thought_relates(Uuid::new_v4(), Uuid::new_v4(), 0.5);
        s.strengthen(0.2, 1.0);
        assert!((s.strength - 0.7).abs() < f64::EPSILON);
        s.strengthen(0.5, 1.0);
        assert!((s.strength - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn decay_and_floor() {
        let mut s = Synapse::thought_relates(Uuid::new_v4(), Uuid::new_v4(), 0.5);
        s.decay(0.05, 0.1);
        assert!((s.strength - 0.45).abs() < f64::EPSILON);

        // Decay below min
        s.decay_days(100, 0.05, 0.1);
        assert!((s.strength - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn should_prune() {
        let mut s = Synapse::thought_relates(Uuid::new_v4(), Uuid::new_v4(), 0.15);
        assert!(!s.should_prune(0.1));
        s.decay(0.05, 0.1);
        assert!(s.should_prune(0.1));
    }

    #[test]
    fn factory_methods() {
        let sid = Uuid::new_v4();
        let tid = Uuid::new_v4();
        let s = Synapse::session_contains_thought(sid, tid, 0.5);
        assert_eq!(s.source_type, EntityType::Session);
        assert_eq!(s.source_id, sid);
        assert_eq!(s.target_type, EntityType::Thought);
        assert_eq!(s.target_id, tid);

        let iid = Uuid::new_v4();
        let s2 = Synapse::thought_references_idea(tid, iid, 0.5);
        assert_eq!(s2.target_type, EntityType::Idea);
    }

    #[test]
    fn involves_check() {
        let sid = Uuid::new_v4();
        let tid = Uuid::new_v4();
        let s = Synapse::session_contains_thought(sid, tid, 0.5);
        assert!(s.involves(EntityType::Session, sid));
        assert!(s.involves(EntityType::Thought, tid));
        assert!(!s.involves(EntityType::Memory, Uuid::new_v4()));
    }

    #[test]
    fn custom_relationship() {
        let cr = CustomRelationship {
            id: "contradicts".into(),
            display_name: "Contradicts".into(),
            inverse: Some("contradicted_by".into()),
            decay_rate: Some(0.1),
            provider_id: "philosophy_plugin".into(),
        };
        assert_eq!(cr.inverse.as_deref(), Some("contradicted_by"));
    }

    #[test]
    fn synapse_serialization_roundtrip() {
        let s = Synapse::thought_relates(Uuid::new_v4(), Uuid::new_v4(), 0.5);
        let json = serde_json::to_string(&s).unwrap();
        let deserialized: Synapse = serde_json::from_str(&json).unwrap();
        assert_eq!(s, deserialized);
    }
}
