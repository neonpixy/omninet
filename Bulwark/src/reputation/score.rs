use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::federation_scope::FederationScope;

/// A person's reputation — 5 factors, 0-1000 range.
///
/// Starting score: 500 (neutral). Each factor contributes 0-200.
/// Standing is derived from total score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reputation {
    pub pubkey: String,
    pub score: i32,
    pub factors: ReputationFactors,
    pub history: Vec<ReputationEvent>,
    pub computed_at: DateTime<Utc>,
}

impl Reputation {
    pub fn new(pubkey: impl Into<String>) -> Self {
        Self {
            pubkey: pubkey.into(),
            score: 500,
            factors: ReputationFactors::default(),
            history: Vec::new(),
            computed_at: Utc::now(),
        }
    }

    pub fn standing(&self) -> Standing {
        Standing::from_score(self.score)
    }

    /// Apply a reputation event.
    pub fn apply_event(&mut self, event: ReputationEvent) {
        self.score = (self.score + event.impact()).clamp(0, 1000);
        self.history.push(event);
        self.computed_at = Utc::now();
    }

    /// Recompute score from factors.
    pub fn recompute_from_factors(&mut self) {
        self.score = self.factors.total().clamp(0, 1000);
        self.computed_at = Utc::now();
    }

    /// Compute the standing based only on events from visible communities.
    ///
    /// Replays the event history through the federation scope filter,
    /// counting only events whose `community_id` is visible. Events
    /// with no `community_id` are always included (backward compat).
    ///
    /// Returns the standing and the scoped score. Does NOT mutate `self`.
    pub fn standing_scoped(&self, scope: &FederationScope) -> (Standing, i32) {
        if scope.is_unrestricted() {
            return (self.standing(), self.score);
        }

        let mut score = 500i32; // start from neutral
        for event in &self.history {
            if event.is_visible_in(scope) {
                score = (score + event.impact()).clamp(0, 1000);
            }
        }
        (Standing::from_score(score), score)
    }

    /// Count events from visible communities only.
    pub fn event_count_scoped(&self, scope: &FederationScope) -> usize {
        if scope.is_unrestricted() {
            return self.history.len();
        }
        self.history.iter().filter(|e| e.is_visible_in(scope)).count()
    }
}

/// 5 reputation factors — each 0-200.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReputationFactors {
    pub trade_history: i32,
    pub fulfillment_rate: i32,
    pub endorsements: i32,
    pub collective_membership: i32,
    pub tenure: i32,
}

impl ReputationFactors {
    pub fn total(&self) -> i32 {
        self.trade_history
            + self.fulfillment_rate
            + self.endorsements
            + self.collective_membership
            + self.tenure
    }
}

impl Default for ReputationFactors {
    fn default() -> Self {
        Self {
            trade_history: 100,
            fulfillment_rate: 100,
            endorsements: 100,
            collective_membership: 100,
            tenure: 100,
        }
    }
}

/// Standing derived from reputation score.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Standing {
    Excellent,
    Good,
    Neutral,
    Cautioned,
    Flagged,
}

impl Standing {
    pub fn from_score(score: i32) -> Self {
        match score {
            900..=1000 => Standing::Excellent,
            700..=899 => Standing::Good,
            400..=699 => Standing::Neutral,
            200..=399 => Standing::Cautioned,
            _ => Standing::Flagged,
        }
    }
}

/// A reputation-changing event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReputationEvent {
    pub event_type: ReputationEventType,
    pub occurred_at: DateTime<Utc>,
    pub reference: Option<String>,
    /// Which community this event originated from.
    ///
    /// `None` for events that predate federation scoping or are
    /// network-global. Events without a community_id are always
    /// visible regardless of federation scope.
    #[serde(default)]
    pub community_id: Option<String>,
}

impl ReputationEvent {
    pub fn new(event_type: ReputationEventType) -> Self {
        Self {
            event_type,
            occurred_at: Utc::now(),
            reference: None,
            community_id: None,
        }
    }

    /// Create a reputation event tagged with a community.
    pub fn new_in_community(
        event_type: ReputationEventType,
        community_id: impl Into<String>,
    ) -> Self {
        Self {
            event_type,
            occurred_at: Utc::now(),
            reference: None,
            community_id: Some(community_id.into()),
        }
    }

    pub fn impact(&self) -> i32 {
        self.event_type.impact()
    }

    /// Whether this event is visible within the given federation scope.
    ///
    /// Events with no `community_id` are always visible (backward compat).
    pub fn is_visible_in(&self, scope: &FederationScope) -> bool {
        match &self.community_id {
            None => true,
            Some(id) => scope.is_visible(id),
        }
    }
}

/// Types of reputation events and their impact.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ReputationEventType {
    TradeCompleted,
    RedemptionFulfilled,
    EndorsementReceived,
    CollectiveJoined,
    TenureMilestone,
    RedemptionDisputed,
    FraudReported,
    FraudConfirmed,
    CollectiveExpelled,
    SuspiciousPattern,
}

impl ReputationEventType {
    pub fn impact(&self) -> i32 {
        match self {
            ReputationEventType::TradeCompleted => 5,
            ReputationEventType::RedemptionFulfilled => 10,
            ReputationEventType::EndorsementReceived => 20,
            ReputationEventType::CollectiveJoined => 15,
            ReputationEventType::TenureMilestone => 25,
            ReputationEventType::RedemptionDisputed => -30,
            ReputationEventType::FraudReported => -50,
            ReputationEventType::FraudConfirmed => -200,
            ReputationEventType::CollectiveExpelled => -75,
            ReputationEventType::SuspiciousPattern => -40,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_reputation_is_neutral() {
        let rep = Reputation::new("alice");
        assert_eq!(rep.score, 500);
        assert_eq!(rep.standing(), Standing::Neutral);
    }

    #[test]
    fn positive_events_increase_score() {
        let mut rep = Reputation::new("alice");
        rep.apply_event(ReputationEvent::new(ReputationEventType::TradeCompleted));
        assert_eq!(rep.score, 505);
        rep.apply_event(ReputationEvent::new(ReputationEventType::EndorsementReceived));
        assert_eq!(rep.score, 525);
    }

    #[test]
    fn negative_events_decrease_score() {
        let mut rep = Reputation::new("alice");
        rep.apply_event(ReputationEvent::new(ReputationEventType::FraudConfirmed));
        assert_eq!(rep.score, 300);
        assert_eq!(rep.standing(), Standing::Cautioned);
    }

    #[test]
    fn score_clamped_to_range() {
        let mut rep = Reputation::new("alice");
        rep.score = 950;
        rep.apply_event(ReputationEvent::new(ReputationEventType::TenureMilestone)); // +25
        rep.apply_event(ReputationEvent::new(ReputationEventType::TenureMilestone)); // +25
        assert_eq!(rep.score, 1000); // clamped

        let mut rep2 = Reputation::new("bob");
        rep2.score = 50;
        rep2.apply_event(ReputationEvent::new(ReputationEventType::FraudConfirmed)); // -200
        assert_eq!(rep2.score, 0); // clamped
    }

    #[test]
    fn standing_thresholds() {
        assert_eq!(Standing::from_score(950), Standing::Excellent);
        assert_eq!(Standing::from_score(750), Standing::Good);
        assert_eq!(Standing::from_score(500), Standing::Neutral);
        assert_eq!(Standing::from_score(300), Standing::Cautioned);
        assert_eq!(Standing::from_score(100), Standing::Flagged);
    }

    #[test]
    fn factors_total() {
        let factors = ReputationFactors::default();
        assert_eq!(factors.total(), 500);
    }

    // ── Federation Scope ──────────────────────────────────────────────────

    #[test]
    fn standing_scoped_unrestricted_matches_standing() {
        let mut rep = Reputation::new("alice");
        rep.apply_event(ReputationEvent::new_in_community(
            ReputationEventType::TradeCompleted,
            "community_a",
        ));
        rep.apply_event(ReputationEvent::new_in_community(
            ReputationEventType::EndorsementReceived,
            "community_b",
        ));

        let scope = FederationScope::new();
        let (standing, score) = rep.standing_scoped(&scope);
        assert_eq!(standing, rep.standing());
        assert_eq!(score, rep.score);
    }

    #[test]
    fn standing_scoped_filters_by_community() {
        let mut rep = Reputation::new("alice");
        // +5 from community_a
        rep.apply_event(ReputationEvent::new_in_community(
            ReputationEventType::TradeCompleted,
            "community_a",
        ));
        // -200 from community_b (not in scope)
        rep.apply_event(ReputationEvent::new_in_community(
            ReputationEventType::FraudConfirmed,
            "community_b",
        ));

        // Full score: 500 + 5 - 200 = 305
        assert_eq!(rep.score, 305);
        assert_eq!(rep.standing(), Standing::Cautioned);

        // Scoped to only community_a: 500 + 5 = 505
        let scope = FederationScope::from_communities(["community_a"]);
        let (standing, score) = rep.standing_scoped(&scope);
        assert_eq!(score, 505);
        assert_eq!(standing, Standing::Neutral);
    }

    #[test]
    fn standing_scoped_includes_untagged_events() {
        let mut rep = Reputation::new("alice");
        // Untagged event (legacy / global) — always visible.
        rep.apply_event(ReputationEvent::new(ReputationEventType::TenureMilestone)); // +25
        // Tagged event from outside scope.
        rep.apply_event(ReputationEvent::new_in_community(
            ReputationEventType::FraudReported,
            "community_b",
        )); // -50

        // Full: 500 + 25 - 50 = 475
        assert_eq!(rep.score, 475);

        // Scoped to community_a: 500 + 25 = 525 (untagged included, community_b excluded)
        let scope = FederationScope::from_communities(["community_a"]);
        let (_, score) = rep.standing_scoped(&scope);
        assert_eq!(score, 525);
    }

    #[test]
    fn event_count_scoped() {
        let mut rep = Reputation::new("alice");
        rep.apply_event(ReputationEvent::new(ReputationEventType::TradeCompleted));
        rep.apply_event(ReputationEvent::new_in_community(
            ReputationEventType::TradeCompleted,
            "alpha",
        ));
        rep.apply_event(ReputationEvent::new_in_community(
            ReputationEventType::TradeCompleted,
            "beta",
        ));
        rep.apply_event(ReputationEvent::new_in_community(
            ReputationEventType::TradeCompleted,
            "gamma",
        ));

        assert_eq!(rep.event_count_scoped(&FederationScope::new()), 4);
        // Scoped to alpha: untagged (1) + alpha (1) = 2
        assert_eq!(
            rep.event_count_scoped(&FederationScope::from_communities(["alpha"])),
            2
        );
    }

    #[test]
    fn new_in_community_creates_tagged_event() {
        let event = ReputationEvent::new_in_community(
            ReputationEventType::TradeCompleted,
            "test_community",
        );
        assert_eq!(event.community_id, Some("test_community".to_string()));
        assert_eq!(event.impact(), 5);
    }

    #[test]
    fn event_visibility_in_scope() {
        let scoped = FederationScope::from_communities(["alpha"]);

        let untagged = ReputationEvent::new(ReputationEventType::TradeCompleted);
        assert!(untagged.is_visible_in(&scoped), "untagged always visible");

        let alpha = ReputationEvent::new_in_community(
            ReputationEventType::TradeCompleted,
            "alpha",
        );
        assert!(alpha.is_visible_in(&scoped));

        let beta = ReputationEvent::new_in_community(
            ReputationEventType::TradeCompleted,
            "beta",
        );
        assert!(!beta.is_visible_in(&scoped));
    }

    #[test]
    fn reputation_event_serde_backward_compat() {
        // Old-format JSON without community_id should deserialize fine.
        let json = r#"{"event_type":"TradeCompleted","occurred_at":"2025-01-01T00:00:00Z","reference":null}"#;
        let event: ReputationEvent = serde_json::from_str(json).expect("deserialize legacy event");
        assert_eq!(event.community_id, None);
        assert_eq!(event.event_type, ReputationEventType::TradeCompleted);
    }
}
