//! Integration tests proving cross-module Yoke workflows.

use uuid::Uuid;
use x::VectorClock;
use yoke::*;

/// Brand Asset Management: create assets, version them, link provenance,
/// record activities, mark milestones.
#[test]
fn brand_asset_management_workflow() {
    let mut graph = RelationshipGraph::new();
    let mut timeline = Timeline::new("brand-team");

    // Alice creates the original logo
    let logo_v1 = Uuid::new_v4().to_string();
    timeline.record(
        ActivityRecord::new("cpub1alice", ActivityAction::Created, &logo_v1, TargetType::Asset)
            .in_community("brand-team")
            .with_context("initial logo concept"),
    );

    // Bob derives a dark-mode variant
    let logo_dark = Uuid::new_v4().to_string();
    graph.add_link(YokeLink::new(
        &logo_dark,
        &logo_v1,
        RelationType::DerivedFrom,
        "cpub1bob",
    ));
    timeline.record(
        ActivityRecord::new("cpub1bob", ActivityAction::Created, &logo_dark, TargetType::Asset)
            .in_community("brand-team")
            .with_context("dark mode variant"),
    );

    // Carol comments on the dark variant
    let comment_id = Uuid::new_v4().to_string();
    graph.add_link(YokeLink::new(
        &comment_id,
        &logo_dark,
        RelationType::CommentOn,
        "cpub1carol",
    ));
    timeline.record(
        ActivityRecord::new(
            "cpub1carol",
            ActivityAction::Commented,
            &logo_dark,
            TargetType::Asset,
        )
        .in_community("brand-team"),
    );

    // Dave approves the dark variant
    let approval_id = Uuid::new_v4().to_string();
    graph.add_link(YokeLink::new(
        &approval_id,
        &logo_dark,
        RelationType::ApprovedBy,
        "cpub1dave",
    ));
    timeline.record(
        ActivityRecord::new(
            "cpub1dave",
            ActivityAction::Approved,
            &logo_dark,
            TargetType::Asset,
        )
        .in_community("brand-team"),
    );

    // Mark milestone
    timeline.mark_milestone(
        Milestone::new(
            "Dark mode logo approved",
            MilestoneSignificance::Notable,
            "cpub1dave",
        )
        .in_community("brand-team")
        .with_related_event(&approval_id),
    );

    // Verify provenance
    let ancestors = graph.ancestors(&logo_dark);
    assert_eq!(ancestors.len(), 1);
    assert_eq!(ancestors[0].entity_id, logo_v1);

    // Verify activity stream
    assert_eq!(timeline.activity_count(), 4);
    assert_eq!(timeline.in_community("brand-team").len(), 4);
    assert_eq!(timeline.for_target(&logo_dark).len(), 3);
    assert_eq!(
        timeline.by_action(&ActivityAction::Approved).len(),
        1
    );
    assert_eq!(timeline.milestone_count(), 1);

    // Verify social links on the dark variant
    assert_eq!(graph.comments_on(&logo_dark).len(), 1);
    assert_eq!(
        graph
            .reverse_links_of_type(&logo_dark, &RelationType::ApprovedBy)
            .len(),
        1
    );
}

/// Version chain: tag versions, branch, work, merge back.
#[test]
fn version_chain_branch_and_merge() {
    let idea_id = Uuid::new_v4();
    let mut chain = VersionChain::new(idea_id);
    let mut graph = RelationshipGraph::new();

    // v1.0 on main
    let mut clock = VectorClock::new();
    clock.increment("alice");
    let v1 = VersionTag::new(idea_id, "v1.0", clock.clone(), "cpub1alice")
        .with_message("initial design");
    let v1_id = v1.id;
    chain.tag_version(v1).unwrap();

    // Branch for rebrand exploration
    chain
        .create_branch("rebrand", v1_id, "cpub1bob")
        .unwrap();

    // Work on the branch
    clock.increment("bob");
    let v_rebrand = VersionTag::new(idea_id, "rebrand-draft", clock.clone(), "cpub1bob")
        .on_branch("rebrand")
        .with_message("trying new color palette");
    let v_rebrand_id = v_rebrand.id;
    chain.tag_version(v_rebrand).unwrap();

    // Track provenance: rebrand-draft derives from v1.0
    graph.add_link(YokeLink::new(
        v_rebrand_id.to_string(),
        v1_id.to_string(),
        RelationType::BranchedFrom,
        "cpub1bob",
    ));

    // Continue on main
    clock.increment("alice");
    let v2 = VersionTag::new(idea_id, "v2.0", clock.clone(), "cpub1alice")
        .with_message("refinements");
    chain.tag_version(v2).unwrap();

    // Merge rebrand back
    let merge_version = Uuid::new_v4();
    chain
        .merge_branch("rebrand", "main", merge_version, "cpub1bob")
        .unwrap();

    // Track merge in graph
    graph.add_link(YokeLink::new(
        v_rebrand_id.to_string(),
        merge_version.to_string(),
        RelationType::MergedInto,
        "cpub1bob",
    ));

    // Verify version chain state
    assert_eq!(chain.version_count(), 3);
    assert_eq!(chain.branch_count(), 2);
    assert!(chain.is_branch_merged("rebrand"));
    assert!(!chain.is_branch_merged("main")); // main can't be merged
    assert_eq!(chain.versions_on_branch("main").len(), 2);
    assert_eq!(chain.versions_on_branch("rebrand").len(), 1);
    assert_eq!(chain.latest_version("main").unwrap().name, "v2.0");

    // Verify graph links
    assert_eq!(graph.link_count(), 2);
}

/// Ceremony validation across all types.
#[test]
fn ceremony_validation_comprehensive() {
    // Valid ceremonies
    assert!(CeremonyRecord::new(CeremonyType::CovenantOath)
        .with_principal("cpub1alice")
        .validate()
        .is_ok());

    assert!(CeremonyRecord::new(CeremonyType::CommunityFormation)
        .with_principal("cpub1founder")
        .in_community("guild")
        .validate()
        .is_ok());

    assert!(CeremonyRecord::new(CeremonyType::UnionFormation)
        .with_principal("cpub1a")
        .with_principal("cpub1b")
        .validate()
        .is_ok());

    assert!(CeremonyRecord::new(CeremonyType::Dissolution)
        .with_principal("cpub1leader")
        .in_community("old-guild")
        .validate()
        .is_ok());

    // Invalid ceremonies
    assert!(CeremonyRecord::new(CeremonyType::CovenantOath)
        .validate()
        .is_err());

    assert!(CeremonyRecord::new(CeremonyType::CommunityFormation)
        .with_principal("cpub1founder")
        .validate() // missing community
        .is_err());

    assert!(CeremonyRecord::new(CeremonyType::UnionFormation)
        .with_principal("cpub1a")
        .validate() // only one principal
        .is_err());

    // Custom is always valid
    assert!(CeremonyRecord::new(CeremonyType::Custom("any".into()))
        .validate()
        .is_ok());
}

/// Community lifecycle: formation ceremony, activities, milestone, dissolution.
#[test]
fn community_lifecycle() {
    let mut timeline = Timeline::new("design-guild");

    // Formation ceremony
    let formation = CeremonyRecord::new(CeremonyType::CommunityFormation)
        .with_principal("cpub1founder")
        .with_officiant("cpub1elder")
        .with_witness("cpub1member1")
        .with_witness("cpub1member2")
        .in_community("design-guild")
        .with_content("We form this guild to serve design.");
    assert!(formation.validate().is_ok());

    // Record activities
    timeline.record(
        ActivityRecord::new(
            "cpub1founder",
            ActivityAction::Created,
            "charter",
            TargetType::Community,
        )
        .in_community("design-guild"),
    );

    for i in 0..5 {
        timeline.record(
            ActivityRecord::new(
                format!("cpub1member{i}"),
                ActivityAction::Custom("joined".into()),
                "design-guild",
                TargetType::Community,
            )
            .in_community("design-guild"),
        );
    }

    timeline.mark_milestone(
        Milestone::new("First 5 members", MilestoneSignificance::Notable, "cpub1founder")
            .in_community("design-guild"),
    );

    // Dissolution ceremony
    let dissolution = CeremonyRecord::new(CeremonyType::Dissolution)
        .with_principal("cpub1founder")
        .in_community("design-guild")
        .with_content("The guild closes with gratitude.");
    assert!(dissolution.validate().is_ok());

    assert_eq!(timeline.activity_count(), 6);
    assert_eq!(timeline.milestone_count(), 1);
    assert_eq!(timeline.in_community("design-guild").len(), 6);
}

/// Graph snapshot round-trip through JSON.
#[test]
fn graph_snapshot_persistence() {
    let mut graph = RelationshipGraph::new();
    graph.add_link(YokeLink::new("a", "b", RelationType::DerivedFrom, "cpub1alice"));
    graph.add_link(YokeLink::new("b", "c", RelationType::VersionOf, "cpub1bob"));
    graph.add_link(YokeLink::new("d", "c", RelationType::CommentOn, "cpub1carol"));
    graph.add_link(YokeLink::new("e", "b", RelationType::Endorses, "cpub1dave"));

    // Snapshot
    let snapshot = graph.snapshot();
    let json = serde_json::to_string(&snapshot).unwrap();

    // Restore
    let restored_snap: GraphSnapshot = serde_json::from_str(&json).unwrap();
    let restored = RelationshipGraph::from_snapshot(restored_snap);

    assert_eq!(restored.link_count(), 4);
    assert_eq!(restored.entity_count(), 5);
    assert_eq!(restored.links_from("a").len(), 1);
    assert_eq!(restored.comments_on("c").len(), 1);
    assert_eq!(restored.endorsements_of("b").len(), 1);

    // Traversal still works
    let ancestors = restored.ancestors("a");
    assert_eq!(ancestors.len(), 2); // b and c via provenance
}

/// Timeline capacity eviction under load.
#[test]
fn timeline_capacity_under_load() {
    let config = TimelineConfig {
        max_activities: 100,
        max_milestones: 1000,
    };
    let mut timeline = Timeline::new("stress-test").with_config(config);

    // Record 200 activities — should cap at 100
    for i in 0..200 {
        timeline.record(ActivityRecord::new(
            "cpub1actor",
            ActivityAction::Created,
            format!("item-{i}"),
            TargetType::Idea,
        ));
    }

    assert_eq!(timeline.activity_count(), 100);
    // Oldest evicted — first item should be item-100
    assert_eq!(timeline.activities[0].target_id, "item-100");
    assert_eq!(timeline.activities[99].target_id, "item-199");
}

/// Builder tag construction matches Globe filter expectations.
#[test]
fn builder_tags_match_globe_filters() {
    // Relationship tags
    let link = YokeLink::new("src", "tgt", RelationType::DerivedFrom, "cpub1test");
    let tags = builder::relationship_tags(&link);
    assert_eq!(tags[0][0], "source");
    assert_eq!(tags[1][0], "target");
    assert_eq!(tags[2][0], "rel");

    // Activity tags
    let record = ActivityRecord::new(
        "cpub1alice",
        ActivityAction::Approved,
        "asset-1",
        TargetType::Asset,
    )
    .in_community("guild");
    let tags = builder::activity_tags(&record);
    assert_eq!(tags[0][0], "actor");
    assert_eq!(tags[1][0], "action");
    assert_eq!(tags[2][0], "target");
    assert_eq!(tags[3][0], "community");

    // Ceremony tags
    let ceremony = CeremonyRecord::new(CeremonyType::CovenantOath)
        .with_principal("cpub1alice")
        .in_community("guild");
    let tags = builder::ceremony_tags(&ceremony);
    assert_eq!(tags[1][1], "CovenantOath");
    assert_eq!(tags[2][1], "guild");
    assert_eq!(tags[3][0], "p"); // participant p-tag
}

/// Entity removal cleans up both directions.
#[test]
fn graph_entity_removal() {
    let mut graph = RelationshipGraph::new();
    graph.add_link(YokeLink::new("a", "b", RelationType::DerivedFrom, "cpub1alice"));
    graph.add_link(YokeLink::new("b", "c", RelationType::References, "cpub1bob"));
    graph.add_link(YokeLink::new("d", "b", RelationType::CommentOn, "cpub1carol"));
    graph.add_link(YokeLink::new("e", "f", RelationType::Endorses, "cpub1dave"));

    assert_eq!(graph.link_count(), 4);

    // Remove b — should remove 3 links (a→b, b→c, d→b), leave e→f
    graph.remove_entity("b");
    assert_eq!(graph.link_count(), 1);
    assert!(graph.links_from("a").is_empty());
    assert!(graph.links_to("c").is_empty());
    assert_eq!(graph.links_from("e").len(), 1);
}

/// Filtered graph traversal by time and author.
#[test]
fn graph_filtered_queries() {
    let mut graph = RelationshipGraph::new();

    let mut link1 = YokeLink::new("a", "b", RelationType::DerivedFrom, "cpub1alice");
    link1.created_at = chrono::Utc::now() - chrono::Duration::days(30);
    graph.add_link(link1);

    let link2 = YokeLink::new("a", "c", RelationType::DerivedFrom, "cpub1bob");
    graph.add_link(link2);

    let link3 = YokeLink::new("a", "d", RelationType::CommentOn, "cpub1alice");
    graph.add_link(link3);

    // Filter by author
    assert_eq!(graph.links_from_by_author("a", "cpub1alice").len(), 2);
    assert_eq!(graph.links_from_by_author("a", "cpub1bob").len(), 1);

    // Filter by time — only recent links
    let since = chrono::Utc::now() - chrono::Duration::days(7);
    let until = chrono::Utc::now() + chrono::Duration::days(1);
    let recent = graph.links_from_between("a", since, until);
    assert_eq!(recent.len(), 2); // b is 30 days old, c and d are recent
}
