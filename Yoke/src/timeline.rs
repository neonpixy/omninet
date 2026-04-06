use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ai_provenance::AdvisorAttribution;

/// What happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActivityAction {
    Created,
    Updated,
    Deleted,
    Approved,
    Rejected,
    Commented,
    Shared,
    Transferred,
    Branched,
    Merged,
    Tagged,
    Published,
    Endorsed,
    Flagged,
    /// Federation proposed to another community
    FederationProposed,
    /// Federation accepted with another community
    FederationAccepted,
    /// Federation withdrawn from another community
    FederationWithdrawn,
    /// An action performed or assisted by the AI Advisor (R6C).
    AdvisorAssisted(AdvisorAttribution),
    Custom(String),
}

/// What kind of thing was acted on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetType {
    Event,
    Idea,
    Community,
    Person,
    Proposal,
    Asset,
    /// A federation agreement between communities
    Federation,
    Custom(String),
}

/// A single entry in the activity stream.
///
/// "Alice approved logo-v3 in the Design community."
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityRecord {
    pub id: Uuid,
    pub actor: String,
    pub action: ActivityAction,
    pub target_id: String,
    pub target_type: TargetType,
    pub context: Option<String>,
    pub community_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl ActivityRecord {
    pub fn new(
        actor: impl Into<String>,
        action: ActivityAction,
        target_id: impl Into<String>,
        target_type: TargetType,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            actor: actor.into(),
            action,
            target_id: target_id.into(),
            target_type,
            context: None,
            community_id: None,
            created_at: Utc::now(),
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn in_community(mut self, community_id: impl Into<String>) -> Self {
        self.community_id = Some(community_id.into());
        self
    }
}

/// The significance of a milestone.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MilestoneSignificance {
    Minor,
    Notable,
    Major,
    Historic,
}

/// A named moment in community history.
///
/// "The Design Guild was formed." "Omnidea reached 1,000 participants."
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub significance: MilestoneSignificance,
    pub community_id: Option<String>,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub related_events: Vec<String>,
}

impl Milestone {
    pub fn new(
        name: impl Into<String>,
        significance: MilestoneSignificance,
        author: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            significance,
            community_id: None,
            author: author.into(),
            created_at: Utc::now(),
            related_events: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn in_community(mut self, community_id: impl Into<String>) -> Self {
        self.community_id = Some(community_id.into());
        self
    }

    pub fn with_related_event(mut self, event_id: impl Into<String>) -> Self {
        self.related_events.push(event_id.into());
        self
    }
}

/// Configuration for timeline capacity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineConfig {
    /// Maximum number of activity records (oldest evicted when exceeded).
    pub max_activities: usize,
    /// Maximum number of milestones (no eviction — milestones are permanent).
    pub max_milestones: usize,
}

impl Default for TimelineConfig {
    fn default() -> Self {
        Self {
            max_activities: 10_000,
            max_milestones: 1_000,
        }
    }
}

/// An activity timeline for a community or entity.
///
/// Answers: "What happened here?" and "What moments mattered?"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeline {
    pub owner_id: String,
    pub activities: Vec<ActivityRecord>,
    pub milestones: Vec<Milestone>,
    pub config: TimelineConfig,
}

impl Timeline {
    pub fn new(owner_id: impl Into<String>) -> Self {
        Self {
            owner_id: owner_id.into(),
            activities: Vec::new(),
            milestones: Vec::new(),
            config: TimelineConfig::default(),
        }
    }

    pub fn with_config(mut self, config: TimelineConfig) -> Self {
        self.config = config;
        self
    }

    /// Record an activity. Evicts oldest entries when over capacity.
    pub fn record(&mut self, activity: ActivityRecord) {
        self.activities.push(activity);
        while self.activities.len() > self.config.max_activities {
            self.activities.remove(0);
        }
    }

    /// Prune activities older than a cutoff timestamp.
    pub fn prune_before(&mut self, cutoff: DateTime<Utc>) -> usize {
        let before = self.activities.len();
        self.activities.retain(|a| a.created_at >= cutoff);
        before - self.activities.len()
    }

    pub fn mark_milestone(&mut self, milestone: Milestone) {
        self.milestones.push(milestone);
    }

    /// Activities by a specific actor.
    pub fn by_actor(&self, actor: &str) -> Vec<&ActivityRecord> {
        self.activities.iter().filter(|a| a.actor == actor).collect()
    }

    /// Activities of a specific action type.
    pub fn by_action(&self, action: &ActivityAction) -> Vec<&ActivityRecord> {
        self.activities
            .iter()
            .filter(|a| &a.action == action)
            .collect()
    }

    /// Activities targeting a specific entity.
    pub fn for_target(&self, target_id: &str) -> Vec<&ActivityRecord> {
        self.activities
            .iter()
            .filter(|a| a.target_id == target_id)
            .collect()
    }

    /// Activities within a time range.
    pub fn between(
        &self,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Vec<&ActivityRecord> {
        self.activities
            .iter()
            .filter(|a| a.created_at >= since && a.created_at <= until)
            .collect()
    }

    /// Activities in a specific community.
    pub fn in_community(&self, community_id: &str) -> Vec<&ActivityRecord> {
        self.activities
            .iter()
            .filter(|a| a.community_id.as_deref() == Some(community_id))
            .collect()
    }

    /// Milestones at or above a given significance.
    pub fn milestones_at_least(
        &self,
        significance: MilestoneSignificance,
    ) -> Vec<&Milestone> {
        self.milestones
            .iter()
            .filter(|m| m.significance >= significance)
            .collect()
    }

    pub fn activity_count(&self) -> usize {
        self.activities.len()
    }

    pub fn milestone_count(&self) -> usize {
        self.milestones.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_activity() {
        let mut timeline = Timeline::new("community-1");
        timeline.record(ActivityRecord::new(
            "cpub1alice",
            ActivityAction::Created,
            "asset-1",
            TargetType::Asset,
        ));
        assert_eq!(timeline.activity_count(), 1);
    }

    #[test]
    fn activity_with_context() {
        let record = ActivityRecord::new(
            "cpub1bob",
            ActivityAction::Approved,
            "logo-v3",
            TargetType::Asset,
        )
        .with_context("looks great, ship it")
        .in_community("design-guild");

        assert_eq!(record.context.as_deref(), Some("looks great, ship it"));
        assert_eq!(record.community_id.as_deref(), Some("design-guild"));
    }

    #[test]
    fn query_by_actor() {
        let mut timeline = Timeline::new("community-1");
        timeline.record(ActivityRecord::new(
            "cpub1alice",
            ActivityAction::Created,
            "a1",
            TargetType::Asset,
        ));
        timeline.record(ActivityRecord::new(
            "cpub1bob",
            ActivityAction::Created,
            "a2",
            TargetType::Asset,
        ));
        timeline.record(ActivityRecord::new(
            "cpub1alice",
            ActivityAction::Updated,
            "a1",
            TargetType::Asset,
        ));

        assert_eq!(timeline.by_actor("cpub1alice").len(), 2);
        assert_eq!(timeline.by_actor("cpub1bob").len(), 1);
        assert_eq!(timeline.by_actor("cpub1nobody").len(), 0);
    }

    #[test]
    fn query_by_action() {
        let mut timeline = Timeline::new("t");
        timeline.record(ActivityRecord::new("a", ActivityAction::Created, "x", TargetType::Idea));
        timeline.record(ActivityRecord::new("b", ActivityAction::Created, "y", TargetType::Idea));
        timeline.record(ActivityRecord::new("a", ActivityAction::Approved, "x", TargetType::Idea));

        assert_eq!(timeline.by_action(&ActivityAction::Created).len(), 2);
        assert_eq!(timeline.by_action(&ActivityAction::Approved).len(), 1);
        assert_eq!(timeline.by_action(&ActivityAction::Deleted).len(), 0);
    }

    #[test]
    fn query_for_target() {
        let mut timeline = Timeline::new("t");
        timeline.record(ActivityRecord::new("a", ActivityAction::Created, "logo", TargetType::Asset));
        timeline.record(ActivityRecord::new("b", ActivityAction::Commented, "logo", TargetType::Asset));
        timeline.record(ActivityRecord::new("c", ActivityAction::Created, "icon", TargetType::Asset));

        assert_eq!(timeline.for_target("logo").len(), 2);
        assert_eq!(timeline.for_target("icon").len(), 1);
    }

    #[test]
    fn query_in_community() {
        let mut timeline = Timeline::new("t");
        timeline.record(
            ActivityRecord::new("a", ActivityAction::Created, "x", TargetType::Idea)
                .in_community("guild-a"),
        );
        timeline.record(
            ActivityRecord::new("b", ActivityAction::Created, "y", TargetType::Idea)
                .in_community("guild-b"),
        );
        timeline.record(ActivityRecord::new("c", ActivityAction::Created, "z", TargetType::Idea));

        assert_eq!(timeline.in_community("guild-a").len(), 1);
        assert_eq!(timeline.in_community("guild-b").len(), 1);
        assert_eq!(timeline.in_community("guild-c").len(), 0);
    }

    #[test]
    fn milestones() {
        let mut timeline = Timeline::new("community-1");
        timeline.mark_milestone(
            Milestone::new("Community formed", MilestoneSignificance::Historic, "cpub1alice")
                .with_description("The Design Guild begins")
                .in_community("design-guild"),
        );
        timeline.mark_milestone(
            Milestone::new("First 10 members", MilestoneSignificance::Notable, "cpub1bob")
                .in_community("design-guild"),
        );
        timeline.mark_milestone(
            Milestone::new("Logo v2 shipped", MilestoneSignificance::Minor, "cpub1carol"),
        );

        assert_eq!(timeline.milestone_count(), 3);
        assert_eq!(
            timeline.milestones_at_least(MilestoneSignificance::Notable).len(),
            2
        );
        assert_eq!(
            timeline.milestones_at_least(MilestoneSignificance::Historic).len(),
            1
        );
    }

    #[test]
    fn milestone_with_related_events() {
        let m = Milestone::new("Charter ratified", MilestoneSignificance::Major, "cpub1alice")
            .with_related_event("proposal-event-1")
            .with_related_event("vote-event-2");
        assert_eq!(m.related_events.len(), 2);
    }

    #[test]
    fn custom_action_and_target() {
        let record = ActivityRecord::new(
            "cpub1alice",
            ActivityAction::Custom("archived".into()),
            "project-1",
            TargetType::Custom("project".into()),
        );
        assert_eq!(record.action, ActivityAction::Custom("archived".into()));
        assert_eq!(record.target_type, TargetType::Custom("project".into()));
    }

    #[test]
    fn time_range_query() {
        let mut timeline = Timeline::new("t");

        let now = Utc::now();
        let mut r1 = ActivityRecord::new("a", ActivityAction::Created, "x", TargetType::Idea);
        r1.created_at = now - chrono::Duration::hours(2);
        timeline.record(r1);

        let mut r2 = ActivityRecord::new("b", ActivityAction::Created, "y", TargetType::Idea);
        r2.created_at = now;
        timeline.record(r2);

        let mut r3 = ActivityRecord::new("c", ActivityAction::Created, "z", TargetType::Idea);
        r3.created_at = now + chrono::Duration::hours(2);
        timeline.record(r3);

        let since = now - chrono::Duration::hours(1);
        let until = now + chrono::Duration::hours(1);
        assert_eq!(timeline.between(since, until).len(), 1);
    }

    #[test]
    fn capacity_eviction() {
        let config = TimelineConfig {
            max_activities: 3,
            max_milestones: 100,
        };
        let mut timeline = Timeline::new("t").with_config(config);

        for i in 0..5 {
            timeline.record(ActivityRecord::new(
                "actor",
                ActivityAction::Created,
                format!("item-{i}"),
                TargetType::Idea,
            ));
        }

        assert_eq!(timeline.activity_count(), 3);
        // Oldest evicted — items 0 and 1 gone, 2-4 remain
        assert_eq!(timeline.activities[0].target_id, "item-2");
        assert_eq!(timeline.activities[2].target_id, "item-4");
    }

    #[test]
    fn prune_before_cutoff() {
        let mut timeline = Timeline::new("t");
        let now = Utc::now();

        let mut old = ActivityRecord::new("a", ActivityAction::Created, "old", TargetType::Idea);
        old.created_at = now - chrono::Duration::days(30);
        timeline.record(old);

        let mut recent =
            ActivityRecord::new("b", ActivityAction::Created, "recent", TargetType::Idea);
        recent.created_at = now;
        timeline.record(recent);

        let cutoff = now - chrono::Duration::days(7);
        let pruned = timeline.prune_before(cutoff);
        assert_eq!(pruned, 1);
        assert_eq!(timeline.activity_count(), 1);
        assert_eq!(timeline.activities[0].target_id, "recent");
    }

    #[test]
    fn federation_activity_actions() {
        let proposed = ActivityRecord::new(
            "cpub1community_a",
            ActivityAction::FederationProposed,
            "agreement-1",
            TargetType::Federation,
        ).in_community("community-a");
        assert_eq!(proposed.action, ActivityAction::FederationProposed);
        assert_eq!(proposed.target_type, TargetType::Federation);

        let accepted = ActivityRecord::new(
            "cpub1community_b",
            ActivityAction::FederationAccepted,
            "agreement-1",
            TargetType::Federation,
        );
        assert_eq!(accepted.action, ActivityAction::FederationAccepted);

        let withdrawn = ActivityRecord::new(
            "cpub1community_a",
            ActivityAction::FederationWithdrawn,
            "agreement-1",
            TargetType::Federation,
        );
        assert_eq!(withdrawn.action, ActivityAction::FederationWithdrawn);
    }

    #[test]
    fn serde_round_trip() {
        let mut timeline = Timeline::new("community-1");
        timeline.record(ActivityRecord::new(
            "cpub1alice",
            ActivityAction::Created,
            "asset-1",
            TargetType::Asset,
        ));
        timeline.mark_milestone(Milestone::new(
            "First asset",
            MilestoneSignificance::Minor,
            "cpub1alice",
        ));

        let json = serde_json::to_string(&timeline).unwrap();
        let restored: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.activity_count(), 1);
        assert_eq!(restored.milestone_count(), 1);
    }
}
