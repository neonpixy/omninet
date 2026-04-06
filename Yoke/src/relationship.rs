use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use x::Value;

/// Typed relationship between two entities (events, ideas, people).
///
/// The vocabulary of edges in Yoke's relationship graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationType {
    /// This was created from that (creative lineage)
    DerivedFrom,
    /// This is a version of that (version chain)
    VersionOf,
    /// This was approved by that (proposal/vote)
    ApprovedBy,
    /// This is a comment on that
    CommentOn,
    /// This replaces that (newer version, correction)
    Supersedes,
    /// This references that (citation, mention)
    References,
    /// This was branched from that (design exploration)
    BranchedFrom,
    /// This was merged into that (branch reunion)
    MergedInto,
    /// This responds to that (reply, answer)
    RespondsTo,
    /// This endorses that (approval, recommendation)
    Endorses,
    /// This amends that (constitutional change, charter update)
    Amends,
    /// This community is formally federated with that community
    FederatedWith,
    /// This community has withdrawn federation from that community
    Defederated,
    /// App-defined relationship type
    Custom(String),
}

impl RelationType {
    /// Whether this relationship implies provenance (creative lineage).
    pub fn is_provenance(&self) -> bool {
        matches!(
            self,
            RelationType::DerivedFrom
                | RelationType::VersionOf
                | RelationType::BranchedFrom
                | RelationType::MergedInto
                | RelationType::Amends
        )
    }

    /// Whether this relationship is social (between people/actions).
    pub fn is_social(&self) -> bool {
        matches!(
            self,
            RelationType::ApprovedBy
                | RelationType::CommentOn
                | RelationType::RespondsTo
                | RelationType::Endorses
        )
    }

    /// Whether this relationship is structural (organization/replacement).
    pub fn is_structural(&self) -> bool {
        matches!(
            self,
            RelationType::Supersedes | RelationType::References | RelationType::FederatedWith | RelationType::Defederated
        )
    }
}

impl std::fmt::Display for RelationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RelationType::DerivedFrom => write!(f, "derived-from"),
            RelationType::VersionOf => write!(f, "version-of"),
            RelationType::ApprovedBy => write!(f, "approved-by"),
            RelationType::CommentOn => write!(f, "comment-on"),
            RelationType::Supersedes => write!(f, "supersedes"),
            RelationType::References => write!(f, "references"),
            RelationType::BranchedFrom => write!(f, "branched-from"),
            RelationType::MergedInto => write!(f, "merged-into"),
            RelationType::RespondsTo => write!(f, "responds-to"),
            RelationType::Endorses => write!(f, "endorses"),
            RelationType::Amends => write!(f, "amends"),
            RelationType::FederatedWith => write!(f, "federated-with"),
            RelationType::Defederated => write!(f, "defederated"),
            RelationType::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

/// A typed, signed link between two entities.
///
/// Source and target are string IDs — could be event IDs (hex), idea UUIDs,
/// or crown IDs. Yoke doesn't care what the IDs refer to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YokeLink {
    pub id: Uuid,
    pub source: String,
    pub target: String,
    pub relationship: RelationType,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub metadata: HashMap<String, Value>,
}

impl YokeLink {
    pub fn new(
        source: impl Into<String>,
        target: impl Into<String>,
        relationship: RelationType,
        author: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            source: source.into(),
            target: target.into(),
            relationship,
            author: author.into(),
            created_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Whether this link represents creative provenance.
    pub fn is_provenance(&self) -> bool {
        self.relationship.is_provenance()
    }

    /// Whether this link represents a social interaction.
    pub fn is_social(&self) -> bool {
        self.relationship.is_social()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_link() {
        let link = YokeLink::new("event-a", "event-b", RelationType::DerivedFrom, "cpub1alice");
        assert_eq!(link.source, "event-a");
        assert_eq!(link.target, "event-b");
        assert_eq!(link.relationship, RelationType::DerivedFrom);
        assert_eq!(link.author, "cpub1alice");
        assert!(link.metadata.is_empty());
    }

    #[test]
    fn link_with_metadata() {
        let link = YokeLink::new("a", "b", RelationType::VersionOf, "cpub1bob")
            .with_metadata("version", Value::String("2.0".into()))
            .with_metadata("approved", Value::Bool(true));
        assert_eq!(link.metadata.len(), 2);
        assert_eq!(
            link.metadata.get("version"),
            Some(&Value::String("2.0".into()))
        );
    }

    #[test]
    fn relationship_categories() {
        assert!(RelationType::DerivedFrom.is_provenance());
        assert!(RelationType::VersionOf.is_provenance());
        assert!(RelationType::BranchedFrom.is_provenance());
        assert!(RelationType::MergedInto.is_provenance());
        assert!(RelationType::Amends.is_provenance());

        assert!(!RelationType::CommentOn.is_provenance());
        assert!(!RelationType::Endorses.is_provenance());

        assert!(RelationType::ApprovedBy.is_social());
        assert!(RelationType::CommentOn.is_social());
        assert!(RelationType::RespondsTo.is_social());
        assert!(RelationType::Endorses.is_social());

        assert!(!RelationType::DerivedFrom.is_social());

        assert!(RelationType::Supersedes.is_structural());
        assert!(RelationType::References.is_structural());
        assert!(!RelationType::DerivedFrom.is_structural());
    }

    #[test]
    fn custom_relationship() {
        let rel = RelationType::Custom("blocks".into());
        assert!(!rel.is_provenance());
        assert!(!rel.is_social());
        assert!(!rel.is_structural());
        assert_eq!(rel.to_string(), "custom:blocks");
    }

    #[test]
    fn display_formatting() {
        assert_eq!(RelationType::DerivedFrom.to_string(), "derived-from");
        assert_eq!(RelationType::VersionOf.to_string(), "version-of");
        assert_eq!(RelationType::ApprovedBy.to_string(), "approved-by");
        assert_eq!(RelationType::MergedInto.to_string(), "merged-into");
    }

    #[test]
    fn link_serde_round_trip() {
        let link = YokeLink::new("src", "tgt", RelationType::References, "cpub1test")
            .with_metadata("note", Value::String("see also".into()));
        let json = serde_json::to_string(&link).unwrap();
        let restored: YokeLink = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.source, "src");
        assert_eq!(restored.target, "tgt");
        assert_eq!(restored.relationship, RelationType::References);
        assert_eq!(restored.metadata.len(), 1);
    }

    #[test]
    fn provenance_link() {
        let link = YokeLink::new("remix", "original", RelationType::DerivedFrom, "cpub1artist");
        assert!(link.is_provenance());
        assert!(!link.is_social());
    }

    #[test]
    fn social_link() {
        let link = YokeLink::new("comment", "post", RelationType::CommentOn, "cpub1reader");
        assert!(link.is_social());
        assert!(!link.is_provenance());
    }

    #[test]
    fn federation_relation_is_structural() {
        assert!(RelationType::FederatedWith.is_structural());
        assert!(RelationType::Defederated.is_structural());
        assert!(!RelationType::FederatedWith.is_provenance());
        assert!(!RelationType::FederatedWith.is_social());
        assert!(!RelationType::Defederated.is_provenance());
        assert!(!RelationType::Defederated.is_social());
    }

    #[test]
    fn federation_display() {
        assert_eq!(RelationType::FederatedWith.to_string(), "federated-with");
        assert_eq!(RelationType::Defederated.to_string(), "defederated");
    }

    #[test]
    fn federation_link_serde_round_trip() {
        let link = YokeLink::new("community-a", "community-b", RelationType::FederatedWith, "cpub1delegate");
        let json = serde_json::to_string(&link).unwrap();
        let restored: YokeLink = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.relationship, RelationType::FederatedWith);
        assert_eq!(restored.source, "community-a");
        assert_eq!(restored.target, "community-b");
    }
}
