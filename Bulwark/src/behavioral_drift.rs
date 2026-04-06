//! # Behavioral Drift Detection
//!
//! Detect long-con attacks by tracking structural changes in behavior over time.
//! This is NOT surveillance of WHAT people do — it is detection of WHETHER their
//! behavioral SHAPE changes.
//!
//! ## Philosophy
//!
//! - Does NOT read content. Only counts actions by type.
//! - Does NOT profile people. Only compares them to their OWN past.
//! - Does NOT punish change. Alerts inform, they don't restrict.
//! - Detects the SHAPE of manipulation without surveilling the CONTENT.
//!
//! A person who genuinely grows into a leadership role will show drift — and
//! that's fine. The community evaluates whether the drift is natural growth
//! or concerning.
//!
//! ## Integration
//!
//! Behavioral drift + power concentration = strong signal. If one person shows
//! both high drift AND high power concentration, the combined alert is elevated
//! to critical. See R2E (Power Concentration Index) for the other half.
//!
//! ## Covenant Alignment
//!
//! **Dignity** — shape-only detection preserves the inherent worth of every person.
//! **Sovereignty** — people choose how they engage; drift informs, never restricts.
//! **Consent** — all monitoring is structural and deidentified; no content inspection.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::federation_scope::FederationScope;

// ---------------------------------------------------------------------------
// Activity — the raw input to drift computation
// ---------------------------------------------------------------------------

/// A single recorded activity. This is the raw input fed into drift computation.
///
/// Activities carry only structural metadata — the action type and when it
/// occurred. They never carry content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Activity {
    /// The category of action (e.g. "governance.vote", "content.create", "social.message").
    pub action_type: String,
    /// When the activity occurred.
    pub timestamp: DateTime<Utc>,
}

impl Activity {
    /// Create a new activity record.
    pub fn new(action_type: impl Into<String>, timestamp: DateTime<Utc>) -> Self {
        Self {
            action_type: action_type.into(),
            timestamp,
        }
    }
}

// ---------------------------------------------------------------------------
// BaselineMetrics — the statistical fingerprint of someone's behavior
// ---------------------------------------------------------------------------

/// Structural metrics that describe the SHAPE of someone's behavior over a
/// period. Every metric is a rate or proportion — never raw content.
///
/// These metrics are computed from [`Activity`] records by [`DriftComputer`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BaselineMetrics {
    /// Actions per week (from Yoke ActivityRecords).
    pub action_frequency: f64,
    /// Proportion of each action type (values sum to ~1.0).
    pub action_type_distribution: HashMap<String, f64>,
    /// Percentage of governance proposals voted on (0.0–1.0).
    pub governance_participation_rate: f64,
    /// .idea files created per week.
    pub content_creation_rate: f64,
    /// Equipment interactions per week.
    pub social_engagement_rate: f64,
    /// Governance role transitions during the period.
    pub role_changes: usize,
}

impl BaselineMetrics {
    /// Create metrics with all zeros — useful for testing and initialization.
    pub fn zero() -> Self {
        Self {
            action_frequency: 0.0,
            action_type_distribution: HashMap::new(),
            governance_participation_rate: 0.0,
            content_creation_rate: 0.0,
            social_engagement_rate: 0.0,
            role_changes: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// BehavioralBaseline — the reference point for drift
// ---------------------------------------------------------------------------

/// A person's behavioral baseline within a specific community.
///
/// Established over the first `baseline_period` of membership (default 180 days).
/// Once established, this becomes the reference against which future behavior
/// is compared.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BehavioralBaseline {
    /// The person's public key.
    pub pubkey: String,
    /// The community this baseline applies to.
    pub community_id: String,
    /// Duration of the baseline period in seconds.
    pub baseline_period_secs: u64,
    /// The computed baseline metrics.
    pub metrics: BaselineMetrics,
    /// When this baseline was established.
    pub established_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// DriftFactor — one axis of behavioral change
// ---------------------------------------------------------------------------

/// A single dimension of behavioral drift, explaining what changed and by how much.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriftFactor {
    /// Human-readable name of the metric (e.g. "action_frequency").
    pub metric_name: String,
    /// The baseline value for this metric.
    pub baseline_value: f64,
    /// The current value for this metric.
    pub current_value: f64,
    /// Normalized deviation (0.0 = identical, 1.0 = maximum divergence).
    pub deviation: f64,
}

// ---------------------------------------------------------------------------
// BehavioralDrift — the result of drift computation
// ---------------------------------------------------------------------------

/// The result of comparing current behavior against a baseline.
///
/// `drift_score` ranges from 0.0 (identical to baseline) to 1.0 (completely
/// different). The score is the weighted average of all [`DriftFactor`] deviations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BehavioralDrift {
    /// The person's public key.
    pub pubkey: String,
    /// The community this drift applies to.
    pub community_id: String,
    /// The baseline this was compared against.
    pub baseline: BehavioralBaseline,
    /// Current-period metrics.
    pub current: BaselineMetrics,
    /// Overall drift score (0.0 = identical, 1.0 = completely different).
    pub drift_score: f64,
    /// Per-metric breakdown of what changed.
    pub drift_factors: Vec<DriftFactor>,
    /// When this drift was computed.
    pub computed_at: DateTime<Utc>,
}

impl BehavioralDrift {
    /// Whether the drift score exceeds the alert threshold.
    pub fn exceeds_alert(&self, config: &DriftConfig) -> bool {
        self.drift_score >= config.alert_threshold
    }

    /// Whether the drift score exceeds the critical threshold.
    pub fn exceeds_critical(&self, config: &DriftConfig) -> bool {
        self.drift_score >= config.critical_threshold
    }
}

// ---------------------------------------------------------------------------
// DriftAlert — what gets surfaced to governance
// ---------------------------------------------------------------------------

/// The severity level of a drift alert.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DriftAlertLevel {
    /// Drift is within normal range. No action needed.
    Normal,
    /// Drift exceeds the alert threshold. Community may want to review.
    Alert,
    /// Drift exceeds the critical threshold. Governance review recommended.
    Critical,
}

/// A drift alert surfaced for community awareness.
///
/// Alerts inform — they never restrict. A person showing high drift may be
/// genuinely growing into leadership, or may be engaged in a long con. The
/// community evaluates which.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriftAlert {
    /// The drift computation that triggered this alert.
    pub drift: BehavioralDrift,
    /// The alert severity level.
    pub level: DriftAlertLevel,
    /// Top contributing factors, sorted by deviation (highest first).
    pub top_factors: Vec<DriftFactor>,
}

// ---------------------------------------------------------------------------
// DriftConfig — per-community configuration
// ---------------------------------------------------------------------------

/// Per-community drift detection configuration, stored in the Charter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriftConfig {
    /// Drift score above this triggers an alert (default 0.6).
    pub alert_threshold: f64,
    /// Drift score above this triggers a governance review (default 0.8).
    pub critical_threshold: f64,
    /// How often to recompute drift, in days (default 30).
    pub computation_interval_days: u64,
    /// How long the baseline period lasts, in days (default 180).
    pub baseline_period_days: u64,
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            alert_threshold: 0.6,
            critical_threshold: 0.8,
            computation_interval_days: 30,
            baseline_period_days: 180,
        }
    }
}

impl DriftConfig {
    /// Validate that thresholds are sensible.
    pub fn validate(&self) -> Result<(), String> {
        if self.alert_threshold <= 0.0 || self.alert_threshold >= 1.0 {
            return Err("alert_threshold must be between 0.0 and 1.0 exclusive".into());
        }
        if self.critical_threshold <= 0.0 || self.critical_threshold >= 1.0 {
            return Err("critical_threshold must be between 0.0 and 1.0 exclusive".into());
        }
        if self.alert_threshold >= self.critical_threshold {
            return Err("alert_threshold must be less than critical_threshold".into());
        }
        if self.computation_interval_days == 0 {
            return Err("computation_interval_days must be > 0".into());
        }
        if self.baseline_period_days == 0 {
            return Err("baseline_period_days must be > 0".into());
        }
        Ok(())
    }

    /// Baseline period as seconds (for storage in `BehavioralBaseline`).
    pub fn baseline_period_secs(&self) -> u64 {
        self.baseline_period_days * 24 * 60 * 60
    }
}

// ---------------------------------------------------------------------------
// DriftComputer — the pure computation engine
// ---------------------------------------------------------------------------

/// Pure-function engine for computing behavioral drift.
///
/// `DriftComputer` is stateless. All inputs are provided per call, all outputs
/// are returned. No I/O, no side effects, no storage. This makes it trivially
/// testable and safe to run anywhere.
pub struct DriftComputer;

impl DriftComputer {
    // Weights for each metric dimension in the final drift score.
    // These sum to 1.0 and are tuned so that governance-related changes
    // weigh more heavily (they are the primary long-con signal).
    const WEIGHT_ACTION_FREQUENCY: f64 = 0.15;
    const WEIGHT_ACTION_DISTRIBUTION: f64 = 0.20;
    const WEIGHT_GOVERNANCE: f64 = 0.25;
    const WEIGHT_CONTENT: f64 = 0.15;
    const WEIGHT_SOCIAL: f64 = 0.15;
    const WEIGHT_ROLE_CHANGES: f64 = 0.10;

    /// Compute behavioral drift by comparing current-period activities against
    /// a stored baseline.
    ///
    /// This is a **pure function**: same inputs always produce the same outputs
    /// (modulo the `computed_at` timestamp which uses `Utc::now()`).
    ///
    /// # Arguments
    ///
    /// * `baseline` — The person's established behavioral baseline.
    /// * `recent_activities` — Activities from the current computation period.
    /// * `period_weeks` — How many weeks the recent activities span.
    /// * `governance_proposals_available` — Total proposals available to vote
    ///   on during the period (used to compute participation rate).
    /// * `governance_votes_cast` — How many the person voted on.
    /// * `role_changes` — Role transitions during the current period.
    ///
    /// # Returns
    ///
    /// A [`BehavioralDrift`] with the overall drift score and per-factor breakdown.
    pub fn compute(
        baseline: &BehavioralBaseline,
        recent_activities: &[Activity],
        period_weeks: f64,
        governance_proposals_available: u64,
        governance_votes_cast: u64,
        role_changes: usize,
    ) -> BehavioralDrift {
        let current = Self::compute_metrics(
            recent_activities,
            period_weeks,
            governance_proposals_available,
            governance_votes_cast,
            role_changes,
        );

        let factors = Self::compute_factors(&baseline.metrics, &current);
        let drift_score = Self::weighted_score(&factors);

        BehavioralDrift {
            pubkey: baseline.pubkey.clone(),
            community_id: baseline.community_id.clone(),
            baseline: baseline.clone(),
            current,
            drift_score,
            drift_factors: factors,
            computed_at: Utc::now(),
        }
    }

    /// Compute [`BaselineMetrics`] from raw activity data.
    ///
    /// This is exposed publicly so callers can build a baseline from the same
    /// logic used for current-period metrics.
    pub fn compute_metrics(
        activities: &[Activity],
        period_weeks: f64,
        governance_proposals_available: u64,
        governance_votes_cast: u64,
        role_changes: usize,
    ) -> BaselineMetrics {
        if period_weeks <= 0.0 {
            return BaselineMetrics::zero();
        }

        let total = activities.len() as f64;
        let action_frequency = total / period_weeks;

        // Count occurrences of each action type.
        let mut type_counts: HashMap<String, f64> = HashMap::new();
        for activity in activities {
            *type_counts.entry(activity.action_type.clone()).or_insert(0.0) += 1.0;
        }

        // Normalize to proportions.
        let action_type_distribution = if total > 0.0 {
            type_counts
                .into_iter()
                .map(|(k, v)| (k, v / total))
                .collect()
        } else {
            HashMap::new()
        };

        // Governance participation: fraction of available proposals voted on.
        let governance_participation_rate = if governance_proposals_available > 0 {
            (governance_votes_cast as f64 / governance_proposals_available as f64).min(1.0)
        } else {
            0.0
        };

        // Content creation: count "content.*" actions per week.
        let content_actions = activities
            .iter()
            .filter(|a| a.action_type.starts_with("content."))
            .count() as f64;
        let content_creation_rate = content_actions / period_weeks;

        // Social engagement: count "social.*" actions per week.
        let social_actions = activities
            .iter()
            .filter(|a| a.action_type.starts_with("social."))
            .count() as f64;
        let social_engagement_rate = social_actions / period_weeks;

        BaselineMetrics {
            action_frequency,
            action_type_distribution,
            governance_participation_rate,
            content_creation_rate,
            social_engagement_rate,
            role_changes,
        }
    }

    /// Compute per-factor deviations between baseline and current metrics.
    fn compute_factors(baseline: &BaselineMetrics, current: &BaselineMetrics) -> Vec<DriftFactor> {
        let mut factors = Vec::with_capacity(6);

        // 1. Action frequency deviation.
        factors.push(DriftFactor {
            metric_name: "action_frequency".into(),
            baseline_value: baseline.action_frequency,
            current_value: current.action_frequency,
            deviation: Self::rate_deviation(baseline.action_frequency, current.action_frequency),
        });

        // 2. Action type distribution deviation (Jensen-Shannon style).
        let dist_dev = Self::distribution_deviation(
            &baseline.action_type_distribution,
            &current.action_type_distribution,
        );
        factors.push(DriftFactor {
            metric_name: "action_type_distribution".into(),
            baseline_value: 0.0, // distributions don't reduce to a single scalar
            current_value: dist_dev,
            deviation: dist_dev,
        });

        // 3. Governance participation.
        factors.push(DriftFactor {
            metric_name: "governance_participation_rate".into(),
            baseline_value: baseline.governance_participation_rate,
            current_value: current.governance_participation_rate,
            deviation: Self::rate_deviation(
                baseline.governance_participation_rate,
                current.governance_participation_rate,
            ),
        });

        // 4. Content creation rate.
        factors.push(DriftFactor {
            metric_name: "content_creation_rate".into(),
            baseline_value: baseline.content_creation_rate,
            current_value: current.content_creation_rate,
            deviation: Self::rate_deviation(
                baseline.content_creation_rate,
                current.content_creation_rate,
            ),
        });

        // 5. Social engagement rate.
        factors.push(DriftFactor {
            metric_name: "social_engagement_rate".into(),
            baseline_value: baseline.social_engagement_rate,
            current_value: current.social_engagement_rate,
            deviation: Self::rate_deviation(
                baseline.social_engagement_rate,
                current.social_engagement_rate,
            ),
        });

        // 6. Role changes.
        factors.push(DriftFactor {
            metric_name: "role_changes".into(),
            baseline_value: baseline.role_changes as f64,
            current_value: current.role_changes as f64,
            deviation: Self::rate_deviation(
                baseline.role_changes as f64,
                current.role_changes as f64,
            ),
        });

        factors
    }

    /// Weighted average of all factor deviations. Result is clamped to [0.0, 1.0].
    fn weighted_score(factors: &[DriftFactor]) -> f64 {
        // Map factor names to weights. Use ordered iteration for determinism.
        let weights: &[(&str, f64)] = &[
            ("action_frequency", Self::WEIGHT_ACTION_FREQUENCY),
            ("action_type_distribution", Self::WEIGHT_ACTION_DISTRIBUTION),
            ("governance_participation_rate", Self::WEIGHT_GOVERNANCE),
            ("content_creation_rate", Self::WEIGHT_CONTENT),
            ("social_engagement_rate", Self::WEIGHT_SOCIAL),
            ("role_changes", Self::WEIGHT_ROLE_CHANGES),
        ];

        let mut score = 0.0;
        for (name, weight) in weights {
            if let Some(factor) = factors.iter().find(|f| f.metric_name == *name) {
                score += factor.deviation * weight;
            }
        }

        score.clamp(0.0, 1.0)
    }

    /// Compute normalized deviation between two rate values.
    ///
    /// Uses relative change normalized to [0, 1]: `|a - b| / max(a, b, 1.0)`.
    /// The floor of 1.0 in the denominator prevents division by zero when both
    /// values are near zero, and produces sensible results for small values.
    fn rate_deviation(baseline: f64, current: f64) -> f64 {
        let diff = (baseline - current).abs();
        let max_val = baseline.abs().max(current.abs()).max(1.0);
        (diff / max_val).clamp(0.0, 1.0)
    }

    /// Compute distance between two probability distributions.
    ///
    /// Uses a simplified L1 distance (total variation distance): the sum of
    /// absolute differences across all keys from both distributions, divided
    /// by 2 (since both should sum to ~1.0, max L1 = 2.0). Result in [0, 1].
    ///
    /// When one distribution is empty, the distance is 1.0 if the other is
    /// non-empty, or 0.0 if both are empty.
    fn distribution_deviation(
        baseline: &HashMap<String, f64>,
        current: &HashMap<String, f64>,
    ) -> f64 {
        if baseline.is_empty() && current.is_empty() {
            return 0.0;
        }
        if baseline.is_empty() || current.is_empty() {
            return 1.0;
        }

        // Collect all keys from both distributions.
        let mut all_keys: Vec<&String> = baseline.keys().collect();
        for k in current.keys() {
            if !baseline.contains_key(k) {
                all_keys.push(k);
            }
        }

        let mut l1 = 0.0;
        for key in &all_keys {
            let b = baseline.get(*key).copied().unwrap_or(0.0);
            let c = current.get(*key).copied().unwrap_or(0.0);
            l1 += (b - c).abs();
        }

        // L1 distance between two distributions that each sum to 1.0 has
        // max value 2.0 (completely disjoint supports). Divide by 2 to normalize.
        (l1 / 2.0).clamp(0.0, 1.0)
    }

    /// Evaluate a [`BehavioralDrift`] against a [`DriftConfig`] and produce
    /// a [`DriftAlert`].
    pub fn evaluate(drift: &BehavioralDrift, config: &DriftConfig) -> DriftAlert {
        let level = if drift.drift_score >= config.critical_threshold {
            DriftAlertLevel::Critical
        } else if drift.drift_score >= config.alert_threshold {
            DriftAlertLevel::Alert
        } else {
            DriftAlertLevel::Normal
        };

        // Top factors sorted by deviation descending.
        let mut top_factors = drift.drift_factors.clone();
        top_factors.sort_by(|a, b| {
            b.deviation
                .partial_cmp(&a.deviation)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        DriftAlert {
            drift: drift.clone(),
            level,
            top_factors,
        }
    }

    /// Filter a set of drift alerts to only those from visible communities.
    ///
    /// Drift is already per-community — this filters which communities'
    /// alerts are considered when making cross-community safety decisions.
    ///
    /// When the scope is unrestricted, all alerts pass through.
    pub fn filter_alerts_scoped<'a>(
        alerts: &'a [DriftAlert],
        scope: &FederationScope,
    ) -> Vec<&'a DriftAlert> {
        if scope.is_unrestricted() {
            return alerts.iter().collect();
        }
        alerts
            .iter()
            .filter(|alert| scope.is_visible(&alert.drift.community_id))
            .collect()
    }

    /// Compute the maximum drift score across all visible communities.
    ///
    /// Useful for answering: "Is this person showing concerning drift in
    /// any community within this federation?"
    ///
    /// Returns `None` if no drift data exists for visible communities.
    pub fn max_drift_scoped(
        drifts: &[BehavioralDrift],
        scope: &FederationScope,
    ) -> Option<f64> {
        drifts
            .iter()
            .filter(|d| scope.is_visible(&d.community_id))
            .map(|d| d.drift_score)
            .reduce(f64::max)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_baseline(metrics: BaselineMetrics) -> BehavioralBaseline {
        BehavioralBaseline {
            pubkey: "test_pubkey".into(),
            community_id: "test_community".into(),
            baseline_period_secs: 180 * 24 * 60 * 60, // 180 days
            metrics,
            established_at: Utc::now() - Duration::days(365),
        }
    }

    fn make_activities(types: &[&str], count_each: usize) -> Vec<Activity> {
        let now = Utc::now();
        let mut activities = Vec::new();
        for (i, action_type) in types.iter().enumerate() {
            for j in 0..count_each {
                activities.push(Activity::new(
                    *action_type,
                    now - Duration::hours((i * count_each + j) as i64),
                ));
            }
        }
        activities
    }

    fn baseline_with_rates(
        action_freq: f64,
        governance: f64,
        content: f64,
        social: f64,
        role_changes: usize,
        distribution: HashMap<String, f64>,
    ) -> BaselineMetrics {
        BaselineMetrics {
            action_frequency: action_freq,
            action_type_distribution: distribution,
            governance_participation_rate: governance,
            content_creation_rate: content,
            social_engagement_rate: social,
            role_changes,
        }
    }

    // -----------------------------------------------------------------------
    // DriftConfig tests
    // -----------------------------------------------------------------------

    #[test]
    fn default_config() {
        let config = DriftConfig::default();
        assert!((config.alert_threshold - 0.6).abs() < f64::EPSILON);
        assert!((config.critical_threshold - 0.8).abs() < f64::EPSILON);
        assert_eq!(config.computation_interval_days, 30);
        assert_eq!(config.baseline_period_days, 180);
    }

    #[test]
    fn config_validation_valid() {
        let config = DriftConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_validation_alert_too_high() {
        let config = DriftConfig {
            alert_threshold: 0.9,
            critical_threshold: 0.8,
            ..DriftConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_validation_thresholds_equal() {
        let config = DriftConfig {
            alert_threshold: 0.7,
            critical_threshold: 0.7,
            ..DriftConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_validation_zero_interval() {
        let config = DriftConfig {
            computation_interval_days: 0,
            ..DriftConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_validation_zero_baseline_period() {
        let config = DriftConfig {
            baseline_period_days: 0,
            ..DriftConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_baseline_period_secs() {
        let config = DriftConfig::default();
        assert_eq!(config.baseline_period_secs(), 180 * 24 * 60 * 60);
    }

    // -----------------------------------------------------------------------
    // BaselineMetrics tests
    // -----------------------------------------------------------------------

    #[test]
    fn zero_metrics() {
        let m = BaselineMetrics::zero();
        assert!((m.action_frequency - 0.0).abs() < f64::EPSILON);
        assert!(m.action_type_distribution.is_empty());
        assert!((m.governance_participation_rate - 0.0).abs() < f64::EPSILON);
        assert!((m.content_creation_rate - 0.0).abs() < f64::EPSILON);
        assert!((m.social_engagement_rate - 0.0).abs() < f64::EPSILON);
        assert_eq!(m.role_changes, 0);
    }

    // -----------------------------------------------------------------------
    // DriftComputer::compute_metrics tests
    // -----------------------------------------------------------------------

    #[test]
    fn compute_metrics_empty_activities() {
        let metrics = DriftComputer::compute_metrics(&[], 4.0, 10, 5, 0);
        assert!((metrics.action_frequency - 0.0).abs() < f64::EPSILON);
        assert!(metrics.action_type_distribution.is_empty());
        assert!((metrics.governance_participation_rate - 0.5).abs() < f64::EPSILON);
        assert!((metrics.content_creation_rate - 0.0).abs() < f64::EPSILON);
        assert!((metrics.social_engagement_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_metrics_zero_period() {
        let activities = make_activities(&["social.message"], 10);
        let metrics = DriftComputer::compute_metrics(&activities, 0.0, 0, 0, 0);
        // Should return zero metrics to avoid division by zero.
        assert!((metrics.action_frequency - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_metrics_basic() {
        let activities = make_activities(&["content.create", "social.message"], 10);
        let metrics = DriftComputer::compute_metrics(&activities, 4.0, 20, 10, 1);

        // 20 activities / 4 weeks = 5.0 per week.
        assert!((metrics.action_frequency - 5.0).abs() < f64::EPSILON);

        // 50% content, 50% social.
        assert!((metrics.action_type_distribution["content.create"] - 0.5).abs() < f64::EPSILON);
        assert!((metrics.action_type_distribution["social.message"] - 0.5).abs() < f64::EPSILON);

        // 10 / 20 = 0.5.
        assert!((metrics.governance_participation_rate - 0.5).abs() < f64::EPSILON);

        // 10 content actions / 4 weeks = 2.5.
        assert!((metrics.content_creation_rate - 2.5).abs() < f64::EPSILON);

        // 10 social actions / 4 weeks = 2.5.
        assert!((metrics.social_engagement_rate - 2.5).abs() < f64::EPSILON);

        assert_eq!(metrics.role_changes, 1);
    }

    #[test]
    fn compute_metrics_governance_caps_at_one() {
        // More votes than proposals (edge case).
        let metrics = DriftComputer::compute_metrics(&[], 4.0, 5, 10, 0);
        assert!((metrics.governance_participation_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_metrics_no_governance_proposals() {
        let metrics = DriftComputer::compute_metrics(&[], 4.0, 0, 0, 0);
        assert!((metrics.governance_participation_rate - 0.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Rate deviation tests
    // -----------------------------------------------------------------------

    #[test]
    fn rate_deviation_identical() {
        assert!((DriftComputer::rate_deviation(5.0, 5.0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn rate_deviation_zero_baseline() {
        // 0 to 3: diff=3, max(0,3,1)=3, 3/3=1.0
        assert!((DriftComputer::rate_deviation(0.0, 3.0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn rate_deviation_both_zero() {
        assert!((DriftComputer::rate_deviation(0.0, 0.0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn rate_deviation_small_values() {
        // 0.1 to 0.3: diff=0.2, max(0.1, 0.3, 1.0)=1.0, 0.2/1.0=0.2
        let dev = DriftComputer::rate_deviation(0.1, 0.3);
        assert!((dev - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn rate_deviation_large_change() {
        // 10 to 1: diff=9, max(10, 1, 1)=10, 9/10=0.9
        let dev = DriftComputer::rate_deviation(10.0, 1.0);
        assert!((dev - 0.9).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Distribution deviation tests
    // -----------------------------------------------------------------------

    #[test]
    fn distribution_deviation_identical() {
        let mut d = HashMap::new();
        d.insert("a".into(), 0.5);
        d.insert("b".into(), 0.5);
        let dev = DriftComputer::distribution_deviation(&d, &d);
        assert!((dev - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn distribution_deviation_completely_different() {
        let mut a = HashMap::new();
        a.insert("x".into(), 1.0);
        let mut b = HashMap::new();
        b.insert("y".into(), 1.0);
        let dev = DriftComputer::distribution_deviation(&a, &b);
        // L1 = |1-0| + |0-1| = 2, normalized = 1.0.
        assert!((dev - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn distribution_deviation_both_empty() {
        let dev = DriftComputer::distribution_deviation(&HashMap::new(), &HashMap::new());
        assert!((dev - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn distribution_deviation_one_empty() {
        let mut a = HashMap::new();
        a.insert("x".into(), 1.0);
        let dev = DriftComputer::distribution_deviation(&a, &HashMap::new());
        assert!((dev - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn distribution_deviation_partial_overlap() {
        let mut a = HashMap::new();
        a.insert("x".into(), 0.6);
        a.insert("y".into(), 0.4);

        let mut b = HashMap::new();
        b.insert("x".into(), 0.3);
        b.insert("y".into(), 0.3);
        b.insert("z".into(), 0.4);

        // L1 = |0.6-0.3| + |0.4-0.3| + |0.0-0.4| = 0.3 + 0.1 + 0.4 = 0.8
        // Normalized = 0.8 / 2.0 = 0.4
        let dev = DriftComputer::distribution_deviation(&a, &b);
        assert!((dev - 0.4).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Full drift computation tests
    // -----------------------------------------------------------------------

    #[test]
    fn identical_behavior_zero_drift() {
        let mut dist = HashMap::new();
        dist.insert("content.create".into(), 0.5);
        dist.insert("social.message".into(), 0.5);

        let metrics = baseline_with_rates(5.0, 0.5, 2.5, 2.5, 0, dist);
        let baseline = make_baseline(metrics);

        // Reproduce the exact same activity pattern.
        let activities = make_activities(&["content.create", "social.message"], 10);
        let drift = DriftComputer::compute(&baseline, &activities, 4.0, 20, 10, 0);

        assert!(
            drift.drift_score < 0.01,
            "identical behavior should produce near-zero drift, got {}",
            drift.drift_score
        );
    }

    #[test]
    fn natural_growth_low_drift() {
        // Baseline: moderate activity.
        let mut dist = HashMap::new();
        dist.insert("content.create".into(), 0.5);
        dist.insert("social.message".into(), 0.5);
        let metrics = baseline_with_rates(5.0, 0.3, 2.5, 2.5, 0, dist);
        let baseline = make_baseline(metrics);

        // Current: slightly more active, slightly more governance — natural growth.
        let mut activities = make_activities(&["content.create", "social.message"], 12);
        activities.extend(make_activities(&["governance.vote"], 3));
        let drift = DriftComputer::compute(&baseline, &activities, 4.0, 20, 8, 0);

        assert!(
            drift.drift_score < 0.4,
            "natural growth should produce low-moderate drift, got {}",
            drift.drift_score
        );
        assert!(
            drift.drift_score > 0.0,
            "growth should produce some drift, got {}",
            drift.drift_score
        );
    }

    #[test]
    fn suspicious_shift_high_drift() {
        // Baseline: normal member, mostly content and social.
        let mut dist = HashMap::new();
        dist.insert("content.create".into(), 0.6);
        dist.insert("social.message".into(), 0.4);
        let metrics = baseline_with_rates(5.0, 0.1, 3.0, 2.0, 0, dist);
        let baseline = make_baseline(metrics);

        // Current: sudden shift to governance-heavy, role accumulation.
        let activities = make_activities(&["governance.vote", "governance.propose"], 20);
        let drift = DriftComputer::compute(&baseline, &activities, 4.0, 15, 14, 3);

        assert!(
            drift.drift_score > 0.5,
            "suspicious shift should produce high drift, got {}",
            drift.drift_score
        );
    }

    #[test]
    fn completely_different_behavior_max_drift() {
        // Baseline: all social, no governance, low activity.
        let mut dist = HashMap::new();
        dist.insert("social.message".into(), 1.0);
        let metrics = baseline_with_rates(2.0, 0.0, 0.0, 2.0, 0, dist);
        let baseline = make_baseline(metrics);

        // Current: all governance and content, no social, high activity,
        // many role changes.
        let activities = make_activities(&["governance.propose", "content.create"], 50);
        let drift = DriftComputer::compute(&baseline, &activities, 4.0, 10, 10, 5);

        assert!(
            drift.drift_score > 0.7,
            "completely different behavior should produce very high drift, got {}",
            drift.drift_score
        );
    }

    // -----------------------------------------------------------------------
    // Threshold and alerting tests
    // -----------------------------------------------------------------------

    #[test]
    fn exceeds_alert_threshold() {
        let mut dist = HashMap::new();
        dist.insert("social.message".into(), 1.0);
        let metrics = baseline_with_rates(2.0, 0.0, 0.0, 2.0, 0, dist);
        let baseline = make_baseline(metrics);

        let activities = make_activities(&["governance.propose", "content.create"], 50);
        let drift = DriftComputer::compute(&baseline, &activities, 4.0, 10, 10, 5);
        let config = DriftConfig::default();

        assert!(drift.exceeds_alert(&config));
    }

    #[test]
    fn normal_does_not_exceed_alert() {
        let mut dist = HashMap::new();
        dist.insert("content.create".into(), 0.5);
        dist.insert("social.message".into(), 0.5);
        let metrics = baseline_with_rates(5.0, 0.5, 2.5, 2.5, 0, dist);
        let baseline = make_baseline(metrics);

        let activities = make_activities(&["content.create", "social.message"], 10);
        let drift = DriftComputer::compute(&baseline, &activities, 4.0, 20, 10, 0);
        let config = DriftConfig::default();

        assert!(!drift.exceeds_alert(&config));
        assert!(!drift.exceeds_critical(&config));
    }

    #[test]
    fn evaluate_produces_normal_alert() {
        let mut dist = HashMap::new();
        dist.insert("content.create".into(), 0.5);
        dist.insert("social.message".into(), 0.5);
        let metrics = baseline_with_rates(5.0, 0.5, 2.5, 2.5, 0, dist);
        let baseline = make_baseline(metrics);

        let activities = make_activities(&["content.create", "social.message"], 10);
        let drift = DriftComputer::compute(&baseline, &activities, 4.0, 20, 10, 0);
        let config = DriftConfig::default();

        let alert = DriftComputer::evaluate(&drift, &config);
        assert_eq!(alert.level, DriftAlertLevel::Normal);
    }

    #[test]
    fn evaluate_produces_alert_level() {
        let mut dist = HashMap::new();
        dist.insert("social.message".into(), 1.0);
        let metrics = baseline_with_rates(2.0, 0.0, 0.0, 2.0, 0, dist);
        let baseline = make_baseline(metrics);

        // Moderate shift.
        let activities = make_activities(&["governance.vote", "content.create"], 15);
        let drift = DriftComputer::compute(&baseline, &activities, 4.0, 10, 7, 1);
        let config = DriftConfig::default();

        let alert = DriftComputer::evaluate(&drift, &config);
        // Verify it's at least Alert (could be Critical depending on magnitude).
        assert_ne!(alert.level, DriftAlertLevel::Normal);
    }

    #[test]
    fn evaluate_produces_critical_level() {
        let mut dist = HashMap::new();
        dist.insert("social.message".into(), 1.0);
        let metrics = baseline_with_rates(2.0, 0.0, 0.0, 2.0, 0, dist);
        let baseline = make_baseline(metrics);

        // Extreme shift.
        let activities = make_activities(&["governance.propose", "content.create"], 50);
        let drift = DriftComputer::compute(&baseline, &activities, 4.0, 10, 10, 5);

        let config = DriftConfig {
            alert_threshold: 0.5,
            critical_threshold: 0.7,
            ..DriftConfig::default()
        };

        let alert = DriftComputer::evaluate(&drift, &config);
        assert_eq!(alert.level, DriftAlertLevel::Critical);
    }

    #[test]
    fn alert_top_factors_sorted_by_deviation() {
        let mut dist = HashMap::new();
        dist.insert("social.message".into(), 1.0);
        let metrics = baseline_with_rates(2.0, 0.0, 0.0, 2.0, 0, dist);
        let baseline = make_baseline(metrics);

        let activities = make_activities(&["governance.propose", "content.create"], 30);
        let drift = DriftComputer::compute(&baseline, &activities, 4.0, 10, 10, 3);
        let config = DriftConfig::default();

        let alert = DriftComputer::evaluate(&drift, &config);
        for window in alert.top_factors.windows(2) {
            assert!(
                window[0].deviation >= window[1].deviation,
                "top_factors should be sorted descending by deviation"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Narrative scenario tests (Year 1-3 from spec)
    // -----------------------------------------------------------------------

    #[test]
    fn year_one_normal_member() {
        // Baseline: established during months 1-6.
        let mut dist = HashMap::new();
        dist.insert("content.create".into(), 0.4);
        dist.insert("social.message".into(), 0.5);
        dist.insert("governance.vote".into(), 0.1);
        let metrics = baseline_with_rates(8.0, 0.2, 3.2, 4.0, 0, dist);
        let baseline = make_baseline(metrics);

        // Year 1 check: nearly identical behavior.
        let mut activities = make_activities(&["content.create"], 13);
        activities.extend(make_activities(&["social.message"], 16));
        activities.extend(make_activities(&["governance.vote"], 3));

        let drift = DriftComputer::compute(&baseline, &activities, 4.0, 15, 3, 0);

        // Spec says ~0.1.
        assert!(
            drift.drift_score < 0.2,
            "Year 1 normal member should show minimal drift, got {}",
            drift.drift_score
        );
    }

    #[test]
    fn year_two_growing_engagement() {
        // Same baseline as year one.
        let mut dist = HashMap::new();
        dist.insert("content.create".into(), 0.4);
        dist.insert("social.message".into(), 0.5);
        dist.insert("governance.vote".into(), 0.1);
        let metrics = baseline_with_rates(8.0, 0.2, 3.2, 4.0, 0, dist);
        let baseline = make_baseline(metrics);

        // Year 2: moderately increased governance, slightly more activity.
        let mut activities = make_activities(&["content.create"], 14);
        activities.extend(make_activities(&["social.message"], 16));
        activities.extend(make_activities(&["governance.vote"], 8));

        let drift = DriftComputer::compute(&baseline, &activities, 4.0, 20, 10, 0);

        // Spec says ~0.3.
        assert!(
            drift.drift_score < 0.45,
            "Year 2 growing engagement should show moderate drift, got {}",
            drift.drift_score
        );
        assert!(
            drift.drift_score > 0.05,
            "Year 2 should show some drift, got {}",
            drift.drift_score
        );
    }

    #[test]
    fn year_three_sudden_shift() {
        // Same baseline.
        let mut dist = HashMap::new();
        dist.insert("content.create".into(), 0.4);
        dist.insert("social.message".into(), 0.5);
        dist.insert("governance.vote".into(), 0.1);
        let metrics = baseline_with_rates(8.0, 0.2, 3.2, 4.0, 0, dist);
        let baseline = make_baseline(metrics);

        // Year 3: high proposal rate, role accumulation, different content types.
        let mut activities = make_activities(&["governance.propose"], 20);
        activities.extend(make_activities(&["governance.vote"], 15));
        activities.extend(make_activities(&["content.create"], 5));

        let drift = DriftComputer::compute(&baseline, &activities, 4.0, 20, 18, 3);

        let config = DriftConfig::default();
        // Spec says drift ~0.7, should trigger alert.
        assert!(
            drift.drift_score >= config.alert_threshold,
            "Year 3 sudden shift should trigger alert (drift={}, threshold={})",
            drift.drift_score,
            config.alert_threshold,
        );
    }

    // -----------------------------------------------------------------------
    // Serialization tests
    // -----------------------------------------------------------------------

    #[test]
    fn drift_config_serialization_roundtrip() {
        let config = DriftConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: DriftConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn behavioral_drift_serialization_roundtrip() {
        let mut dist = HashMap::new();
        dist.insert("content.create".into(), 1.0);
        let metrics = baseline_with_rates(5.0, 0.5, 2.5, 0.0, 0, dist);
        let baseline = make_baseline(metrics);

        let activities = make_activities(&["content.create"], 10);
        let drift = DriftComputer::compute(&baseline, &activities, 4.0, 10, 5, 0);

        let json = serde_json::to_string(&drift).expect("serialize");
        let deserialized: BehavioralDrift = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(drift.pubkey, deserialized.pubkey);
        assert_eq!(drift.community_id, deserialized.community_id);
        assert!((drift.drift_score - deserialized.drift_score).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Edge case tests
    // -----------------------------------------------------------------------

    #[test]
    fn drift_factors_always_has_six_entries() {
        let metrics = BaselineMetrics::zero();
        let baseline = make_baseline(metrics);
        let drift = DriftComputer::compute(&baseline, &[], 4.0, 0, 0, 0);
        assert_eq!(drift.drift_factors.len(), 6);
    }

    #[test]
    fn drift_score_bounded_zero_to_one() {
        // Even with extreme values, score should stay in [0, 1].
        let mut dist = HashMap::new();
        dist.insert("a".into(), 1.0);
        let metrics = baseline_with_rates(100.0, 1.0, 50.0, 50.0, 10, dist);
        let baseline = make_baseline(metrics);

        let drift = DriftComputer::compute(&baseline, &[], 4.0, 0, 0, 0);
        assert!(drift.drift_score >= 0.0);
        assert!(drift.drift_score <= 1.0);
    }

    #[test]
    fn activity_new() {
        let now = Utc::now();
        let a = Activity::new("test.action", now);
        assert_eq!(a.action_type, "test.action");
        assert_eq!(a.timestamp, now);
    }

    #[test]
    fn drift_alert_level_derives() {
        // Verify DriftAlertLevel derives work.
        let level = DriftAlertLevel::Critical;
        let cloned = level;
        assert_eq!(level, cloned);

        let json = serde_json::to_string(&level).expect("serialize");
        let deserialized: DriftAlertLevel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(level, deserialized);
    }

    #[test]
    fn types_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BehavioralBaseline>();
        assert_send_sync::<BaselineMetrics>();
        assert_send_sync::<BehavioralDrift>();
        assert_send_sync::<DriftFactor>();
        assert_send_sync::<DriftConfig>();
        assert_send_sync::<DriftAlert>();
        assert_send_sync::<DriftAlertLevel>();
        assert_send_sync::<Activity>();
    }

    // ── Federation Scope ──────────────────────────────────────────────────

    fn make_drift_for_community(community_id: &str, drift_score: f64) -> BehavioralDrift {
        let metrics = BaselineMetrics::zero();
        BehavioralDrift {
            pubkey: "alice".into(),
            community_id: community_id.into(),
            baseline: BehavioralBaseline {
                pubkey: "alice".into(),
                community_id: community_id.into(),
                baseline_period_secs: 180 * 24 * 60 * 60,
                metrics: metrics.clone(),
                established_at: Utc::now() - Duration::days(365),
            },
            current: metrics,
            drift_score,
            drift_factors: vec![],
            computed_at: Utc::now(),
        }
    }

    #[test]
    fn filter_alerts_scoped_unrestricted() {
        let config = DriftConfig::default();
        let drifts = [
            make_drift_for_community("alpha", 0.7),
            make_drift_for_community("beta", 0.3),
        ];
        let alerts: Vec<DriftAlert> = drifts
            .iter()
            .map(|d| DriftComputer::evaluate(d, &config))
            .collect();

        let filtered = DriftComputer::filter_alerts_scoped(&alerts, &FederationScope::new());
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_alerts_scoped_filters_communities() {
        let config = DriftConfig::default();
        let drifts = [
            make_drift_for_community("alpha", 0.7),
            make_drift_for_community("beta", 0.3),
            make_drift_for_community("gamma", 0.9),
        ];
        let alerts: Vec<DriftAlert> = drifts
            .iter()
            .map(|d| DriftComputer::evaluate(d, &config))
            .collect();

        let scope = FederationScope::from_communities(["alpha", "gamma"]);
        let filtered = DriftComputer::filter_alerts_scoped(&alerts, &scope);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].drift.community_id, "alpha");
        assert_eq!(filtered[1].drift.community_id, "gamma");
    }

    #[test]
    fn max_drift_scoped_returns_max_in_scope() {
        let drifts = vec![
            make_drift_for_community("alpha", 0.3),
            make_drift_for_community("beta", 0.9),
            make_drift_for_community("gamma", 0.6),
        ];

        // Unrestricted: max is 0.9 (beta).
        let max = DriftComputer::max_drift_scoped(&drifts, &FederationScope::new());
        assert!((max.unwrap() - 0.9).abs() < f64::EPSILON);

        // Scoped to alpha + gamma: max is 0.6 (gamma).
        let scope = FederationScope::from_communities(["alpha", "gamma"]);
        let max = DriftComputer::max_drift_scoped(&drifts, &scope);
        assert!((max.unwrap() - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn max_drift_scoped_none_when_no_visible() {
        let drifts = vec![
            make_drift_for_community("alpha", 0.5),
        ];
        let scope = FederationScope::from_communities(["beta"]);
        assert!(DriftComputer::max_drift_scoped(&drifts, &scope).is_none());
    }

    #[test]
    fn max_drift_scoped_empty_drifts() {
        let drifts: Vec<BehavioralDrift> = vec![];
        assert!(DriftComputer::max_drift_scoped(&drifts, &FederationScope::new()).is_none());
    }
}
