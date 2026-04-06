//! Quest health for the Undercroft observatory.
//!
//! Wraps Quest's `ObservatoryReport` -- all data is deidentified aggregate metrics.
//! NO pubkeys, NO individual activity, NO actor names. Only counts, rates,
//! and distributions.
//!
//! `health_score()` provides a single composite number (0.0-1.0) for "how
//! healthy is Quest on the network," used by Omny/Home at a glance.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use quest::ObservatoryReport;

/// Quest health metrics for the Undercroft observatory.
///
/// All data is deidentified aggregate metrics sourced from Quest's
/// `ObservatoryReport`. Provides convenience accessors and a composite
/// health score for the Omny/Home dashboard.
///
/// # Examples
///
/// ```
/// use undercroft::QuestHealth;
///
/// let health = QuestHealth::empty();
/// assert_eq!(health.total_participants(), 0);
/// assert_eq!(health.health_score(), 0.0);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestHealth {
    /// The underlying observatory report from Quest.
    pub report: ObservatoryReport,
    /// When this health snapshot was computed.
    pub computed_at: DateTime<Utc>,
}

impl QuestHealth {
    /// Build quest health from an observatory report.
    ///
    /// # Arguments
    ///
    /// * `report` - The observatory report from Quest (already deidentified).
    ///
    /// # Examples
    ///
    /// ```
    /// use quest::ObservatoryReport;
    /// use undercroft::QuestHealth;
    ///
    /// let report = ObservatoryReport::empty();
    /// let health = QuestHealth::from_report(&report);
    /// assert_eq!(health.total_participants(), 0);
    /// ```
    #[must_use]
    pub fn from_report(report: &ObservatoryReport) -> Self {
        Self {
            report: report.clone(),
            computed_at: Utc::now(),
        }
    }

    /// An empty quest health snapshot (no activity).
    ///
    /// # Examples
    ///
    /// ```
    /// use undercroft::QuestHealth;
    ///
    /// let health = QuestHealth::empty();
    /// assert_eq!(health.total_participants(), 0);
    /// assert_eq!(health.engagement_rate(), 0.0);
    /// assert_eq!(health.health_score(), 0.0);
    /// ```
    #[must_use]
    pub fn empty() -> Self {
        Self {
            report: ObservatoryReport::empty(),
            computed_at: Utc::now(),
        }
    }

    /// Total number of Quest participants.
    #[must_use]
    pub fn total_participants(&self) -> usize {
        self.report.total_participants
    }

    /// Engagement rate: participants / opt_in_count, if available.
    ///
    /// Returns 0.0 if no one has opted in. This measures what fraction
    /// of opted-in users are actively participating.
    #[must_use]
    pub fn engagement_rate(&self) -> f64 {
        if self.report.opt_in_count == 0 {
            return 0.0;
        }
        self.report.total_participants as f64 / self.report.opt_in_count as f64
    }

    /// Composite health score (0.0-1.0) based on activity levels.
    ///
    /// Weighted average of:
    /// - Participation signal (25%): are people participating?
    /// - Mission completion rate (25%): are missions being completed?
    /// - Challenge activity (25%): are challenges active and populated?
    /// - Cooperative activity (25%): are raids and mentorships happening?
    ///
    /// This gives Omny/Home a single number for "how healthy is Quest."
    ///
    /// # Examples
    ///
    /// ```
    /// use undercroft::QuestHealth;
    ///
    /// let health = QuestHealth::empty();
    /// assert_eq!(health.health_score(), 0.0);
    /// ```
    #[must_use]
    pub fn health_score(&self) -> f64 {
        let r = &self.report;

        // Participation signal: clamp at 1.0 if >= 10 participants
        let participation_signal = if r.total_participants == 0 {
            0.0
        } else {
            (r.total_participants as f64 / 10.0).min(1.0)
        };

        // Mission completion rate is already 0.0-1.0
        let mission_signal = r.average_completion_rate;

        // Challenge activity: clamp at 1.0 if >= 5 active challenges with participants
        let challenge_signal = if r.active_challenges == 0 {
            0.0
        } else {
            let base = (r.active_challenges as f64 / 5.0).min(1.0);
            let participant_factor = if r.total_challenge_participants > 0 {
                1.0
            } else {
                0.5
            };
            base * participant_factor
        };

        // Cooperative activity: clamp at 1.0 if raids + mentorships >= 3
        let coop_total = r.active_raids + r.active_mentorships;
        let coop_signal = if coop_total == 0 {
            0.0
        } else {
            (coop_total as f64 / 3.0).min(1.0)
        };

        // Weighted average
        let score = participation_signal * 0.25
            + mission_signal * 0.25
            + challenge_signal * 0.25
            + coop_signal * 0.25;

        // Clamp to [0.0, 1.0] for safety
        score.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn from_report_basic() {
        let report = ObservatoryReport::empty();
        let health = QuestHealth::from_report(&report);
        assert_eq!(health.total_participants(), 0);
        assert_eq!(health.engagement_rate(), 0.0);
        assert_eq!(health.health_score(), 0.0);
    }

    #[test]
    fn empty_quest_health() {
        let health = QuestHealth::empty();
        assert_eq!(health.total_participants(), 0);
        assert_eq!(health.engagement_rate(), 0.0);
        assert_eq!(health.health_score(), 0.0);
    }

    #[test]
    fn engagement_rate_with_participants() {
        let mut report = ObservatoryReport::empty();
        report.total_participants = 50;
        report.opt_in_count = 100;

        let health = QuestHealth::from_report(&report);
        assert!((health.engagement_rate() - 0.5).abs() < 0.001);
    }

    #[test]
    fn engagement_rate_full_participation() {
        let mut report = ObservatoryReport::empty();
        report.total_participants = 100;
        report.opt_in_count = 100;

        let health = QuestHealth::from_report(&report);
        assert!((health.engagement_rate() - 1.0).abs() < 0.001);
    }

    #[test]
    fn engagement_rate_zero_opt_in() {
        let mut report = ObservatoryReport::empty();
        report.total_participants = 0;
        report.opt_in_count = 0;

        let health = QuestHealth::from_report(&report);
        assert_eq!(health.engagement_rate(), 0.0);
    }

    #[test]
    fn health_score_all_zeros() {
        let health = QuestHealth::empty();
        assert_eq!(health.health_score(), 0.0);
    }

    #[test]
    fn health_score_full_participation_only() {
        let mut report = ObservatoryReport::empty();
        report.total_participants = 20; // should clamp to 1.0

        let health = QuestHealth::from_report(&report);
        // participation_signal = 1.0, others = 0.0
        // score = 1.0 * 0.25 = 0.25
        assert!((health.health_score() - 0.25).abs() < 0.001);
    }

    #[test]
    fn health_score_with_missions() {
        let mut report = ObservatoryReport::empty();
        report.total_participants = 10;
        report.average_completion_rate = 0.8;

        let health = QuestHealth::from_report(&report);
        // participation = 1.0 * 0.25 = 0.25
        // missions = 0.8 * 0.25 = 0.20
        // total = 0.45
        assert!((health.health_score() - 0.45).abs() < 0.001);
    }

    #[test]
    fn health_score_with_challenges() {
        let mut report = ObservatoryReport::empty();
        report.active_challenges = 5;
        report.total_challenge_participants = 10;

        let health = QuestHealth::from_report(&report);
        // challenges = 1.0 * 1.0 * 0.25 = 0.25
        assert!((health.health_score() - 0.25).abs() < 0.001);
    }

    #[test]
    fn health_score_with_coop() {
        let mut report = ObservatoryReport::empty();
        report.active_raids = 2;
        report.active_mentorships = 1;

        let health = QuestHealth::from_report(&report);
        // coop = (3/3) * 0.25 = 0.25
        assert!((health.health_score() - 0.25).abs() < 0.001);
    }

    #[test]
    fn health_score_fully_healthy() {
        let mut report = ObservatoryReport::empty();
        report.total_participants = 100;
        report.average_completion_rate = 1.0;
        report.active_challenges = 10;
        report.total_challenge_participants = 50;
        report.active_raids = 5;
        report.active_mentorships = 3;

        let health = QuestHealth::from_report(&report);
        assert!((health.health_score() - 1.0).abs() < 0.001);
    }

    #[test]
    fn health_score_clamped() {
        // Even with absurd values, score should never exceed 1.0
        let mut report = ObservatoryReport::empty();
        report.total_participants = 10000;
        report.average_completion_rate = 1.0;
        report.active_challenges = 1000;
        report.total_challenge_participants = 5000;
        report.active_raids = 100;
        report.active_mentorships = 100;

        let health = QuestHealth::from_report(&report);
        assert!(health.health_score() <= 1.0);
        assert!(health.health_score() >= 0.0);
    }

    #[test]
    fn health_score_challenges_without_participants() {
        let mut report = ObservatoryReport::empty();
        report.active_challenges = 5;
        report.total_challenge_participants = 0;

        let health = QuestHealth::from_report(&report);
        // challenges = 1.0 * 0.5 * 0.25 = 0.125
        assert!((health.health_score() - 0.125).abs() < 0.001);
    }

    #[test]
    fn serde_round_trip() {
        let mut report = ObservatoryReport::empty();
        report.total_participants = 42;
        report.total_cool_distributed = 5000;

        let health = QuestHealth::from_report(&report);
        let json = serde_json::to_string(&health).unwrap();
        let restored: QuestHealth = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.total_participants(), 42);
        assert_eq!(restored.report.total_cool_distributed, 5000);
    }

    #[test]
    fn serde_round_trip_empty() {
        let health = QuestHealth::empty();
        let json = serde_json::to_string(&health).unwrap();
        let restored: QuestHealth = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.total_participants(), 0);
        assert_eq!(restored.health_score(), 0.0);
    }

    #[test]
    fn no_individual_data_in_output() {
        let mut report = ObservatoryReport::empty();
        report.total_participants = 100;
        report.total_cool_distributed = 50000;

        let health = QuestHealth::from_report(&report);
        let json = serde_json::to_string(&health).unwrap();

        // Verify no individual identifiers
        assert!(!json.contains("cpub"));
        assert!(!json.contains("pubkey"));
        assert!(!json.contains("actor"));
    }
}
