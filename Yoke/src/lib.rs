//! # Yoke — History & Provenance
//!
//! The binding thread. Yoke remembers. Version history, provenance chains,
//! collective memory, and the ceremonial record. We are yoked to our history —
//! not as a burden, but as a foundation.
//!
//! ## What Yoke Provides
//!
//! - **Typed relationships** — a vocabulary of edges connecting events, ideas,
//!   and people. DerivedFrom, VersionOf, ApprovedBy, CommentOn, and more.
//! - **Version history** — named snapshots and branches for any .idea,
//!   built on top of Ideas' CRDT operations.
//! - **Activity timelines** — what happened, when, by whom. Community history.
//! - **Relationship graphs** — traverse the web of connections between entities.
//! - **Ceremonial records** — Covenant oaths, community formations, unions,
//!   leadership transitions. The moments that matter.
//!
//! ## Architecture
//!
//! Yoke is pure data structures and logic. Zero async, zero platform dependencies.
//! It defines the vocabulary and graph that apps use to track history and
//! provenance. Globe events carry Yoke data across the network (kinds 25000-25999).

pub mod ai_provenance;
pub mod builder;
pub mod ceremony;
pub mod error;
pub mod graph;
pub mod provenance;
pub mod relationship;
pub mod timeline;
pub mod version;

// Re-exports
pub use ceremony::{CeremonyParticipant, CeremonyRecord, CeremonyType, ParticipantRole};
pub use error::YokeError;
pub use graph::{Direction, GraphSnapshot, RelationshipGraph, TraversalNode};
pub use relationship::{RelationType, YokeLink};
pub use timeline::{
    ActivityAction, ActivityRecord, Milestone, MilestoneSignificance, TargetType, Timeline,
    TimelineConfig,
};
pub use version::{Branch, MergeRecord, VersionChain, VersionTag};
pub use provenance::{
    Corroboration, EventData, ProvenanceChain, ProvenanceComputer, ProvenanceFactors,
    ProvenanceLink, ProvenanceScore,
};
pub use ai_provenance::{
    AdvisorAttribution, AuthorshipEntry, AuthorshipSource, IdeaAuthorship,
};

/// Yoke event kind constants (range 25000-25999).
pub mod kind {
    /// Typed relationship between two entities.
    ///
    /// Content: JSON YokeLink
    /// Tags: `["source", id]`, `["target", id]`, `["rel", type]`
    pub const RELATIONSHIP: u32 = 25000;

    /// Version tag — a named snapshot of an .idea.
    ///
    /// Content: JSON VersionTag
    /// Tags: `["d", idea_id]`, `["branch", name]`, `["version", name]`
    pub const VERSION_TAG: u32 = 25001;

    /// Branch — fork a version timeline.
    ///
    /// Content: JSON Branch
    /// Tags: `["d", idea_id]`, `["branch", name]`, `["from", version_id]`
    pub const BRANCH: u32 = 25002;

    /// Merge — join branches back together.
    ///
    /// Content: JSON MergeRecord
    /// Tags: `["d", idea_id]`, `["source", branch]`, `["target", branch]`
    pub const MERGE: u32 = 25003;

    /// Milestone — a named moment in community history.
    ///
    /// Content: JSON Milestone
    /// Tags: `["d", milestone_id]`, `["community", id]`
    pub const MILESTONE: u32 = 25004;

    /// Ceremony — a Covenant moment (oath, formation, dissolution).
    ///
    /// Content: JSON CeremonyRecord
    /// Tags: `["d", ceremony_id]`, `["type", ceremony_type]`, `["community", id]`
    pub const CEREMONY: u32 = 25005;

    /// Activity — a single entry in the activity stream.
    ///
    /// Content: JSON ActivityRecord
    /// Tags: `["actor", crown_id]`, `["action", type]`, `["target", id]`
    pub const ACTIVITY: u32 = 25006;

    /// Check if a kind is in Yoke's range (25000-25999).
    pub fn is_yoke_kind(kind: u32) -> bool {
        (25000..26000).contains(&kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_range() {
        assert!(kind::is_yoke_kind(25000));
        assert!(kind::is_yoke_kind(25006));
        assert!(kind::is_yoke_kind(25999));
        assert!(!kind::is_yoke_kind(24999));
        assert!(!kind::is_yoke_kind(26000));
    }

    #[test]
    fn full_provenance_workflow() {
        // Scenario: Alice creates a logo, Bob remixes it, Carol comments,
        // Dave approves the remix, Eve tags a version.

        let mut graph = RelationshipGraph::new();

        // Bob's remix derives from Alice's original
        graph.add_link(YokeLink::new(
            "bobs-remix",
            "alices-original",
            RelationType::DerivedFrom,
            "cpub1bob",
        ));

        // Carol comments on the remix
        graph.add_link(YokeLink::new(
            "carols-comment",
            "bobs-remix",
            RelationType::CommentOn,
            "cpub1carol",
        ));

        // Dave's approval references the remix
        graph.add_link(YokeLink::new(
            "daves-approval",
            "bobs-remix",
            RelationType::ApprovedBy,
            "cpub1dave",
        ));

        // Provenance chain from remix leads to original
        let ancestors = graph.ancestors("bobs-remix");
        assert_eq!(ancestors.len(), 1);
        assert_eq!(ancestors[0].entity_id, "alices-original");

        // Social links on the remix
        assert_eq!(graph.comments_on("bobs-remix").len(), 1);

        // Descendants of original
        let desc = graph.descendants("alices-original");
        assert_eq!(desc.len(), 1);
        assert_eq!(desc[0].entity_id, "bobs-remix");
    }

    #[test]
    fn full_version_workflow() {
        use uuid::Uuid;
        use x::VectorClock;

        let idea_id = Uuid::new_v4();
        let mut chain = VersionChain::new(idea_id);

        // Tag initial version
        let mut clock = VectorClock::new();
        clock.increment("alice");
        let v1 = VersionTag::new(idea_id, "v1.0", clock.clone(), "cpub1alice")
            .with_message("initial design");
        let v1_id = v1.id;
        chain.tag_version(v1).unwrap();

        // Create experimental branch
        chain
            .create_branch("dark-mode", v1_id, "cpub1bob")
            .unwrap();

        // Tag on the branch
        clock.increment("bob");
        let v_dark = VersionTag::new(idea_id, "dark-v1", clock.clone(), "cpub1bob")
            .on_branch("dark-mode")
            .with_message("dark mode exploration");
        chain.tag_version(v_dark).unwrap();

        // Tag v2 on main
        clock.increment("alice");
        let v2 = VersionTag::new(idea_id, "v2.0", clock.clone(), "cpub1alice")
            .with_message("color refresh");
        chain.tag_version(v2).unwrap();

        // Merge dark-mode back
        chain
            .merge_branch("dark-mode", "main", Uuid::new_v4(), "cpub1bob")
            .unwrap();

        assert_eq!(chain.version_count(), 3);
        assert_eq!(chain.branch_count(), 2);
        assert!(chain.is_branch_merged("dark-mode"));
        assert_eq!(chain.versions_on_branch("main").len(), 2);
        assert_eq!(chain.versions_on_branch("dark-mode").len(), 1);
        assert_eq!(chain.latest_version("main").unwrap().name, "v2.0");
    }

    #[test]
    fn full_timeline_workflow() {
        let mut timeline = Timeline::new("design-guild");

        // Record activities
        timeline.record(
            ActivityRecord::new(
                "cpub1alice",
                ActivityAction::Created,
                "logo-v1",
                TargetType::Asset,
            )
            .in_community("design-guild")
            .with_context("brand refresh project"),
        );

        timeline.record(
            ActivityRecord::new(
                "cpub1bob",
                ActivityAction::Commented,
                "logo-v1",
                TargetType::Asset,
            )
            .in_community("design-guild"),
        );

        timeline.record(
            ActivityRecord::new(
                "cpub1carol",
                ActivityAction::Approved,
                "logo-v1",
                TargetType::Asset,
            )
            .in_community("design-guild"),
        );

        // Mark milestone
        timeline.mark_milestone(
            Milestone::new(
                "Brand refresh complete",
                MilestoneSignificance::Major,
                "cpub1alice",
            )
            .with_description("New logo approved by the guild")
            .in_community("design-guild")
            .with_related_event("approval-event-id"),
        );

        assert_eq!(timeline.activity_count(), 3);
        assert_eq!(timeline.milestone_count(), 1);
        assert_eq!(timeline.for_target("logo-v1").len(), 3);
        assert_eq!(timeline.by_actor("cpub1alice").len(), 1);
        assert_eq!(
            timeline.by_action(&ActivityAction::Approved).len(),
            1
        );
        assert_eq!(timeline.in_community("design-guild").len(), 3);
    }

    #[test]
    fn full_ceremony_workflow() {
        // Community formation ceremony
        let formation = CeremonyRecord::new(CeremonyType::CommunityFormation)
            .with_principal("cpub1founder")
            .with_officiant("cpub1elder")
            .with_witness("cpub1member1")
            .with_witness("cpub1member2")
            .with_witness("cpub1member3")
            .in_community("design-guild")
            .with_content("We form this guild to serve the craft of design.")
            .with_related_event("charter-event");

        assert_eq!(formation.principals(), vec!["cpub1founder"]);
        assert_eq!(formation.witnesses().len(), 3);
        assert_eq!(formation.officiants(), vec!["cpub1elder"]);
        assert_eq!(formation.participant_count(), 5);

        // Covenant oath
        let oath = CeremonyRecord::new(CeremonyType::CovenantOath)
            .with_principal("cpub1newcomer")
            .with_witness("cpub1sponsor")
            .with_content("I enter freely. I understand my rights and duties.");

        assert_eq!(oath.principals(), vec!["cpub1newcomer"]);
        assert_eq!(oath.witnesses(), vec!["cpub1sponsor"]);
    }
}
