//! # Financial Pattern Detection (R2F)
//!
//! Community-governed financial accountability. Not surveillance — pattern detection
//! that flags potential abuse for community governance review.
//!
//! From Consortium Art. 2 §5: "Consortia shall conduct their economic affairs with
//! full transparency and public accountability."
//!
//! ## Sovereignty Preserved
//!
//! Individuals can always transact peer-to-peer outside any community context.
//! No community = no policy = all private. Communities opt into financial transparency
//! by setting a policy in their charter. No global surveillance.
//!
//! ## Transaction Tiers
//!
//! - **Private** — below threshold. Buyer + seller only. No record visible to governance.
//! - **Receipted** — above private threshold. Receipt visible to governance: amount, parties,
//!   timestamp. NOT content/purpose.
//! - **Approved** — above receipted threshold. Requires multi-sig from governance before execution.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::FortuneError;

// ---------------------------------------------------------------------------
// Transaction Tiers
// ---------------------------------------------------------------------------

/// The transparency tier of a transaction, determined by community policy.
///
/// Communities choose their own thresholds. No community = no policy = all Private.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TransactionTier {
    /// Below community threshold. Buyer + seller only. No record visible to governance.
    Private,
    /// Above private threshold. Receipt visible to governance: amount, parties, timestamp.
    /// NOT content/purpose.
    Receipted,
    /// Above receipted threshold. Requires multi-signature from community governance
    /// before execution.
    Approved,
}

/// Scope of a tier policy — which transactions it applies to.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TierScope {
    /// Policy applies to all transactions within the community.
    #[default]
    AllTransactions,
    /// Policy applies only to transactions between members of the same community.
    CommunityTransactionsOnly,
    /// Policy applies only to transactions crossing community boundaries.
    InterCommunityOnly,
}

/// Community-configured transaction tier thresholds. Stored in Kingdom Charter.
///
/// Communities choose their own thresholds within Covenant bounds.
/// Default values provide a sensible starting point.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransactionTierPolicy {
    /// Below this amount, transactions are fully private (default: 500 Cool).
    pub private_ceiling: i64,
    /// Above private_ceiling and below this, transactions are receipted (default: 10_000 Cool).
    pub receipted_ceiling: i64,
    /// Above receipted_ceiling, transactions require multi-sig approval.
    /// This is always equal to receipted_ceiling (the floor for Approved is the ceiling
    /// of Receipted).
    pub approved_floor: i64,
    /// Maximum denomination for a single CashNote (default: 1_000 Cool).
    pub cash_note_max_denomination: i64,
    /// Which transactions this policy applies to.
    pub policy_applies_to: TierScope,
}

impl TransactionTierPolicy {
    /// Classify a transaction amount into a tier.
    ///
    /// # Examples
    ///
    /// ```
    /// use fortune::pattern_detection::{TransactionTierPolicy, TransactionTier};
    ///
    /// let policy = TransactionTierPolicy::default();
    /// assert_eq!(policy.classify(100), TransactionTier::Private);
    /// assert_eq!(policy.classify(500), TransactionTier::Private);
    /// assert_eq!(policy.classify(501), TransactionTier::Receipted);
    /// assert_eq!(policy.classify(10_001), TransactionTier::Approved);
    /// ```
    #[must_use]
    pub fn classify(&self, amount: i64) -> TransactionTier {
        if amount <= self.private_ceiling {
            TransactionTier::Private
        } else if amount <= self.receipted_ceiling {
            TransactionTier::Receipted
        } else {
            TransactionTier::Approved
        }
    }

    /// Check whether a CashNote denomination is within the cap.
    #[must_use]
    pub fn is_cash_note_within_cap(&self, denomination: i64) -> bool {
        denomination <= self.cash_note_max_denomination
    }
}

impl Default for TransactionTierPolicy {
    fn default() -> Self {
        Self {
            private_ceiling: 500,
            receipted_ceiling: 10_000,
            approved_floor: 10_000,
            cash_note_max_denomination: 1_000,
            policy_applies_to: TierScope::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Transaction Receipt
// ---------------------------------------------------------------------------

/// A receipt for a receipted or approved transaction.
///
/// Governance sees that a transaction happened — amount, parties, timestamp.
/// **No content field.** Governance never sees what a transaction was for.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransactionReceipt {
    pub id: Uuid,
    pub from_pubkey: String,
    pub to_pubkey: String,
    pub amount: i64,
    pub tier: TransactionTier,
    pub timestamp: DateTime<Utc>,
    pub community_id: String,
}

impl TransactionReceipt {
    /// Create a new receipt for a transaction classified by the given policy.
    #[must_use]
    pub fn new(
        from_pubkey: String,
        to_pubkey: String,
        amount: i64,
        community_id: String,
        policy: &TransactionTierPolicy,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from_pubkey,
            to_pubkey,
            amount,
            tier: policy.classify(amount),
            timestamp: Utc::now(),
            community_id,
        }
    }

    /// Create a receipt with an explicit tier and timestamp (useful for testing).
    #[must_use]
    pub fn with_details(
        from_pubkey: String,
        to_pubkey: String,
        amount: i64,
        tier: TransactionTier,
        community_id: String,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from_pubkey,
            to_pubkey,
            amount,
            tier,
            timestamp,
            community_id,
        }
    }
}

// ---------------------------------------------------------------------------
// Approval Workflow
// ---------------------------------------------------------------------------

/// Status of an approval request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ApprovalStatus {
    /// Waiting for signatures.
    Pending,
    /// Required approvals met.
    Approved,
    /// Approved and transferred.
    Executed,
    /// Majority rejected.
    Rejected,
    /// Deadline passed without enough signatures.
    Expired,
}

/// A single approver's signature on an approval request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApprovalSignature {
    pub approver_pubkey: String,
    pub approved: bool,
    pub signed_at: DateTime<Utc>,
}

/// A request for multi-sig approval on an Approved-tier transaction.
///
/// Created when a transaction exceeds the receipted ceiling. The transaction
/// blocks until the required number of governance approvers sign off.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub transaction: TransactionReceipt,
    pub requested_by: String,
    pub approvers: Vec<ApprovalSignature>,
    pub required_approvals: usize,
    pub status: ApprovalStatus,
    pub expires_at: DateTime<Utc>,
}

impl ApprovalRequest {
    /// Create a new pending approval request.
    #[must_use]
    pub fn new(
        transaction: TransactionReceipt,
        requested_by: String,
        required_approvals: usize,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            transaction,
            requested_by,
            approvers: Vec::new(),
            required_approvals,
            status: ApprovalStatus::Pending,
            expires_at,
        }
    }

    /// Add an approval or rejection signature. Returns error if the request is not pending
    /// or the approver has already signed.
    pub fn add_signature(
        &mut self,
        approver_pubkey: String,
        approved: bool,
    ) -> Result<(), FortuneError> {
        // Check request is still pending
        if self.status != ApprovalStatus::Pending {
            return Err(FortuneError::ApprovalAlreadyResolved(
                self.id.to_string(),
            ));
        }

        // Check not expired
        if Utc::now() >= self.expires_at {
            self.status = ApprovalStatus::Expired;
            return Err(FortuneError::ApprovalExpired(self.id.to_string()));
        }

        // Check for duplicate
        if self
            .approvers
            .iter()
            .any(|s| s.approver_pubkey == approver_pubkey)
        {
            return Err(FortuneError::DuplicateApprover(approver_pubkey));
        }

        self.approvers.push(ApprovalSignature {
            approver_pubkey,
            approved,
            signed_at: Utc::now(),
        });

        // Re-evaluate status
        self.evaluate();
        Ok(())
    }

    /// Re-evaluate approval status based on current signatures.
    fn evaluate(&mut self) {
        let approvals = self.approvers.iter().filter(|s| s.approved).count();
        let rejections = self.approvers.iter().filter(|s| !s.approved).count();

        if approvals >= self.required_approvals {
            self.status = ApprovalStatus::Approved;
        } else if rejections >= self.required_approvals {
            self.status = ApprovalStatus::Rejected;
        }
    }

    /// Mark an approved request as executed. Returns error if not in Approved status.
    pub fn mark_executed(&mut self) -> Result<(), FortuneError> {
        if self.status != ApprovalStatus::Approved {
            return Err(FortuneError::ApprovalAlreadyResolved(
                self.id.to_string(),
            ));
        }
        self.status = ApprovalStatus::Executed;
        Ok(())
    }

    /// Check whether this request has expired and update status if so.
    pub fn check_expiry(&mut self) -> bool {
        if self.status == ApprovalStatus::Pending && Utc::now() >= self.expires_at {
            self.status = ApprovalStatus::Expired;
            true
        } else {
            false
        }
    }

    /// Number of approval signatures received so far.
    #[must_use]
    pub fn approval_count(&self) -> usize {
        self.approvers.iter().filter(|s| s.approved).count()
    }

    /// Number of rejection signatures received so far.
    #[must_use]
    pub fn rejection_count(&self) -> usize {
        self.approvers.iter().filter(|s| !s.approved).count()
    }
}

// ---------------------------------------------------------------------------
// Financial Pattern Detection
// ---------------------------------------------------------------------------

/// Severity level for a financial alert.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AlertSeverity {
    /// Informational — no action needed, logged for governance awareness.
    Info,
    /// Warning — governance should review.
    Warning,
    /// Escalation — requires governance response.
    Escalation,
}

/// Automated response to a detected pattern.
///
/// No `BlockTransaction` — communities decide, not algorithms.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AutoAction {
    /// Notify community governance of the detected pattern.
    NotifyGovernance,
    /// Hold the transaction pending governance review (Approved-tier only).
    HoldTransaction,
    /// Require additional approval signatures beyond the normal threshold.
    RequireAdditionalApproval,
}

/// The type of financial pattern detected.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PatternType {
    /// Transactions systematically kept just below the receipted ceiling.
    Structuring,
    /// CashNotes created and redeemed in rapid succession.
    RapidCashCycling,
    /// Funds cycling A -> B -> C -> A with net transfer near zero.
    CircularFlow,
    /// Sudden spike in transaction volume (>3 std dev from rolling average).
    VolumeAnomaly,
}

/// An alert generated by the financial pattern detector.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FinancialAlert {
    pub id: Uuid,
    pub pattern: PatternType,
    pub pubkeys_involved: Vec<String>,
    pub community_id: String,
    pub severity: AlertSeverity,
    pub description: String,
    pub detected_at: DateTime<Utc>,
    pub auto_action: Option<AutoAction>,
}

/// Configuration for the pattern detector's sensitivity thresholds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetectorConfig {
    /// Number of transactions within the structuring window to trigger an alert.
    pub structuring_count_threshold: usize,
    /// Percentage of the receipted ceiling to consider "just below" (0.0-1.0).
    /// e.g., 0.10 means within 10% of the ceiling.
    pub structuring_proximity_pct: f64,
    /// Time window in seconds for structuring detection.
    pub structuring_window_secs: i64,
    /// Minimum seconds between CashMint and CashRedemption to NOT flag as rapid cycling.
    pub cash_cycling_min_gap_secs: i64,
    /// Standard deviations above rolling average for volume anomaly detection.
    pub volume_anomaly_std_devs: f64,
    /// Rolling window size (number of periods) for volume anomaly baseline.
    pub volume_rolling_window: usize,
    /// Period duration in seconds for volume anomaly calculation.
    pub volume_period_secs: i64,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            structuring_count_threshold: 5,
            structuring_proximity_pct: 0.10,
            structuring_window_secs: 86_400, // 24 hours
            cash_cycling_min_gap_secs: 3600,  // 1 hour
            volume_anomaly_std_devs: 3.0,
            volume_rolling_window: 30,
            volume_period_secs: 86_400, // 1 day
        }
    }
}

/// Detects suspicious financial patterns for community governance review.
///
/// This is not surveillance — it is pattern detection that flags potential abuse.
/// Communities opt into this by setting a `TransactionTierPolicy`.
/// Alerts go to governance; governance decides. No algorithm blocks transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialPatternDetector {
    config: DetectorConfig,
    alerts: Vec<FinancialAlert>,
}

impl FinancialPatternDetector {
    /// Create a new detector with the given configuration.
    #[must_use]
    pub fn new(config: DetectorConfig) -> Self {
        Self {
            config,
            alerts: Vec::new(),
        }
    }

    /// Create a detector with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(DetectorConfig::default())
    }

    /// Get all alerts generated so far.
    #[must_use]
    pub fn alerts(&self) -> &[FinancialAlert] {
        &self.alerts
    }

    /// Get the detector configuration.
    #[must_use]
    pub fn config(&self) -> &DetectorConfig {
        &self.config
    }

    /// Detect structuring: transactions systematically kept just below the receipted ceiling.
    ///
    /// Scans a list of receipts and flags pubkeys with too many transactions
    /// within a configured percentage of the ceiling during a sliding window.
    pub fn detect_structuring(
        &mut self,
        receipts: &[TransactionReceipt],
        policy: &TransactionTierPolicy,
        community_id: &str,
    ) -> Vec<FinancialAlert> {
        let ceiling = policy.receipted_ceiling;
        let floor = ceiling - (ceiling as f64 * self.config.structuring_proximity_pct) as i64;
        let window = chrono::Duration::seconds(self.config.structuring_window_secs);

        // Group receipts by sender pubkey
        let mut by_sender: HashMap<&str, Vec<&TransactionReceipt>> = HashMap::new();
        for receipt in receipts {
            if receipt.community_id == community_id {
                by_sender
                    .entry(&receipt.from_pubkey)
                    .or_default()
                    .push(receipt);
            }
        }

        let mut new_alerts = Vec::new();

        for (pubkey, mut txns) in by_sender {
            txns.sort_by_key(|t| t.timestamp);

            // Sliding window: for each transaction, count how many near-ceiling
            // transactions occur within the window ending at that transaction.
            for i in 0..txns.len() {
                let window_start = txns[i].timestamp - window;
                let near_ceiling: Vec<&&TransactionReceipt> = txns[..=i]
                    .iter()
                    .filter(|t| {
                        t.timestamp >= window_start
                            && t.amount >= floor
                            && t.amount <= ceiling
                    })
                    .collect();

                if near_ceiling.len() >= self.config.structuring_count_threshold {
                    let alert = FinancialAlert {
                        id: Uuid::new_v4(),
                        pattern: PatternType::Structuring,
                        pubkeys_involved: vec![pubkey.to_string()],
                        community_id: community_id.to_string(),
                        severity: AlertSeverity::Warning,
                        description: format!(
                            "{} transactions within {}% of receipted ceiling ({}) \
                             detected for pubkey in sliding window",
                            near_ceiling.len(),
                            (self.config.structuring_proximity_pct * 100.0) as i64,
                            ceiling,
                        ),
                        detected_at: Utc::now(),
                        auto_action: Some(AutoAction::NotifyGovernance),
                    };
                    new_alerts.push(alert);
                    break; // One alert per pubkey per scan
                }
            }
        }

        self.alerts.extend(new_alerts.clone());
        new_alerts
    }

    /// Detect rapid cash cycling: CashNotes created and redeemed in quick succession.
    ///
    /// Takes a list of (mint_time, redeem_time, issuer_pubkey, redeemer_pubkey) events.
    pub fn detect_rapid_cash_cycling(
        &mut self,
        events: &[(DateTime<Utc>, DateTime<Utc>, String, String)],
        community_id: &str,
    ) -> Vec<FinancialAlert> {
        let min_gap = chrono::Duration::seconds(self.config.cash_cycling_min_gap_secs);
        let mut new_alerts = Vec::new();

        for (mint_time, redeem_time, issuer, redeemer) in events {
            let gap = *redeem_time - *mint_time;
            if gap < min_gap && gap >= chrono::Duration::zero() {
                let alert = FinancialAlert {
                    id: Uuid::new_v4(),
                    pattern: PatternType::RapidCashCycling,
                    pubkeys_involved: vec![issuer.clone(), redeemer.clone()],
                    community_id: community_id.to_string(),
                    severity: AlertSeverity::Warning,
                    description: format!(
                        "CashNote minted and redeemed within {} seconds (minimum gap: {} seconds)",
                        gap.num_seconds(),
                        self.config.cash_cycling_min_gap_secs,
                    ),
                    detected_at: Utc::now(),
                    auto_action: Some(AutoAction::NotifyGovernance),
                };
                new_alerts.push(alert);
            }
        }

        self.alerts.extend(new_alerts.clone());
        new_alerts
    }

    /// Detect circular flow: funds cycling A -> B -> C -> A with net transfer near zero.
    ///
    /// Performs graph analysis on receipts to find cycles where the net transfer
    /// between each pair is within `tolerance` of zero.
    pub fn detect_circular_flow(
        &mut self,
        receipts: &[TransactionReceipt],
        community_id: &str,
        tolerance: i64,
    ) -> Vec<FinancialAlert> {
        // Build a net flow graph: for each (from, to) pair, sum the amounts
        let mut net_flow: HashMap<(&str, &str), i64> = HashMap::new();
        for receipt in receipts {
            if receipt.community_id != community_id {
                continue;
            }
            *net_flow
                .entry((&receipt.from_pubkey, &receipt.to_pubkey))
                .or_default() += receipt.amount;
        }

        // Look for cycles of length 2-4 where opposing flows nearly cancel out.
        // For simplicity, detect 3-node cycles (A->B->C->A): the most common pattern.
        let participants: Vec<&str> = {
            let mut set = std::collections::HashSet::new();
            for receipt in receipts {
                if receipt.community_id == community_id {
                    set.insert(receipt.from_pubkey.as_str());
                    set.insert(receipt.to_pubkey.as_str());
                }
            }
            set.into_iter().collect()
        };

        let mut new_alerts = Vec::new();
        let mut detected_cycles: std::collections::HashSet<Vec<String>> =
            std::collections::HashSet::new();

        for &a in &participants {
            for &b in &participants {
                if b == a {
                    continue;
                }
                for &c in &participants {
                    if c == a || c == b {
                        continue;
                    }
                    // Check A->B, B->C, C->A all exist
                    let ab = net_flow.get(&(a, b)).copied().unwrap_or(0);
                    let bc = net_flow.get(&(b, c)).copied().unwrap_or(0);
                    let ca = net_flow.get(&(c, a)).copied().unwrap_or(0);

                    if ab == 0 || bc == 0 || ca == 0 {
                        continue;
                    }

                    // Check if flows are roughly equal (circular)
                    let avg = (ab + bc + ca) / 3;
                    let all_close = (ab - avg).abs() <= tolerance
                        && (bc - avg).abs() <= tolerance
                        && (ca - avg).abs() <= tolerance;

                    if all_close {
                        // Normalize cycle to avoid duplicates (sort and use smallest first)
                        let mut cycle = vec![a.to_string(), b.to_string(), c.to_string()];
                        cycle.sort();
                        if detected_cycles.insert(cycle.clone()) {
                            let alert = FinancialAlert {
                                id: Uuid::new_v4(),
                                pattern: PatternType::CircularFlow,
                                pubkeys_involved: cycle,
                                community_id: community_id.to_string(),
                                severity: AlertSeverity::Escalation,
                                description: format!(
                                    "Circular flow detected: amounts {}, {}, {} \
                                     (tolerance: {})",
                                    ab, bc, ca, tolerance,
                                ),
                                detected_at: Utc::now(),
                                auto_action: Some(AutoAction::RequireAdditionalApproval),
                            };
                            new_alerts.push(alert);
                        }
                    }
                }
            }
        }

        self.alerts.extend(new_alerts.clone());
        new_alerts
    }

    /// Detect volume anomaly: sudden spike in transaction volume for a pubkey.
    ///
    /// Compares the current period's count to a rolling average of historical counts.
    /// Flags if current > rolling_avg + (std_devs * std_dev).
    ///
    /// `period_counts` is a map from pubkey to a vec of transaction counts per period
    /// (most recent period last).
    pub fn detect_volume_anomaly(
        &mut self,
        period_counts: &HashMap<String, Vec<u64>>,
        community_id: &str,
    ) -> Vec<FinancialAlert> {
        let mut new_alerts = Vec::new();

        for (pubkey, counts) in period_counts {
            if counts.len() < 2 {
                continue;
            }

            let window_size = self.config.volume_rolling_window.min(counts.len() - 1);
            let current = *counts.last().unwrap_or(&0) as f64;

            // Historical periods (everything except the current one)
            let historical = &counts[counts.len() - 1 - window_size..counts.len() - 1];
            if historical.is_empty() {
                continue;
            }

            let mean = historical.iter().sum::<u64>() as f64 / historical.len() as f64;
            let variance = historical
                .iter()
                .map(|&c| {
                    let diff = c as f64 - mean;
                    diff * diff
                })
                .sum::<f64>()
                / historical.len() as f64;
            let std_dev = variance.sqrt();

            let threshold = mean + self.config.volume_anomaly_std_devs * std_dev;

            if current > threshold && std_dev > 0.0 {
                let alert = FinancialAlert {
                    id: Uuid::new_v4(),
                    pattern: PatternType::VolumeAnomaly,
                    pubkeys_involved: vec![pubkey.clone()],
                    community_id: community_id.to_string(),
                    severity: AlertSeverity::Warning,
                    description: format!(
                        "Volume anomaly: current period {} exceeds threshold {:.1} \
                         (mean: {:.1}, std_dev: {:.1}, multiplier: {}x)",
                        current as u64,
                        threshold,
                        mean,
                        std_dev,
                        self.config.volume_anomaly_std_devs,
                    ),
                    detected_at: Utc::now(),
                    auto_action: Some(AutoAction::NotifyGovernance),
                };
                new_alerts.push(alert);
            }
        }

        self.alerts.extend(new_alerts.clone());
        new_alerts
    }

    /// Clear all stored alerts. Useful after governance has reviewed them.
    pub fn clear_alerts(&mut self) {
        self.alerts.clear();
    }

    /// Alerts for a specific community.
    #[must_use]
    pub fn alerts_for_community(&self, community_id: &str) -> Vec<&FinancialAlert> {
        self.alerts
            .iter()
            .filter(|a| a.community_id == community_id)
            .collect()
    }

    // -----------------------------------------------------------------------
    // Federation-scoped queries
    // -----------------------------------------------------------------------

    /// Alerts visible within a federation scope.
    ///
    /// When the scope is unrestricted, returns all alerts (same as `alerts()`).
    /// When scoped, returns only alerts from visible communities.
    ///
    /// From Constellation Art. 3 §3 — federation is a data boundary.
    #[must_use]
    pub fn alerts_for_federation(
        &self,
        scope: &crate::federation_scope::EconomicFederationScope,
    ) -> Vec<&FinancialAlert> {
        if scope.is_unrestricted() {
            self.alerts.iter().collect()
        } else {
            self.alerts
                .iter()
                .filter(|a| scope.is_visible(&a.community_id))
                .collect()
        }
    }

    /// Detect structuring across all communities visible within a federation scope.
    ///
    /// Runs structuring detection independently for each visible community found
    /// in the receipts. This respects community sovereignty — each community's
    /// receipts are analyzed against their own policy, not mixed together.
    pub fn detect_structuring_scoped(
        &mut self,
        receipts: &[TransactionReceipt],
        policy: &TransactionTierPolicy,
        scope: &crate::federation_scope::EconomicFederationScope,
    ) -> Vec<FinancialAlert> {
        // Collect unique community IDs from visible receipts.
        let community_ids: std::collections::HashSet<&str> = receipts
            .iter()
            .filter(|r| scope.is_visible(&r.community_id))
            .map(|r| r.community_id.as_str())
            .collect();

        let mut all_alerts = Vec::new();
        for community_id in community_ids {
            let alerts = self.detect_structuring(receipts, policy, community_id);
            all_alerts.extend(alerts);
        }
        all_alerts
    }

    /// Detect circular flow across all communities visible within a federation scope.
    ///
    /// Analyzes each visible community independently — cross-community flows
    /// are not mixed, preserving community sovereignty boundaries.
    pub fn detect_circular_flow_scoped(
        &mut self,
        receipts: &[TransactionReceipt],
        scope: &crate::federation_scope::EconomicFederationScope,
        tolerance: i64,
    ) -> Vec<FinancialAlert> {
        let community_ids: std::collections::HashSet<&str> = receipts
            .iter()
            .filter(|r| scope.is_visible(&r.community_id))
            .map(|r| r.community_id.as_str())
            .collect();

        let mut all_alerts = Vec::new();
        for community_id in community_ids {
            let alerts = self.detect_circular_flow(receipts, community_id, tolerance);
            all_alerts.extend(alerts);
        }
        all_alerts
    }

    /// Count alerts by community within a federation scope.
    ///
    /// Returns a map of community_id -> alert count, useful for federation-wide
    /// dashboards that show economic health across federated communities.
    #[must_use]
    pub fn alert_counts_by_community(
        &self,
        scope: &crate::federation_scope::EconomicFederationScope,
    ) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for alert in &self.alerts {
            if scope.is_visible(&alert.community_id) {
                *counts.entry(alert.community_id.clone()).or_default() += 1;
            }
        }
        counts
    }
}

impl Default for FinancialPatternDetector {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ---------------------------------------------------------------------------
// CashNote denomination cap enforcement helper
// ---------------------------------------------------------------------------

/// Enforce the CashNote denomination cap from a community's tier policy.
///
/// Returns `Ok(())` if the denomination is within the cap, or
/// `Err(FortuneError::CashNoteDenominationExceeded)` if it exceeds it.
pub fn enforce_cash_note_cap(
    denomination: i64,
    policy: &TransactionTierPolicy,
) -> Result<(), FortuneError> {
    if denomination > policy.cash_note_max_denomination {
        Err(FortuneError::CashNoteDenominationExceeded {
            amount: denomination,
            cap: policy.cash_note_max_denomination,
        })
    } else {
        Ok(())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Tier classification
    // -----------------------------------------------------------------------

    #[test]
    fn classify_private_below_ceiling() {
        let policy = TransactionTierPolicy::default();
        assert_eq!(policy.classify(0), TransactionTier::Private);
        assert_eq!(policy.classify(100), TransactionTier::Private);
        assert_eq!(policy.classify(499), TransactionTier::Private);
        assert_eq!(policy.classify(500), TransactionTier::Private);
    }

    #[test]
    fn classify_receipted_between_ceilings() {
        let policy = TransactionTierPolicy::default();
        assert_eq!(policy.classify(501), TransactionTier::Receipted);
        assert_eq!(policy.classify(5_000), TransactionTier::Receipted);
        assert_eq!(policy.classify(10_000), TransactionTier::Receipted);
    }

    #[test]
    fn classify_approved_above_receipted() {
        let policy = TransactionTierPolicy::default();
        assert_eq!(policy.classify(10_001), TransactionTier::Approved);
        assert_eq!(policy.classify(100_000), TransactionTier::Approved);
        assert_eq!(policy.classify(1_000_000), TransactionTier::Approved);
    }

    #[test]
    fn classify_with_custom_policy() {
        let policy = TransactionTierPolicy {
            private_ceiling: 100,
            receipted_ceiling: 1_000,
            approved_floor: 1_000,
            cash_note_max_denomination: 500,
            policy_applies_to: TierScope::CommunityTransactionsOnly,
        };
        assert_eq!(policy.classify(100), TransactionTier::Private);
        assert_eq!(policy.classify(101), TransactionTier::Receipted);
        assert_eq!(policy.classify(1_000), TransactionTier::Receipted);
        assert_eq!(policy.classify(1_001), TransactionTier::Approved);
    }

    #[test]
    fn classify_negative_amount_is_private() {
        let policy = TransactionTierPolicy::default();
        assert_eq!(policy.classify(-100), TransactionTier::Private);
    }

    // -----------------------------------------------------------------------
    // Receipt generation
    // -----------------------------------------------------------------------

    #[test]
    fn receipt_created_with_correct_tier() {
        let policy = TransactionTierPolicy::default();
        let receipt =
            TransactionReceipt::new("alice".into(), "bob".into(), 600, "community1".into(), &policy);
        assert_eq!(receipt.tier, TransactionTier::Receipted);
        assert_eq!(receipt.amount, 600);
        assert_eq!(receipt.from_pubkey, "alice");
        assert_eq!(receipt.to_pubkey, "bob");
        assert_eq!(receipt.community_id, "community1");
    }

    #[test]
    fn receipt_private_not_generated_for_low_amount() {
        let policy = TransactionTierPolicy::default();
        let receipt =
            TransactionReceipt::new("alice".into(), "bob".into(), 100, "community1".into(), &policy);
        assert_eq!(receipt.tier, TransactionTier::Private);
    }

    #[test]
    fn receipt_approved_for_high_amount() {
        let policy = TransactionTierPolicy::default();
        let receipt = TransactionReceipt::new(
            "alice".into(),
            "bob".into(),
            50_000,
            "community1".into(),
            &policy,
        );
        assert_eq!(receipt.tier, TransactionTier::Approved);
    }

    #[test]
    fn receipt_serialization_roundtrip() {
        let policy = TransactionTierPolicy::default();
        let receipt =
            TransactionReceipt::new("alice".into(), "bob".into(), 750, "comm1".into(), &policy);
        let json = serde_json::to_string(&receipt).unwrap();
        let restored: TransactionReceipt = serde_json::from_str(&json).unwrap();
        assert_eq!(receipt, restored);
    }

    #[test]
    fn receipt_has_no_content_field() {
        // Governance sees amount, parties, timestamp. NOT content/purpose.
        // Verify TransactionReceipt has no field named "content", "purpose", "memo", etc.
        // This is a structural test — if someone adds such a field, this test reminds them
        // that governance must not see transaction purpose.
        let policy = TransactionTierPolicy::default();
        let receipt =
            TransactionReceipt::new("alice".into(), "bob".into(), 600, "comm1".into(), &policy);
        let json = serde_json::to_string(&receipt).unwrap();
        assert!(!json.contains("\"content\""));
        assert!(!json.contains("\"purpose\""));
        assert!(!json.contains("\"memo\""));
    }

    // -----------------------------------------------------------------------
    // Approval workflow
    // -----------------------------------------------------------------------

    fn make_approval_request() -> ApprovalRequest {
        let receipt = TransactionReceipt::with_details(
            "alice".into(),
            "bob".into(),
            50_000,
            TransactionTier::Approved,
            "comm1".into(),
            Utc::now(),
        );
        ApprovalRequest::new(
            receipt,
            "alice".into(),
            2, // require 2 approvals
            Utc::now() + chrono::Duration::hours(24),
        )
    }

    #[test]
    fn approval_starts_pending() {
        let req = make_approval_request();
        assert_eq!(req.status, ApprovalStatus::Pending);
        assert_eq!(req.approval_count(), 0);
        assert_eq!(req.rejection_count(), 0);
    }

    #[test]
    fn approval_single_signature_stays_pending() {
        let mut req = make_approval_request();
        req.add_signature("governor1".into(), true).unwrap();
        assert_eq!(req.status, ApprovalStatus::Pending);
        assert_eq!(req.approval_count(), 1);
    }

    #[test]
    fn approval_reaches_threshold() {
        let mut req = make_approval_request();
        req.add_signature("governor1".into(), true).unwrap();
        req.add_signature("governor2".into(), true).unwrap();
        assert_eq!(req.status, ApprovalStatus::Approved);
        assert_eq!(req.approval_count(), 2);
    }

    #[test]
    fn approval_rejected_when_enough_rejections() {
        let mut req = make_approval_request();
        req.add_signature("governor1".into(), false).unwrap();
        req.add_signature("governor2".into(), false).unwrap();
        assert_eq!(req.status, ApprovalStatus::Rejected);
        assert_eq!(req.rejection_count(), 2);
    }

    #[test]
    fn approval_mixed_signatures() {
        let mut req = make_approval_request();
        req.add_signature("governor1".into(), true).unwrap();
        req.add_signature("governor2".into(), false).unwrap();
        // 1 approve, 1 reject — still pending (need 2 of either)
        assert_eq!(req.status, ApprovalStatus::Pending);
        req.add_signature("governor3".into(), true).unwrap();
        assert_eq!(req.status, ApprovalStatus::Approved);
    }

    #[test]
    fn approval_duplicate_signer_rejected() {
        let mut req = make_approval_request();
        req.add_signature("governor1".into(), true).unwrap();
        let result = req.add_signature("governor1".into(), true);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            FortuneError::DuplicateApprover(_)
        ));
    }

    #[test]
    fn approval_cannot_sign_after_resolved() {
        let mut req = make_approval_request();
        req.add_signature("governor1".into(), true).unwrap();
        req.add_signature("governor2".into(), true).unwrap();
        assert_eq!(req.status, ApprovalStatus::Approved);

        let result = req.add_signature("governor3".into(), true);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            FortuneError::ApprovalAlreadyResolved(_)
        ));
    }

    #[test]
    fn approval_mark_executed() {
        let mut req = make_approval_request();
        req.add_signature("governor1".into(), true).unwrap();
        req.add_signature("governor2".into(), true).unwrap();
        req.mark_executed().unwrap();
        assert_eq!(req.status, ApprovalStatus::Executed);
    }

    #[test]
    fn approval_cannot_execute_if_not_approved() {
        let mut req = make_approval_request();
        let result = req.mark_executed();
        assert!(result.is_err());
    }

    #[test]
    fn approval_expiry_check() {
        let receipt = TransactionReceipt::with_details(
            "alice".into(),
            "bob".into(),
            50_000,
            TransactionTier::Approved,
            "comm1".into(),
            Utc::now(),
        );
        let mut req = ApprovalRequest::new(
            receipt,
            "alice".into(),
            2,
            Utc::now() - chrono::Duration::hours(1), // already expired
        );
        assert!(req.check_expiry());
        assert_eq!(req.status, ApprovalStatus::Expired);
    }

    #[test]
    fn approval_serialization_roundtrip() {
        let req = make_approval_request();
        let json = serde_json::to_string(&req).unwrap();
        let restored: ApprovalRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, restored);
    }

    // -----------------------------------------------------------------------
    // Pattern detection — Structuring
    // -----------------------------------------------------------------------

    fn make_receipts_near_ceiling(
        pubkey: &str,
        count: usize,
        amount: i64,
        community_id: &str,
    ) -> Vec<TransactionReceipt> {
        (0..count)
            .map(|i| TransactionReceipt::with_details(
                pubkey.into(),
                format!("recipient_{i}"),
                amount,
                TransactionTier::Private, // tier doesn't matter for structuring detection
                community_id.into(),
                Utc::now() - chrono::Duration::hours(i as i64),
            ))
            .collect()
    }

    #[test]
    fn structuring_detected_true_positive() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let policy = TransactionTierPolicy::default(); // receipted_ceiling = 10_000

        // 5 transactions at 9_500 (within 10% of 10_000) — structuring
        let receipts = make_receipts_near_ceiling("alice", 5, 9_500, "comm1");
        let alerts = detector.detect_structuring(&receipts, &policy, "comm1");

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].pattern, PatternType::Structuring);
        assert!(alerts[0].pubkeys_involved.contains(&"alice".to_string()));
    }

    #[test]
    fn structuring_not_triggered_below_threshold() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let policy = TransactionTierPolicy::default();

        // Only 3 transactions near ceiling — below the count threshold of 5
        let receipts = make_receipts_near_ceiling("alice", 3, 9_500, "comm1");
        let alerts = detector.detect_structuring(&receipts, &policy, "comm1");

        assert!(alerts.is_empty());
    }

    #[test]
    fn structuring_not_triggered_for_low_amounts() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let policy = TransactionTierPolicy::default();

        // 10 transactions at 1_000 — far below the ceiling, not structuring
        let receipts = make_receipts_near_ceiling("alice", 10, 1_000, "comm1");
        let alerts = detector.detect_structuring(&receipts, &policy, "comm1");

        assert!(alerts.is_empty());
    }

    #[test]
    fn structuring_ignores_other_communities() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let policy = TransactionTierPolicy::default();

        let receipts = make_receipts_near_ceiling("alice", 5, 9_500, "other_comm");
        let alerts = detector.detect_structuring(&receipts, &policy, "comm1");

        assert!(alerts.is_empty());
    }

    // -----------------------------------------------------------------------
    // Pattern detection — Rapid Cash Cycling
    // -----------------------------------------------------------------------

    #[test]
    fn rapid_cash_cycling_detected() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let now = Utc::now();
        let events = vec![(
            now,
            now + chrono::Duration::minutes(5), // redeemed 5 min later (< 1hr gap)
            "alice".into(),
            "bob".into(),
        )];

        let alerts = detector.detect_rapid_cash_cycling(&events, "comm1");
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].pattern, PatternType::RapidCashCycling);
    }

    #[test]
    fn cash_cycling_not_triggered_for_normal_gap() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let now = Utc::now();
        let events = vec![(
            now,
            now + chrono::Duration::hours(2), // redeemed 2 hours later (> 1hr gap)
            "alice".into(),
            "bob".into(),
        )];

        let alerts = detector.detect_rapid_cash_cycling(&events, "comm1");
        assert!(alerts.is_empty());
    }

    #[test]
    fn cash_cycling_multiple_events() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let now = Utc::now();
        let events = vec![
            (
                now,
                now + chrono::Duration::minutes(5),
                "alice".into(),
                "bob".into(),
            ),
            (
                now,
                now + chrono::Duration::minutes(10),
                "carol".into(),
                "dave".into(),
            ),
            (
                now,
                now + chrono::Duration::hours(3), // not flagged
                "eve".into(),
                "frank".into(),
            ),
        ];

        let alerts = detector.detect_rapid_cash_cycling(&events, "comm1");
        assert_eq!(alerts.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Pattern detection — Circular Flow
    // -----------------------------------------------------------------------

    #[test]
    fn circular_flow_detected() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let now = Utc::now();

        // A->B: 1000, B->C: 1000, C->A: 1000 — perfect circle
        let receipts = vec![
            TransactionReceipt::with_details(
                "alice".into(), "bob".into(), 1000, TransactionTier::Receipted,
                "comm1".into(), now,
            ),
            TransactionReceipt::with_details(
                "bob".into(), "carol".into(), 1000, TransactionTier::Receipted,
                "comm1".into(), now + chrono::Duration::minutes(10),
            ),
            TransactionReceipt::with_details(
                "carol".into(), "alice".into(), 1000, TransactionTier::Receipted,
                "comm1".into(), now + chrono::Duration::minutes(20),
            ),
        ];

        let alerts = detector.detect_circular_flow(&receipts, "comm1", 100);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].pattern, PatternType::CircularFlow);
        assert_eq!(alerts[0].severity, AlertSeverity::Escalation);
    }

    #[test]
    fn circular_flow_with_tolerance() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let now = Utc::now();

        // Amounts slightly different but within tolerance
        let receipts = vec![
            TransactionReceipt::with_details(
                "alice".into(), "bob".into(), 1000, TransactionTier::Receipted,
                "comm1".into(), now,
            ),
            TransactionReceipt::with_details(
                "bob".into(), "carol".into(), 1050, TransactionTier::Receipted,
                "comm1".into(), now,
            ),
            TransactionReceipt::with_details(
                "carol".into(), "alice".into(), 980, TransactionTier::Receipted,
                "comm1".into(), now,
            ),
        ];

        let alerts = detector.detect_circular_flow(&receipts, "comm1", 100);
        assert_eq!(alerts.len(), 1);
    }

    #[test]
    fn no_circular_flow_for_one_way_transfers() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let now = Utc::now();

        // A->B and B->C but no C->A — not a cycle
        let receipts = vec![
            TransactionReceipt::with_details(
                "alice".into(), "bob".into(), 1000, TransactionTier::Receipted,
                "comm1".into(), now,
            ),
            TransactionReceipt::with_details(
                "bob".into(), "carol".into(), 1000, TransactionTier::Receipted,
                "comm1".into(), now,
            ),
        ];

        let alerts = detector.detect_circular_flow(&receipts, "comm1", 100);
        assert!(alerts.is_empty());
    }

    // -----------------------------------------------------------------------
    // Pattern detection — Volume Anomaly
    // -----------------------------------------------------------------------

    #[test]
    fn volume_anomaly_detected() {
        let mut detector = FinancialPatternDetector::with_defaults();

        // Steady baseline of ~10 txns/period, then sudden spike to 100
        let mut counts = HashMap::new();
        counts.insert(
            "alice".to_string(),
            vec![10, 11, 9, 10, 12, 10, 9, 11, 10, 10, 100],
        );

        let alerts = detector.detect_volume_anomaly(&counts, "comm1");
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].pattern, PatternType::VolumeAnomaly);
    }

    #[test]
    fn volume_anomaly_not_triggered_for_gradual_increase() {
        let mut detector = FinancialPatternDetector::with_defaults();

        // Gradual increase — current period is within normal variance
        let mut counts = HashMap::new();
        counts.insert(
            "alice".to_string(),
            vec![10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30],
        );

        let alerts = detector.detect_volume_anomaly(&counts, "comm1");
        // Should not trigger because the stddev is high enough to accommodate the trend
        // (variance is large relative to the increase)
        assert!(alerts.is_empty());
    }

    #[test]
    fn volume_anomaly_needs_minimum_history() {
        let mut detector = FinancialPatternDetector::with_defaults();

        // Only one period — not enough data
        let mut counts = HashMap::new();
        counts.insert("alice".to_string(), vec![100]);

        let alerts = detector.detect_volume_anomaly(&counts, "comm1");
        assert!(alerts.is_empty());
    }

    // -----------------------------------------------------------------------
    // CashNote denomination cap
    // -----------------------------------------------------------------------

    #[test]
    fn cash_note_within_cap() {
        let policy = TransactionTierPolicy::default(); // cap = 1_000
        assert!(enforce_cash_note_cap(500, &policy).is_ok());
        assert!(enforce_cash_note_cap(1_000, &policy).is_ok());
    }

    #[test]
    fn cash_note_exceeds_cap() {
        let policy = TransactionTierPolicy::default();
        let result = enforce_cash_note_cap(1_001, &policy);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            FortuneError::CashNoteDenominationExceeded { amount: 1_001, cap: 1_000 }
        ));
    }

    #[test]
    fn cash_note_cap_with_custom_policy() {
        let policy = TransactionTierPolicy {
            cash_note_max_denomination: 5_000,
            ..Default::default()
        };
        assert!(enforce_cash_note_cap(4_999, &policy).is_ok());
        assert!(enforce_cash_note_cap(5_000, &policy).is_ok());
        assert!(enforce_cash_note_cap(5_001, &policy).is_err());
    }

    // -----------------------------------------------------------------------
    // Sovereignty preservation
    // -----------------------------------------------------------------------

    #[test]
    fn no_community_means_all_private() {
        // Without a community policy, everything is private.
        // Individuals transacting peer-to-peer outside any community context
        // have no tier policy applied — all transactions are sovereign.
        let policy = TransactionTierPolicy::default();

        // The point: tier classification requires a policy. No policy = no classification.
        // The caller decides whether to apply a policy. If no community, don't call classify.
        // This test verifies the design intent.
        assert_eq!(policy.classify(0), TransactionTier::Private);
        assert_eq!(policy.classify(500), TransactionTier::Private);
        // But if someone DOES have a policy, it applies:
        assert_ne!(policy.classify(501), TransactionTier::Private);
    }

    #[test]
    fn sovereignty_no_global_surveillance() {
        // Pattern detection only works within a community context.
        // Without a community_id, nothing is flagged.
        let mut detector = FinancialPatternDetector::with_defaults();
        let policy = TransactionTierPolicy::default();

        // Create receipts for "no_community" context
        let receipts = make_receipts_near_ceiling("alice", 10, 9_500, "comm1");

        // Scan for a different community — should find nothing
        let alerts = detector.detect_structuring(&receipts, &policy, "other_community");
        assert!(alerts.is_empty());
    }

    #[test]
    fn community_opt_in_scope() {
        // Communities can restrict their policy scope.
        let policy = TransactionTierPolicy {
            policy_applies_to: TierScope::InterCommunityOnly,
            ..Default::default()
        };
        assert_eq!(policy.policy_applies_to, TierScope::InterCommunityOnly);

        let policy = TransactionTierPolicy {
            policy_applies_to: TierScope::CommunityTransactionsOnly,
            ..Default::default()
        };
        assert_eq!(
            policy.policy_applies_to,
            TierScope::CommunityTransactionsOnly
        );
    }

    // -----------------------------------------------------------------------
    // Serialization & type properties
    // -----------------------------------------------------------------------

    #[test]
    fn tier_policy_serialization_roundtrip() {
        let policy = TransactionTierPolicy::default();
        let json = serde_json::to_string(&policy).unwrap();
        let restored: TransactionTierPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, restored);
    }

    #[test]
    fn alert_severity_ordering() {
        // Verify all three severities exist and are distinct
        assert_ne!(AlertSeverity::Info, AlertSeverity::Warning);
        assert_ne!(AlertSeverity::Warning, AlertSeverity::Escalation);
        assert_ne!(AlertSeverity::Info, AlertSeverity::Escalation);
    }

    #[test]
    fn auto_action_no_block_transaction() {
        // There is no BlockTransaction variant. Communities decide, not algorithms.
        let actions = [
            AutoAction::NotifyGovernance,
            AutoAction::HoldTransaction,
            AutoAction::RequireAdditionalApproval,
        ];
        for action in &actions {
            let json = serde_json::to_string(action).unwrap();
            assert!(!json.contains("Block"));
        }
    }

    #[test]
    fn financial_alert_serialization_roundtrip() {
        let alert = FinancialAlert {
            id: Uuid::new_v4(),
            pattern: PatternType::Structuring,
            pubkeys_involved: vec!["alice".into()],
            community_id: "comm1".into(),
            severity: AlertSeverity::Warning,
            description: "Test alert".into(),
            detected_at: Utc::now(),
            auto_action: Some(AutoAction::NotifyGovernance),
        };
        let json = serde_json::to_string(&alert).unwrap();
        let restored: FinancialAlert = serde_json::from_str(&json).unwrap();
        assert_eq!(alert, restored);
    }

    #[test]
    fn detector_config_defaults_are_sensible() {
        let config = DetectorConfig::default();
        assert_eq!(config.structuring_count_threshold, 5);
        assert!((config.structuring_proximity_pct - 0.10).abs() < f64::EPSILON);
        assert_eq!(config.structuring_window_secs, 86_400);
        assert_eq!(config.cash_cycling_min_gap_secs, 3600);
        assert!((config.volume_anomaly_std_devs - 3.0).abs() < f64::EPSILON);
        assert_eq!(config.volume_rolling_window, 30);
    }

    #[test]
    fn detector_accumulates_alerts() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let policy = TransactionTierPolicy::default();

        let receipts = make_receipts_near_ceiling("alice", 5, 9_500, "comm1");
        detector.detect_structuring(&receipts, &policy, "comm1");

        let now = Utc::now();
        let events = vec![(
            now,
            now + chrono::Duration::minutes(5),
            "bob".into(),
            "carol".into(),
        )];
        detector.detect_rapid_cash_cycling(&events, "comm1");

        assert_eq!(detector.alerts().len(), 2);

        let comm1_alerts = detector.alerts_for_community("comm1");
        assert_eq!(comm1_alerts.len(), 2);

        let comm2_alerts = detector.alerts_for_community("comm2");
        assert!(comm2_alerts.is_empty());
    }

    #[test]
    fn detector_clear_alerts() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let policy = TransactionTierPolicy::default();

        let receipts = make_receipts_near_ceiling("alice", 5, 9_500, "comm1");
        detector.detect_structuring(&receipts, &policy, "comm1");
        assert!(!detector.alerts().is_empty());

        detector.clear_alerts();
        assert!(detector.alerts().is_empty());
    }

    // -----------------------------------------------------------------------
    // Federation-scoped queries
    // -----------------------------------------------------------------------

    #[test]
    fn alerts_for_federation_unrestricted_returns_all() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let policy = TransactionTierPolicy::default();

        // Create alerts in two communities
        let receipts1 = make_receipts_near_ceiling("alice", 5, 9_500, "comm1");
        let receipts2 = make_receipts_near_ceiling("bob", 5, 9_500, "comm2");
        detector.detect_structuring(&receipts1, &policy, "comm1");
        detector.detect_structuring(&receipts2, &policy, "comm2");
        assert_eq!(detector.alerts().len(), 2);

        // Unrestricted scope sees all
        let scope = crate::federation_scope::EconomicFederationScope::new();
        let federated = detector.alerts_for_federation(&scope);
        assert_eq!(federated.len(), 2);
    }

    #[test]
    fn alerts_for_federation_scoped_filters_communities() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let policy = TransactionTierPolicy::default();

        let receipts1 = make_receipts_near_ceiling("alice", 5, 9_500, "comm1");
        let receipts2 = make_receipts_near_ceiling("bob", 5, 9_500, "comm2");
        let receipts3 = make_receipts_near_ceiling("carol", 5, 9_500, "comm3");
        detector.detect_structuring(&receipts1, &policy, "comm1");
        detector.detect_structuring(&receipts2, &policy, "comm2");
        detector.detect_structuring(&receipts3, &policy, "comm3");
        assert_eq!(detector.alerts().len(), 3);

        // Only comm1 and comm3 visible
        let scope = crate::federation_scope::EconomicFederationScope::from_communities(
            ["comm1", "comm3"],
        );
        let federated = detector.alerts_for_federation(&scope);
        assert_eq!(federated.len(), 2);
        assert!(federated.iter().all(|a| a.community_id == "comm1" || a.community_id == "comm3"));
    }

    #[test]
    fn detect_structuring_scoped_across_federation() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let policy = TransactionTierPolicy::default();

        // Receipts from three communities
        let mut receipts = make_receipts_near_ceiling("alice", 5, 9_500, "comm1");
        receipts.extend(make_receipts_near_ceiling("bob", 5, 9_500, "comm2"));
        receipts.extend(make_receipts_near_ceiling("carol", 5, 9_500, "comm3"));

        // Scope to comm1 + comm2 only
        let scope = crate::federation_scope::EconomicFederationScope::from_communities(
            ["comm1", "comm2"],
        );
        let alerts = detector.detect_structuring_scoped(&receipts, &policy, &scope);

        // Should find structuring in comm1 and comm2, not comm3
        assert_eq!(alerts.len(), 2);
        let community_ids: std::collections::HashSet<&str> =
            alerts.iter().map(|a| a.community_id.as_str()).collect();
        assert!(community_ids.contains("comm1"));
        assert!(community_ids.contains("comm2"));
        assert!(!community_ids.contains("comm3"));
    }

    #[test]
    fn alert_counts_by_community_scoped() {
        let mut detector = FinancialPatternDetector::with_defaults();
        let policy = TransactionTierPolicy::default();

        // Two alerts in comm1, one in comm2, one in comm3
        let receipts1 = make_receipts_near_ceiling("alice", 5, 9_500, "comm1");
        let receipts1b = make_receipts_near_ceiling("dave", 5, 9_500, "comm1");
        let receipts2 = make_receipts_near_ceiling("bob", 5, 9_500, "comm2");
        let receipts3 = make_receipts_near_ceiling("carol", 5, 9_500, "comm3");
        detector.detect_structuring(&receipts1, &policy, "comm1");
        detector.detect_structuring(&receipts1b, &policy, "comm1");
        detector.detect_structuring(&receipts2, &policy, "comm2");
        detector.detect_structuring(&receipts3, &policy, "comm3");

        // Scope to comm1 + comm2
        let scope = crate::federation_scope::EconomicFederationScope::from_communities(
            ["comm1", "comm2"],
        );
        let counts = detector.alert_counts_by_community(&scope);
        assert_eq!(counts.get("comm1"), Some(&2));
        assert_eq!(counts.get("comm2"), Some(&1));
        assert_eq!(counts.get("comm3"), None);
    }

    #[test]
    fn types_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TransactionTier>();
        assert_send_sync::<TransactionTierPolicy>();
        assert_send_sync::<TransactionReceipt>();
        assert_send_sync::<ApprovalRequest>();
        assert_send_sync::<ApprovalSignature>();
        assert_send_sync::<ApprovalStatus>();
        assert_send_sync::<FinancialPatternDetector>();
        assert_send_sync::<FinancialAlert>();
        assert_send_sync::<AlertSeverity>();
        assert_send_sync::<AutoAction>();
        assert_send_sync::<PatternType>();
        assert_send_sync::<DetectorConfig>();
        assert_send_sync::<TierScope>();
    }
}
