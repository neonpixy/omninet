use uuid::Uuid;

use super::connection::{EntityType, RelationshipType, Synapse};

/// Builder for filtering synapses.
#[derive(Debug, Clone, Default)]
pub struct SynapseQuery {
    pub source_type: Option<EntityType>,
    pub source_id: Option<Uuid>,
    pub target_type: Option<EntityType>,
    pub target_id: Option<Uuid>,
    pub relationship: Option<RelationshipType>,
    pub min_strength: Option<f64>,
    pub limit: Option<usize>,
}

impl SynapseQuery {
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by source entity.
    pub fn from_entity(mut self, entity_type: EntityType, entity_id: Uuid) -> Self {
        self.source_type = Some(entity_type);
        self.source_id = Some(entity_id);
        self
    }

    /// Filter by target entity.
    pub fn to_entity(mut self, entity_type: EntityType, entity_id: Uuid) -> Self {
        self.target_type = Some(entity_type);
        self.target_id = Some(entity_id);
        self
    }

    /// Filter by either source or target involving this entity.
    pub fn involving(entity_type: EntityType, entity_id: Uuid) -> Self {
        // This is a special case — we store the entity info and check both
        // directions in matches(). We use source fields as a convention.
        Self {
            source_type: Some(entity_type),
            source_id: Some(entity_id),
            ..Default::default()
        }
    }

    /// Filter by relationship type.
    pub fn with_relationship(mut self, relationship: RelationshipType) -> Self {
        self.relationship = Some(relationship);
        self
    }

    /// Only include synapses above this strength.
    pub fn min_strength(mut self, strength: f64) -> Self {
        self.min_strength = Some(strength);
        self
    }

    /// Limit the number of results.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Check if a synapse matches this query.
    pub fn matches(&self, synapse: &Synapse) -> bool {
        // If both source and target are set, do strict matching
        if self.target_type.is_some() || self.target_id.is_some() {
            if let Some(st) = self.source_type {
                if synapse.source_type != st {
                    return false;
                }
            }
            if let Some(sid) = self.source_id {
                if synapse.source_id != sid {
                    return false;
                }
            }
            if let Some(tt) = self.target_type {
                if synapse.target_type != tt {
                    return false;
                }
            }
            if let Some(tid) = self.target_id {
                if synapse.target_id != tid {
                    return false;
                }
            }
        } else {
            // "involving" mode: check either direction
            if let (Some(st), Some(sid)) = (self.source_type, self.source_id) {
                if !synapse.involves(st, sid) {
                    return false;
                }
            }
        }

        if let Some(ref rel) = self.relationship {
            if synapse.relationship != *rel {
                return false;
            }
        }

        if let Some(min) = self.min_strength {
            if synapse.strength < min {
                return false;
            }
        }

        true
    }

    /// Apply this query to a collection of synapses.
    pub fn filter<'a>(&self, synapses: impl Iterator<Item = &'a Synapse>) -> Vec<&'a Synapse> {
        let mut results: Vec<&Synapse> = synapses.filter(|s| self.matches(s)).collect();
        // Sort by strength descending
        results.sort_by(|a, b| b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal));
        if let Some(limit) = self.limit {
            results.truncate(limit);
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synapse::connection::Synapse;

    #[test]
    fn query_builder() {
        let q = SynapseQuery::new()
            .from_entity(EntityType::Session, Uuid::new_v4())
            .with_relationship(RelationshipType::Discusses)
            .min_strength(0.3)
            .limit(10);
        assert!(q.source_type.is_some());
        assert!(q.relationship.is_some());
        assert_eq!(q.min_strength, Some(0.3));
        assert_eq!(q.limit, Some(10));
    }

    #[test]
    fn query_matches_involving() {
        let sid = Uuid::new_v4();
        let tid = Uuid::new_v4();
        let synapse = Synapse::session_contains_thought(sid, tid, 0.5);

        let q = SynapseQuery::involving(EntityType::Session, sid);
        assert!(q.matches(&synapse));

        let q2 = SynapseQuery::involving(EntityType::Thought, tid);
        assert!(q2.matches(&synapse));

        let q3 = SynapseQuery::involving(EntityType::Memory, Uuid::new_v4());
        assert!(!q3.matches(&synapse));
    }

    #[test]
    fn query_matches_directed() {
        let sid = Uuid::new_v4();
        let tid = Uuid::new_v4();
        let synapse = Synapse::session_contains_thought(sid, tid, 0.5);

        let q = SynapseQuery::new()
            .from_entity(EntityType::Session, sid)
            .to_entity(EntityType::Thought, tid);
        assert!(q.matches(&synapse));

        // Wrong direction
        let q2 = SynapseQuery::new()
            .from_entity(EntityType::Thought, tid)
            .to_entity(EntityType::Session, sid);
        assert!(!q2.matches(&synapse));
    }

    #[test]
    fn query_min_strength_filter() {
        let synapse = Synapse::thought_relates(Uuid::new_v4(), Uuid::new_v4(), 0.3);

        let q = SynapseQuery::new().min_strength(0.5);
        assert!(!q.matches(&synapse));

        let q2 = SynapseQuery::new().min_strength(0.2);
        assert!(q2.matches(&synapse));
    }

    #[test]
    fn query_relationship_filter() {
        let synapse = Synapse::thought_relates(Uuid::new_v4(), Uuid::new_v4(), 0.5);

        let q = SynapseQuery::new().with_relationship(RelationshipType::RelatesTo);
        assert!(q.matches(&synapse));

        let q2 = SynapseQuery::new().with_relationship(RelationshipType::References);
        assert!(!q2.matches(&synapse));
    }

    #[test]
    fn filter_sorts_by_strength_and_limits() {
        let s1 = Synapse::thought_relates(Uuid::new_v4(), Uuid::new_v4(), 0.3);
        let s2 = Synapse::thought_relates(Uuid::new_v4(), Uuid::new_v4(), 0.8);
        let s3 = Synapse::thought_relates(Uuid::new_v4(), Uuid::new_v4(), 0.5);
        let all = [s1, s2, s3];

        let q = SynapseQuery::new().limit(2);
        let results = q.filter(all.iter());
        assert_eq!(results.len(), 2);
        assert!(results[0].strength >= results[1].strength);
    }
}
