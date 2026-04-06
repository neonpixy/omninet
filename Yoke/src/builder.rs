//! Helpers for constructing Yoke data as Globe-compatible tag structures.
//!
//! Yoke doesn't depend on Globe directly — these helpers produce the tag
//! arrays that Globe's OmniFilter convenience builders expect. Apps use
//! these to construct UnsignedEvent tags before signing via EventBuilder.

use uuid::Uuid;

use crate::ceremony::{CeremonyRecord, CeremonyType};
use crate::relationship::YokeLink;
use crate::timeline::{ActivityAction, ActivityRecord, Milestone};
use crate::version::VersionTag;

/// Build tags for a YOKE_RELATIONSHIP event (kind 25000).
///
/// Globe filters use: `s` (source), `t` (target), `r` (relationship type).
pub fn relationship_tags(link: &YokeLink) -> Vec<Vec<String>> {
    vec![
        vec!["source".into(), link.source.clone()],
        vec!["target".into(), link.target.clone()],
        vec!["rel".into(), link.relationship.to_string()],
    ]
}

/// Build tags for a YOKE_VERSION_TAG event (kind 25001).
///
/// Globe filters use: `d` (idea_id), `branch`, `version`.
pub fn version_tag_tags(tag: &VersionTag) -> Vec<Vec<String>> {
    vec![
        vec!["d".into(), tag.idea_id.to_string()],
        vec!["branch".into(), tag.branch.clone()],
        vec!["version".into(), tag.name.clone()],
    ]
}

/// Build tags for a YOKE_BRANCH event (kind 25002).
///
/// Globe filters use: `d` (idea_id), `branch` (name), `from` (version_id).
pub fn branch_tags(idea_id: Uuid, branch_name: &str, from_version: Uuid) -> Vec<Vec<String>> {
    vec![
        vec!["d".into(), idea_id.to_string()],
        vec!["branch".into(), branch_name.into()],
        vec!["from".into(), from_version.to_string()],
    ]
}

/// Build tags for a YOKE_MERGE event (kind 25003).
///
/// Globe filters use: `d` (idea_id), `source` (branch), `target` (branch).
pub fn merge_tags(idea_id: Uuid, source_branch: &str, target_branch: &str) -> Vec<Vec<String>> {
    vec![
        vec!["d".into(), idea_id.to_string()],
        vec!["source".into(), source_branch.into()],
        vec!["target".into(), target_branch.into()],
    ]
}

/// Build tags for a YOKE_MILESTONE event (kind 25004).
///
/// Globe filters use: `d` (milestone_id), `community`.
pub fn milestone_tags(milestone: &Milestone) -> Vec<Vec<String>> {
    let mut tags = vec![vec!["d".into(), milestone.id.to_string()]];
    if let Some(community) = &milestone.community_id {
        tags.push(vec!["community".into(), community.clone()]);
    }
    tags
}

/// Build tags for a YOKE_CEREMONY event (kind 25005).
///
/// Globe filters use: `d` (ceremony_id), `type` (ceremony_type), `community`.
pub fn ceremony_tags(ceremony: &CeremonyRecord) -> Vec<Vec<String>> {
    let type_str = match &ceremony.ceremony_type {
        CeremonyType::CovenantOath => "CovenantOath",
        CeremonyType::CommunityFormation => "CommunityFormation",
        CeremonyType::UnionFormation => "UnionFormation",
        CeremonyType::CharterAmendment => "CharterAmendment",
        CeremonyType::Dissolution => "Dissolution",
        CeremonyType::LeadershipTransition => "LeadershipTransition",
        CeremonyType::ConstitutionalReview => "ConstitutionalReview",
        CeremonyType::FederationCeremony => "FederationCeremony",
        CeremonyType::DefederationCeremony => "DefederationCeremony",
        CeremonyType::Custom(s) => s.as_str(),
    };

    let mut tags = vec![
        vec!["d".into(), ceremony.id.to_string()],
        vec!["type".into(), type_str.into()],
    ];
    if let Some(community) = &ceremony.community_id {
        tags.push(vec!["community".into(), community.clone()]);
    }
    // Add participant p-tags for discoverability
    for participant in &ceremony.participants {
        tags.push(vec!["p".into(), participant.crown_id.clone()]);
    }
    tags
}

/// Build tags for a YOKE_ACTIVITY event (kind 25006).
///
/// Globe filters use: `actor` (a), `action`, `target` (t), `community` (c).
pub fn activity_tags(record: &ActivityRecord) -> Vec<Vec<String>> {
    let action_str = match &record.action {
        ActivityAction::Created => "created",
        ActivityAction::Updated => "updated",
        ActivityAction::Deleted => "deleted",
        ActivityAction::Approved => "approved",
        ActivityAction::Rejected => "rejected",
        ActivityAction::Commented => "commented",
        ActivityAction::Shared => "shared",
        ActivityAction::Transferred => "transferred",
        ActivityAction::Branched => "branched",
        ActivityAction::Merged => "merged",
        ActivityAction::Tagged => "tagged",
        ActivityAction::Published => "published",
        ActivityAction::Endorsed => "endorsed",
        ActivityAction::Flagged => "flagged",
        ActivityAction::FederationProposed => "federation_proposed",
        ActivityAction::FederationAccepted => "federation_accepted",
        ActivityAction::FederationWithdrawn => "federation_withdrawn",
        ActivityAction::AdvisorAssisted(_) => "advisor_assisted",
        ActivityAction::Custom(s) => s.as_str(),
    };

    let mut tags = vec![
        vec!["actor".into(), record.actor.clone()],
        vec!["action".into(), action_str.into()],
        vec!["target".into(), record.target_id.clone()],
    ];
    if let Some(community) = &record.community_id {
        tags.push(vec!["community".into(), community.clone()]);
    }
    tags
}

/// Serialize a YokeLink to JSON content for a YOKE_RELATIONSHIP event.
pub fn relationship_content(link: &YokeLink) -> Result<String, serde_json::Error> {
    serde_json::to_string(link)
}

/// Serialize a VersionTag to JSON content for a YOKE_VERSION_TAG event.
pub fn version_tag_content(tag: &VersionTag) -> Result<String, serde_json::Error> {
    serde_json::to_string(tag)
}

/// Serialize a Milestone to JSON content for a YOKE_MILESTONE event.
pub fn milestone_content(milestone: &Milestone) -> Result<String, serde_json::Error> {
    serde_json::to_string(milestone)
}

/// Serialize a CeremonyRecord to JSON content for a YOKE_CEREMONY event.
pub fn ceremony_content(ceremony: &CeremonyRecord) -> Result<String, serde_json::Error> {
    serde_json::to_string(ceremony)
}

/// Serialize an ActivityRecord to JSON content for a YOKE_ACTIVITY event.
pub fn activity_content(record: &ActivityRecord) -> Result<String, serde_json::Error> {
    serde_json::to_string(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relationship::RelationType;
    use crate::timeline::{MilestoneSignificance, TargetType};
    use x::VectorClock;

    #[test]
    fn relationship_tags_structure() {
        let link = YokeLink::new("src-id", "tgt-id", RelationType::DerivedFrom, "cpub1test");
        let tags = relationship_tags(&link);

        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0], vec!["source", "src-id"]);
        assert_eq!(tags[1], vec!["target", "tgt-id"]);
        assert_eq!(tags[2], vec!["rel", "derived-from"]);
    }

    #[test]
    fn relationship_tags_custom_type() {
        let link = YokeLink::new("a", "b", RelationType::Custom("blocks".into()), "cpub1test");
        let tags = relationship_tags(&link);
        assert_eq!(tags[2], vec!["rel", "custom:blocks"]);
    }

    #[test]
    fn version_tag_tags_structure() {
        let idea_id = Uuid::new_v4();
        let tag = VersionTag::new(idea_id, "v2.0", VectorClock::new(), "cpub1test")
            .on_branch("experimental");
        let tags = version_tag_tags(&tag);

        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0][0], "d");
        assert_eq!(tags[0][1], idea_id.to_string());
        assert_eq!(tags[1], vec!["branch", "experimental"]);
        assert_eq!(tags[2], vec!["version", "v2.0"]);
    }

    #[test]
    fn branch_tags_structure() {
        let idea_id = Uuid::new_v4();
        let from = Uuid::new_v4();
        let tags = branch_tags(idea_id, "dark-mode", from);

        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0][0], "d");
        assert_eq!(tags[0][1], idea_id.to_string());
        assert_eq!(tags[1], vec!["branch", "dark-mode"]);
        assert_eq!(tags[2][0], "from");
        assert_eq!(tags[2][1], from.to_string());
    }

    #[test]
    fn merge_tags_structure() {
        let idea_id = Uuid::new_v4();
        let tags = merge_tags(idea_id, "experimental", "main");

        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0][0], "d");
        assert_eq!(tags[0][1], idea_id.to_string());
        assert_eq!(tags[1], vec!["source", "experimental"]);
        assert_eq!(tags[2], vec!["target", "main"]);
    }

    #[test]
    fn milestone_tags_with_community() {
        let m = Milestone::new("Launch day", MilestoneSignificance::Historic, "cpub1test")
            .in_community("design-guild");
        let tags = milestone_tags(&m);

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0][0], "d");
        assert_eq!(tags[1], vec!["community", "design-guild"]);
    }

    #[test]
    fn milestone_tags_without_community() {
        let m = Milestone::new("Personal milestone", MilestoneSignificance::Minor, "cpub1test");
        let tags = milestone_tags(&m);
        assert_eq!(tags.len(), 1);
    }

    #[test]
    fn ceremony_tags_structure() {
        let c = CeremonyRecord::new(CeremonyType::CovenantOath)
            .with_principal("cpub1alice")
            .with_witness("cpub1bob")
            .in_community("guild");
        let tags = ceremony_tags(&c);

        assert_eq!(tags[1], vec!["type", "CovenantOath"]);
        assert_eq!(tags[2], vec!["community", "guild"]);
        // p-tags for participants
        assert_eq!(tags[3], vec!["p", "cpub1alice"]);
        assert_eq!(tags[4], vec!["p", "cpub1bob"]);
    }

    #[test]
    fn ceremony_tags_custom_type() {
        let c = CeremonyRecord::new(CeremonyType::Custom("graduation".into()));
        let tags = ceremony_tags(&c);
        assert_eq!(tags[1], vec!["type", "graduation"]);
    }

    #[test]
    fn activity_tags_structure() {
        let r = ActivityRecord::new(
            "cpub1alice",
            ActivityAction::Approved,
            "logo-v3",
            TargetType::Asset,
        )
        .in_community("design-guild");
        let tags = activity_tags(&r);

        assert_eq!(tags.len(), 4);
        assert_eq!(tags[0], vec!["actor", "cpub1alice"]);
        assert_eq!(tags[1], vec!["action", "approved"]);
        assert_eq!(tags[2], vec!["target", "logo-v3"]);
        assert_eq!(tags[3], vec!["community", "design-guild"]);
    }

    #[test]
    fn activity_tags_custom_action() {
        let r = ActivityRecord::new(
            "cpub1bob",
            ActivityAction::Custom("archived".into()),
            "old-doc",
            TargetType::Idea,
        );
        let tags = activity_tags(&r);
        assert_eq!(tags[1], vec!["action", "archived"]);
    }

    #[test]
    fn activity_tags_without_community() {
        let r = ActivityRecord::new(
            "cpub1alice",
            ActivityAction::Created,
            "new-idea",
            TargetType::Idea,
        );
        let tags = activity_tags(&r);
        assert_eq!(tags.len(), 3); // no community tag
    }

    #[test]
    fn ceremony_tags_federation() {
        let c = CeremonyRecord::new(CeremonyType::FederationCeremony)
            .with_principal("cpub1a")
            .with_principal("cpub1b")
            .in_community("guild");
        let tags = ceremony_tags(&c);
        assert_eq!(tags[1], vec!["type", "FederationCeremony"]);
        assert_eq!(tags[2], vec!["community", "guild"]);
    }

    #[test]
    fn ceremony_tags_defederation() {
        let c = CeremonyRecord::new(CeremonyType::DefederationCeremony)
            .with_principal("cpub1a")
            .in_community("guild");
        let tags = ceremony_tags(&c);
        assert_eq!(tags[1], vec!["type", "DefederationCeremony"]);
    }

    #[test]
    fn activity_tags_federation() {
        let r = ActivityRecord::new(
            "cpub1alice",
            ActivityAction::FederationProposed,
            "agreement-1",
            TargetType::Federation,
        ).in_community("guild");
        let tags = activity_tags(&r);
        assert_eq!(tags[1], vec!["action", "federation_proposed"]);

        let r2 = ActivityRecord::new("cpub1bob", ActivityAction::FederationAccepted, "agreement-1", TargetType::Federation);
        let tags2 = activity_tags(&r2);
        assert_eq!(tags2[1], vec!["action", "federation_accepted"]);

        let r3 = ActivityRecord::new("cpub1alice", ActivityAction::FederationWithdrawn, "agreement-1", TargetType::Federation);
        let tags3 = activity_tags(&r3);
        assert_eq!(tags3[1], vec!["action", "federation_withdrawn"]);
    }

    #[test]
    fn content_serialization_round_trips() {
        let link = YokeLink::new("a", "b", RelationType::References, "cpub1test");
        let json = relationship_content(&link).unwrap();
        let restored: YokeLink = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.source, "a");
        assert_eq!(restored.target, "b");

        let tag = VersionTag::new(Uuid::new_v4(), "v1.0", VectorClock::new(), "cpub1test");
        let json = version_tag_content(&tag).unwrap();
        let restored: VersionTag = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, "v1.0");

        let m = Milestone::new("test", MilestoneSignificance::Minor, "cpub1test");
        let json = milestone_content(&m).unwrap();
        let restored: Milestone = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, "test");

        let c = CeremonyRecord::new(CeremonyType::CovenantOath).with_principal("cpub1alice");
        let json = ceremony_content(&c).unwrap();
        let restored: CeremonyRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.ceremony_type, CeremonyType::CovenantOath);

        let a = ActivityRecord::new("actor", ActivityAction::Created, "target", TargetType::Idea);
        let json = activity_content(&a).unwrap();
        let restored: ActivityRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.actor, "actor");
    }
}
