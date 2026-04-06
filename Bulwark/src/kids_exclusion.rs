//! # KidsSphere: Collective Parental Approval + Immutable Exclusion (R2B)
//!
//! Children are priority #1. Two architectural defenses:
//!
//! 1. **Collective Parental Approval (entry)** — no person interacts with children
//!    in a KidsSphere community unless multiple parents in that community have
//!    COLLECTIVELY met them in person and approved them. Not one parent — multiple.
//!    Not remotely — physically.
//!
//! 2. **Immutable Exclusion (removal)** — Covenant-level protection. Children's
//!    Dignity overrides an adult's community access.
//!
//! ## Access Check Order
//!
//! 1. Is this pubkey on the `KidsSphereExclusionRegistry`? If yes -> DENIED. Full stop.
//! 2. Does this pubkey have valid `KidsSphereApproval` for THIS community? If no -> DENIED.
//! 3. Is the approval expired? If yes -> DENIED until renewed.
//! 4. Has any parent revoked? If yes -> SUSPENDED pending review.
//! 5. All checks pass -> ALLOWED.
//!
//! Both checks are mandatory for all KidsSphere communities -- neither can be
//! disabled by charter.
//!
//! ## Identity Rebirth Defense
//!
//! Even if a predator creates a new Crown identity, they face TWO barriers:
//! 1. They need to enter the Founding Verification Tree (R2E) -- new identity has
//!    a fresh lineage, detectable by graph analysis.
//! 2. They need multiple parents in the specific community to physically meet them
//!    and approve -- a face-to-face gauntlet.
//!
//! ## Covenant Alignment
//!
//! **Dignity** — children's Dignity is absolute; it overrides adult community access.
//! **Sovereignty** — parents retain sovereign authority over who interacts with
//! their children, including single-parent revocation power.
//! **Consent** — collective physical meeting is the ultimate informed consent gate.

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::BulwarkError;
use crate::federation_scope::FederationScope;
use crate::verification::proximity::ProximityProof;

// ---------------------------------------------------------------------------
// Approval Confidence
// ---------------------------------------------------------------------------

/// How confident a parent feels about approving KidsSphere access for a candidate.
///
/// All three levels count as approval, but `Reluctant` triggers a community-wide
/// notification so other parents are aware of the concern.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ApprovalConfidence {
    /// Parent is comfortable with this person having KidsSphere access.
    Comfortable,
    /// Parent approves but has some reservations.
    Cautious,
    /// Parent approves but is reluctant. Triggers community-wide notification.
    Reluctant,
}

impl ApprovalConfidence {
    /// Whether this confidence level triggers a community-wide notification.
    pub fn triggers_notification(&self) -> bool {
        matches!(self, Self::Reluctant)
    }
}

impl std::fmt::Display for ApprovalConfidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Comfortable => write!(f, "comfortable"),
            Self::Cautious => write!(f, "cautious"),
            Self::Reluctant => write!(f, "reluctant"),
        }
    }
}

// ---------------------------------------------------------------------------
// Parental Approval (individual)
// ---------------------------------------------------------------------------

/// A single parent's approval for a candidate to access KidsSphere areas.
///
/// Each parent individually approves after the collective physical meeting.
/// The `child_pubkeys` field specifies WHICH of their children this approval
/// covers — a parent might approve access for their older child but not their
/// younger one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParentalApproval {
    /// The approving parent's public key.
    pub parent_pubkey: String,
    /// Which children this parent is approving access for.
    pub child_pubkeys: Vec<String>,
    /// How confident the parent feels about this approval.
    pub confidence: ApprovalConfidence,
    /// When this individual approval was signed.
    pub signed_at: DateTime<Utc>,
}

impl ParentalApproval {
    /// Create a new parental approval.
    pub fn new(
        parent_pubkey: impl Into<String>,
        child_pubkeys: Vec<String>,
        confidence: ApprovalConfidence,
    ) -> Self {
        Self {
            parent_pubkey: parent_pubkey.into(),
            child_pubkeys,
            confidence,
            signed_at: Utc::now(),
        }
    }

    /// Whether any parent in this approval voted `Reluctant`.
    pub fn is_reluctant(&self) -> bool {
        self.confidence.triggers_notification()
    }
}

// ---------------------------------------------------------------------------
// KidsSphere Approval (collective)
// ---------------------------------------------------------------------------

/// A collective approval allowing a candidate to access KidsSphere areas
/// within a specific community.
///
/// Requires multiple parents to physically meet the candidate at a community
/// gathering, prove proximity, and individually approve. Community-specific —
/// approval in one community does not transfer to another. Annual renewal required.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KidsSphereApproval {
    /// Unique approval identifier.
    pub id: Uuid,
    /// The candidate being approved for KidsSphere access.
    pub candidate_pubkey: String,
    /// Which community this approval is for (does NOT transfer).
    pub community_id: String,
    /// Individual parent approvals collected at the meeting.
    pub approvals: Vec<ParentalApproval>,
    /// Proves all parties were physically present together.
    pub collective_meeting: ProximityProof,
    /// When the collective approval was established.
    pub approved_at: DateTime<Utc>,
    /// When this approval expires (annual renewal required).
    pub expires_at: DateTime<Utc>,
    /// When revoked (if any parent revokes). One revocation = immediate suspension.
    pub revoked_at: Option<DateTime<Utc>>,
}

impl KidsSphereApproval {
    /// Create a new KidsSphere approval after a collective physical meeting.
    ///
    /// Validates:
    /// - Proximity proof has valid evidence
    /// - Minimum number of approvals met (from policy)
    pub fn new(
        candidate_pubkey: impl Into<String>,
        community_id: impl Into<String>,
        approvals: Vec<ParentalApproval>,
        collective_meeting: ProximityProof,
        policy: &KidsSphereApprovalPolicy,
    ) -> Result<Self, BulwarkError> {
        if policy.proximity_required && !collective_meeting.has_proximity_evidence() {
            return Err(BulwarkError::ProximityRequired(
                "KidsSphere collective meeting requires physical proximity proof".into(),
            ));
        }

        if approvals.len() < policy.min_approvals {
            return Err(BulwarkError::InsufficientApprovals {
                have: approvals.len(),
                need: policy.min_approvals,
            });
        }

        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            candidate_pubkey: candidate_pubkey.into(),
            community_id: community_id.into(),
            approvals,
            collective_meeting,
            approved_at: now,
            expires_at: now + Duration::days(policy.renewal_interval_days as i64),
            revoked_at: None,
        })
    }

    /// Whether the approval has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Whether any parent has revoked.
    pub fn is_revoked(&self) -> bool {
        self.revoked_at.is_some()
    }

    /// Whether the approval is currently valid (not expired and not revoked).
    pub fn is_valid(&self) -> bool {
        !self.is_expired() && !self.is_revoked()
    }

    /// Revoke this approval. One parent revoking = immediate suspension.
    pub fn revoke(&mut self) {
        if self.revoked_at.is_none() {
            self.revoked_at = Some(Utc::now());
        }
    }

    /// Whether any parent voted `Reluctant` (triggers community notification).
    pub fn has_reluctant_approval(&self) -> bool {
        self.approvals.iter().any(|a| a.is_reluctant())
    }

    /// How many distinct parents approved.
    pub fn approval_count(&self) -> usize {
        self.approvals.len()
    }
}

// ---------------------------------------------------------------------------
// KidsSphere Approval Policy
// ---------------------------------------------------------------------------

/// Policy for KidsSphere collective parental approval, stored in community Charter.
///
/// Communities can tune the parameters but cannot disable the requirement entirely.
/// Both the exclusion check and the approval check are mandatory — neither can be
/// turned off.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KidsSphereApprovalPolicy {
    /// Minimum number of parents required to approve (default 3).
    /// Cannot be set below 2 — one parent is never enough for KidsSphere access.
    pub min_approvals: usize,
    /// Days between required renewals (default 365 — annual).
    /// Cannot be set above 730 — biennial is the maximum interval.
    pub renewal_interval_days: u64,
    /// Whether one parent revoking triggers a community governance review (default true).
    /// Cannot be set to false — this is a circuit breaker, not a preference.
    pub single_revocation_triggers_review: bool,
    /// Whether physical proximity proof is required at the collective meeting (default true).
    /// Cannot be set to false — remote approval defeats the purpose.
    pub proximity_required: bool,
}

impl Default for KidsSphereApprovalPolicy {
    fn default() -> Self {
        Self {
            min_approvals: DEFAULT_MIN_APPROVALS,
            renewal_interval_days: DEFAULT_RENEWAL_INTERVAL_DAYS,
            single_revocation_triggers_review: true,
            proximity_required: true,
        }
    }
}

impl KidsSphereApprovalPolicy {
    /// Validate that this policy respects Covenant constraints.
    ///
    /// Returns an error if any parameter has been set to an unsafe value.
    pub fn validate(&self) -> Result<(), BulwarkError> {
        if self.min_approvals < MINIMUM_MIN_APPROVALS {
            return Err(BulwarkError::ConfigError(format!(
                "KidsSphere min_approvals must be at least {} (got {})",
                MINIMUM_MIN_APPROVALS, self.min_approvals
            )));
        }
        if self.renewal_interval_days > MAXIMUM_RENEWAL_INTERVAL_DAYS {
            return Err(BulwarkError::ConfigError(format!(
                "KidsSphere renewal_interval_days cannot exceed {} (got {})",
                MAXIMUM_RENEWAL_INTERVAL_DAYS, self.renewal_interval_days
            )));
        }
        if !self.single_revocation_triggers_review {
            return Err(BulwarkError::ConfigError(
                "single_revocation_triggers_review cannot be disabled — it is a circuit breaker"
                    .into(),
            ));
        }
        if !self.proximity_required {
            return Err(BulwarkError::ConfigError(
                "proximity_required cannot be disabled — remote approval defeats KidsSphere's purpose"
                    .into(),
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// KidsSphere Exclusion
// ---------------------------------------------------------------------------

/// A KidsSphere exclusion — Covenant-level protection removing a person from
/// ALL KidsSphere-enabled communities across the entire network.
///
/// Children's Dignity overrides an adult's community access. This is the
/// strongest exclusion in Omnidea.
///
/// Permanent exclusions (`is_permanent == true`) require unanimous adjudication
/// AND an `AdjudicatedPredation` basis. Even permanent exclusions are reviewed
/// by Star Court every 2 years — not for reinstatement, but for process integrity.
///
/// AccusedRights are always on, even for KidsSphere exclusions:
/// - Right to know the charges
/// - Right to respond to evidence
/// - Right to Star Court review
/// - Right to have the process audited
/// - NO right to reinstatement for permanent exclusions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KidsSphereExclusion {
    /// Unique exclusion identifier.
    pub id: Uuid,
    /// The excluded person's public key.
    pub excluded_pubkey: String,
    /// Why this exclusion was established.
    pub basis: KidsExclusionBasis,
    /// The adjudication record supporting this exclusion.
    pub adjudication_record: KidsAdjudicationRecord,
    /// Scope — always AllKidsSphere. Non-negotiable.
    pub scope: KidsSphereExclusionScope,
    /// When the exclusion was established.
    pub established_at: DateTime<Utc>,
    /// Mandatory review schedule (Star Court, every 2 years for process integrity).
    pub review_schedule: KidsReviewSchedule,
    /// Whether this exclusion is permanent.
    ///
    /// `true` only if adjudication was unanimous AND basis is `AdjudicatedPredation`.
    pub is_permanent: bool,
}

impl KidsSphereExclusion {
    /// Create a new KidsSphere exclusion.
    ///
    /// Validates that `is_permanent` is only true when both conditions are met:
    /// unanimous adjudication AND `AdjudicatedPredation` basis.
    pub fn new(
        excluded_pubkey: impl Into<String>,
        basis: KidsExclusionBasis,
        adjudication_record: KidsAdjudicationRecord,
    ) -> Result<Self, BulwarkError> {
        let is_permanent = adjudication_record.unanimous
            && matches!(basis, KidsExclusionBasis::AdjudicatedPredation);

        // Validate minimum adjudication requirements.
        if adjudication_record.dispute_ids.is_empty() && adjudication_record.flag_ids.is_empty() {
            return Err(BulwarkError::KidsExclusionInvalid(
                "exclusion requires at least one dispute or flag".into(),
            ));
        }
        if adjudication_record.adjudicators.is_empty() {
            return Err(BulwarkError::KidsExclusionInvalid(
                "exclusion requires at least one adjudicator".into(),
            ));
        }

        // Cross-community basis requires 3+ independent communities.
        if matches!(basis, KidsExclusionBasis::CrossCommunityMinorSafety)
            && adjudication_record.communities_involved.len() < MINIMUM_CROSS_COMMUNITY_COUNT
        {
            return Err(BulwarkError::KidsExclusionInvalid(format!(
                "CrossCommunityMinorSafety requires at least {} independent communities (got {})",
                MINIMUM_CROSS_COMMUNITY_COUNT,
                adjudication_record.communities_involved.len()
            )));
        }

        let review_schedule = KidsReviewSchedule::new(
            KIDS_REVIEW_INTERVAL_DAYS,
            KidsReviewType::StarCourt,
        );

        Ok(Self {
            id: Uuid::new_v4(),
            excluded_pubkey: excluded_pubkey.into(),
            basis,
            adjudication_record,
            scope: KidsSphereExclusionScope::AllKidsSphere,
            established_at: Utc::now(),
            review_schedule,
            is_permanent,
        })
    }

    /// Whether a review is overdue.
    pub fn is_review_overdue(&self) -> bool {
        self.review_schedule.is_overdue()
    }
}

// ---------------------------------------------------------------------------
// KidsExclusionBasis
// ---------------------------------------------------------------------------

/// Why a person was excluded from all KidsSphere communities.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum KidsExclusionBasis {
    /// Jail Dispute finding of predatory behavior toward a minor.
    ///
    /// Requires: flag category MinorSafety + severity Critical + Dispute status
    /// Closed with finding against the respondent.
    AdjudicatedPredation,

    /// AccountabilityFlags with category MinorSafety from 3+ independent communities.
    ///
    /// Same independence requirements as R2A sustained exclusion (no shared
    /// members between flag sources to prevent coordinated false flagging).
    CrossCommunityMinorSafety,

    /// Bulwark's existing 5-step ChildSafetyProtocol was triggered AND confirmed
    /// by investigation.
    ChildSafetyProtocolTrigger,
}

impl std::fmt::Display for KidsExclusionBasis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AdjudicatedPredation => write!(f, "adjudicated_predation"),
            Self::CrossCommunityMinorSafety => write!(f, "cross_community_minor_safety"),
            Self::ChildSafetyProtocolTrigger => write!(f, "child_safety_protocol_trigger"),
        }
    }
}

// ---------------------------------------------------------------------------
// KidsAdjudicationRecord
// ---------------------------------------------------------------------------

/// The adjudication record supporting a KidsSphere exclusion.
///
/// Documents the disputes, flags, communities, adjudicators, and findings
/// that led to the exclusion. Must be unanimous for the exclusion to be
/// permanent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KidsAdjudicationRecord {
    /// Dispute IDs from Jail that support this exclusion.
    pub dispute_ids: Vec<Uuid>,
    /// AccountabilityFlag IDs from Jail that support this exclusion.
    pub flag_ids: Vec<Uuid>,
    /// Communities involved in the adjudication process.
    pub communities_involved: Vec<String>,
    /// Adjudicator pubkeys who participated in the finding.
    pub adjudicators: Vec<String>,
    /// Summary of the finding.
    pub finding_summary: String,
    /// Whether the adjudication was unanimous.
    ///
    /// Unanimity is required for permanent exclusions under `AdjudicatedPredation`.
    pub unanimous: bool,
}

impl KidsAdjudicationRecord {
    /// Create a new adjudication record.
    pub fn new(finding_summary: impl Into<String>, unanimous: bool) -> Self {
        Self {
            dispute_ids: Vec::new(),
            flag_ids: Vec::new(),
            communities_involved: Vec::new(),
            adjudicators: Vec::new(),
            finding_summary: finding_summary.into(),
            unanimous,
        }
    }

    /// Builder: add a dispute ID.
    pub fn with_dispute(mut self, dispute_id: Uuid) -> Self {
        self.dispute_ids.push(dispute_id);
        self
    }

    /// Builder: add a flag ID.
    pub fn with_flag(mut self, flag_id: Uuid) -> Self {
        self.flag_ids.push(flag_id);
        self
    }

    /// Builder: add a community.
    pub fn with_community(mut self, community_id: impl Into<String>) -> Self {
        self.communities_involved.push(community_id.into());
        self
    }

    /// Builder: add an adjudicator.
    pub fn with_adjudicator(mut self, adjudicator_pubkey: impl Into<String>) -> Self {
        self.adjudicators.push(adjudicator_pubkey.into());
        self
    }

    /// Count communities involved that are visible within the federation scope.
    ///
    /// For `CrossCommunityMinorSafety`, only federated communities should
    /// count toward the 3-community minimum. This method lets callers
    /// verify the threshold is still met within a particular federation.
    pub fn visible_community_count(&self, scope: &FederationScope) -> usize {
        if scope.is_unrestricted() {
            return self.communities_involved.len();
        }
        self.communities_involved
            .iter()
            .filter(|c| scope.is_visible(c))
            .count()
    }
}

// ---------------------------------------------------------------------------
// KidsSphereExclusionScope
// ---------------------------------------------------------------------------

/// The scope of a KidsSphere exclusion — always network-wide.
///
/// This applies to EVERY community with KidsSphere enabled, across the entire
/// network. Non-negotiable. There is only one variant because there is only
/// one acceptable scope for child safety exclusions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum KidsSphereExclusionScope {
    /// Excluded from ALL KidsSphere-enabled communities. Non-negotiable.
    AllKidsSphere,
}

impl std::fmt::Display for KidsSphereExclusionScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AllKidsSphere => write!(f, "all_kids_sphere"),
        }
    }
}

// ---------------------------------------------------------------------------
// KidsReviewSchedule (mirrors Jail's SustainedReviewSchedule for KidsSphere)
// ---------------------------------------------------------------------------

/// Mandatory review schedule for a KidsSphere exclusion.
///
/// Structurally parallel to Jail's `SustainedReviewSchedule` but scoped to
/// KidsSphere. All KidsSphere exclusion reviews go through Star Court
/// (CovenantCourt) — not for reinstatement, but for process integrity.
///
/// Default interval is 730 days (2 years).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KidsReviewSchedule {
    /// Days between reviews (default 730 — biennial for process integrity).
    pub review_interval_days: u64,
    /// When the next review is due.
    pub next_review: DateTime<Utc>,
    /// How many reviews have been completed.
    pub reviews_completed: usize,
    /// What kind of review — always StarCourt for KidsSphere.
    pub review_type: KidsReviewType,
}

impl KidsReviewSchedule {
    /// Create a new review schedule.
    pub fn new(interval_days: u64, review_type: KidsReviewType) -> Self {
        Self {
            review_interval_days: interval_days,
            next_review: Utc::now() + Duration::days(interval_days as i64),
            reviews_completed: 0,
            review_type,
        }
    }

    /// Whether a review is overdue.
    pub fn is_overdue(&self) -> bool {
        Utc::now() > self.next_review
    }

    /// Advance to the next review period.
    pub fn advance(&mut self) {
        self.reviews_completed += 1;
        self.next_review = Utc::now() + Duration::days(self.review_interval_days as i64);
    }
}

/// What kind of review panel conducts the review.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum KidsReviewType {
    /// Review by Star Court — for KidsSphere, this is always the review type.
    /// Not for reinstatement of permanent exclusions — for process integrity only.
    StarCourt,
}

impl std::fmt::Display for KidsReviewType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StarCourt => write!(f, "star_court"),
        }
    }
}

// ---------------------------------------------------------------------------
// Access Check
// ---------------------------------------------------------------------------

/// The result of a KidsSphere access check.
///
/// Access check order is MANDATORY and cannot be reordered:
/// 1. Exclusion registry -> DENIED
/// 2. Valid approval for this community? -> DENIED
/// 3. Expired? -> DENIED
/// 4. Revoked? -> SUSPENDED
/// 5. All pass -> ALLOWED
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KidsSphereAccessResult {
    /// Access granted. All checks passed.
    Allowed,
    /// Access denied — person is on the exclusion registry.
    DeniedExcluded,
    /// Access denied — no valid approval for this community.
    DeniedNoApproval,
    /// Access denied — approval has expired and needs renewal.
    DeniedExpired,
    /// Access suspended — a parent has revoked, pending community review.
    Suspended,
}

impl KidsSphereAccessResult {
    /// Whether access is granted.
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    /// Whether access is denied for any reason.
    pub fn is_denied(&self) -> bool {
        !self.is_allowed()
    }
}

impl std::fmt::Display for KidsSphereAccessResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allowed => write!(f, "allowed"),
            Self::DeniedExcluded => write!(f, "denied_excluded"),
            Self::DeniedNoApproval => write!(f, "denied_no_approval"),
            Self::DeniedExpired => write!(f, "denied_expired"),
            Self::Suspended => write!(f, "suspended"),
        }
    }
}

/// Check whether a pubkey may access KidsSphere areas in a specific community.
///
/// Implements the mandatory 5-step access check:
/// 1. Exclusion registry check
/// 2. Valid approval check
/// 3. Expiration check
/// 4. Revocation check
/// 5. ALLOWED
///
/// Both the exclusion check and the approval check are mandatory.
/// Neither can be disabled by charter.
pub fn check_kids_sphere_access(
    pubkey: &str,
    community_id: &str,
    registry: &KidsSphereExclusionRegistry,
    approval: Option<&KidsSphereApproval>,
) -> KidsSphereAccessResult {
    // Step 1: Exclusion registry check — full stop if excluded.
    if registry.is_excluded(pubkey) {
        return KidsSphereAccessResult::DeniedExcluded;
    }

    // Step 2: Must have a valid approval for THIS community.
    let approval = match approval {
        Some(a) if a.community_id == community_id && a.candidate_pubkey == pubkey => a,
        _ => return KidsSphereAccessResult::DeniedNoApproval,
    };

    // Step 3: Expired?
    if approval.is_expired() {
        return KidsSphereAccessResult::DeniedExpired;
    }

    // Step 4: Revoked by any parent?
    if approval.is_revoked() {
        return KidsSphereAccessResult::Suspended;
    }

    // Step 5: All checks pass.
    KidsSphereAccessResult::Allowed
}

// ---------------------------------------------------------------------------
// KidsSphere Exclusion Registry
// ---------------------------------------------------------------------------

/// Thread-safe registry of pubkeys excluded from all KidsSphere communities.
///
/// This is the first check in the access chain — if a pubkey is here, access
/// is DENIED before anything else is evaluated.
///
/// The registry is append-heavy (exclusions are rare but permanent-ish) and
/// read-heavy (checked on every KidsSphere access). `RwLock` gives concurrent
/// reads with exclusive writes.
#[derive(Debug, Clone)]
pub struct KidsSphereExclusionRegistry {
    /// The set of excluded pubkeys. `Arc<RwLock<_>>` for thread-safe sharing.
    exclusions: Arc<RwLock<HashSet<String>>>,
}

impl Default for KidsSphereExclusionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl KidsSphereExclusionRegistry {
    /// Create a new, empty exclusion registry.
    pub fn new() -> Self {
        Self {
            exclusions: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Check whether a pubkey is excluded from all KidsSphere communities.
    pub fn is_excluded(&self, pubkey: &str) -> bool {
        self.exclusions
            .read()
            .expect("KidsSphereExclusionRegistry lock poisoned")
            .contains(pubkey)
    }

    /// Add a pubkey to the exclusion registry.
    ///
    /// Returns `true` if the pubkey was newly inserted, `false` if already excluded.
    pub fn exclude(&self, pubkey: impl Into<String>) -> bool {
        self.exclusions
            .write()
            .expect("KidsSphereExclusionRegistry lock poisoned")
            .insert(pubkey.into())
    }

    /// How many pubkeys are currently excluded.
    pub fn count(&self) -> usize {
        self.exclusions
            .read()
            .expect("KidsSphereExclusionRegistry lock poisoned")
            .len()
    }

    /// Get a snapshot of all excluded pubkeys.
    ///
    /// Returns a clone so callers don't hold the lock.
    pub fn snapshot(&self) -> HashSet<String> {
        self.exclusions
            .read()
            .expect("KidsSphereExclusionRegistry lock poisoned")
            .clone()
    }
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default minimum number of parent approvals required.
pub const DEFAULT_MIN_APPROVALS: usize = 3;

/// Absolute minimum for `min_approvals` — one parent is never enough.
pub const MINIMUM_MIN_APPROVALS: usize = 2;

/// Default renewal interval in days (365 = annual).
pub const DEFAULT_RENEWAL_INTERVAL_DAYS: u64 = 365;

/// Maximum renewal interval in days (730 = biennial).
pub const MAXIMUM_RENEWAL_INTERVAL_DAYS: u64 = 730;

/// KidsSphere exclusion review interval in days (730 = every 2 years).
pub const KIDS_REVIEW_INTERVAL_DAYS: u64 = 730;

/// Minimum number of independent communities for `CrossCommunityMinorSafety`.
pub const MINIMUM_CROSS_COMMUNITY_COUNT: usize = 3;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verification::proximity::ProximityProof;

    // ── Helpers ──────────────────────────────────────────────────────────

    fn valid_proof() -> ProximityProof {
        ProximityProof::new("test_nonce").with_ble(-45)
    }

    fn make_approval(parent: &str, children: Vec<&str>, confidence: ApprovalConfidence) -> ParentalApproval {
        ParentalApproval::new(
            parent,
            children.into_iter().map(String::from).collect(),
            confidence,
        )
    }

    fn make_approvals(count: usize) -> Vec<ParentalApproval> {
        (0..count)
            .map(|i| make_approval(
                &format!("parent_{i}"),
                vec!["child_a"],
                ApprovalConfidence::Comfortable,
            ))
            .collect()
    }

    fn default_policy() -> KidsSphereApprovalPolicy {
        KidsSphereApprovalPolicy::default()
    }

    fn make_adjudication_record(
        unanimous: bool,
        community_count: usize,
    ) -> KidsAdjudicationRecord {
        let mut record = KidsAdjudicationRecord::new("Finding summary", unanimous)
            .with_dispute(Uuid::new_v4())
            .with_adjudicator("adjudicator_1");
        for i in 0..community_count {
            record = record.with_community(format!("community_{i}"));
        }
        record
    }

    fn make_valid_approval(
        candidate: &str,
        community: &str,
    ) -> KidsSphereApproval {
        KidsSphereApproval::new(
            candidate,
            community,
            make_approvals(3),
            valid_proof(),
            &default_policy(),
        )
        .expect("valid approval")
    }

    // ── ApprovalConfidence ──────────────────────────────────────────────

    #[test]
    fn comfortable_does_not_trigger_notification() {
        assert!(!ApprovalConfidence::Comfortable.triggers_notification());
    }

    #[test]
    fn cautious_does_not_trigger_notification() {
        assert!(!ApprovalConfidence::Cautious.triggers_notification());
    }

    #[test]
    fn reluctant_triggers_notification() {
        assert!(ApprovalConfidence::Reluctant.triggers_notification());
    }

    #[test]
    fn confidence_display() {
        assert_eq!(ApprovalConfidence::Comfortable.to_string(), "comfortable");
        assert_eq!(ApprovalConfidence::Cautious.to_string(), "cautious");
        assert_eq!(ApprovalConfidence::Reluctant.to_string(), "reluctant");
    }

    // ── ParentalApproval ────────────────────────────────────────────────

    #[test]
    fn parental_approval_creation() {
        let approval = make_approval("parent_a", vec!["kid_1", "kid_2"], ApprovalConfidence::Comfortable);
        assert_eq!(approval.parent_pubkey, "parent_a");
        assert_eq!(approval.child_pubkeys.len(), 2);
        assert!(!approval.is_reluctant());
    }

    #[test]
    fn reluctant_approval_detected() {
        let approval = make_approval("parent_a", vec!["kid_1"], ApprovalConfidence::Reluctant);
        assert!(approval.is_reluctant());
    }

    // ── KidsSphereApproval ──────────────────────────────────────────────

    #[test]
    fn approval_requires_minimum_parents() {
        let result = KidsSphereApproval::new(
            "candidate",
            "community_1",
            make_approvals(2), // only 2, need 3
            valid_proof(),
            &default_policy(),
        );
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(BulwarkError::InsufficientApprovals { have: 2, need: 3 })
        ));
    }

    #[test]
    fn approval_requires_proximity_proof() {
        let no_evidence = ProximityProof::new("nonce"); // no BLE/NFC/ultrasonic
        let result = KidsSphereApproval::new(
            "candidate",
            "community_1",
            make_approvals(3),
            no_evidence,
            &default_policy(),
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(BulwarkError::ProximityRequired(_))));
    }

    #[test]
    fn valid_approval_creation() {
        let approval = make_valid_approval("candidate", "community_1");
        assert!(approval.is_valid());
        assert!(!approval.is_expired());
        assert!(!approval.is_revoked());
        assert_eq!(approval.approval_count(), 3);
    }

    #[test]
    fn approval_is_community_specific() {
        let approval = make_valid_approval("candidate", "community_1");
        assert_eq!(approval.community_id, "community_1");
        // A different community_id means this approval doesn't apply there.
    }

    #[test]
    fn approval_revocation() {
        let mut approval = make_valid_approval("candidate", "community_1");
        assert!(!approval.is_revoked());

        approval.revoke();
        assert!(approval.is_revoked());
        assert!(!approval.is_valid());
    }

    #[test]
    fn double_revocation_keeps_original_timestamp() {
        let mut approval = make_valid_approval("candidate", "community_1");
        approval.revoke();
        let first_revoked = approval.revoked_at;

        // Second revoke should not change the timestamp.
        approval.revoke();
        assert_eq!(approval.revoked_at, first_revoked);
    }

    #[test]
    fn expired_approval_is_invalid() {
        let mut approval = make_valid_approval("candidate", "community_1");
        // Force expiration.
        approval.expires_at = Utc::now() - Duration::seconds(1);
        assert!(approval.is_expired());
        assert!(!approval.is_valid());
    }

    #[test]
    fn reluctant_approval_detected_in_collective() {
        let approvals = vec![
            make_approval("p1", vec!["kid"], ApprovalConfidence::Comfortable),
            make_approval("p2", vec!["kid"], ApprovalConfidence::Cautious),
            make_approval("p3", vec!["kid"], ApprovalConfidence::Reluctant),
        ];
        let approval = KidsSphereApproval::new(
            "candidate",
            "community_1",
            approvals,
            valid_proof(),
            &default_policy(),
        )
        .unwrap();
        assert!(approval.has_reluctant_approval());
    }

    #[test]
    fn annual_renewal_default() {
        let approval = make_valid_approval("candidate", "community_1");
        // Expires approximately 365 days from now.
        let days_until_expiry = (approval.expires_at - approval.approved_at).num_days();
        assert_eq!(days_until_expiry, 365);
    }

    // ── KidsSphereApprovalPolicy ────────────────────────────────────────

    #[test]
    fn default_policy_is_valid() {
        assert!(default_policy().validate().is_ok());
    }

    #[test]
    fn policy_rejects_too_few_approvals() {
        let policy = KidsSphereApprovalPolicy {
            min_approvals: 1,
            ..Default::default()
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn policy_rejects_excessive_renewal_interval() {
        let policy = KidsSphereApprovalPolicy {
            renewal_interval_days: 1000,
            ..Default::default()
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn policy_rejects_disabled_revocation_review() {
        let policy = KidsSphereApprovalPolicy {
            single_revocation_triggers_review: false,
            ..Default::default()
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn policy_rejects_disabled_proximity() {
        let policy = KidsSphereApprovalPolicy {
            proximity_required: false,
            ..Default::default()
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn policy_allows_higher_min_approvals() {
        let policy = KidsSphereApprovalPolicy {
            min_approvals: 5,
            ..Default::default()
        };
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn policy_boundary_min_approvals() {
        let policy = KidsSphereApprovalPolicy {
            min_approvals: MINIMUM_MIN_APPROVALS,
            ..Default::default()
        };
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn policy_boundary_renewal_interval() {
        let policy = KidsSphereApprovalPolicy {
            renewal_interval_days: MAXIMUM_RENEWAL_INTERVAL_DAYS,
            ..Default::default()
        };
        assert!(policy.validate().is_ok());
    }

    // ── KidsExclusionBasis ──────────────────────────────────────────────

    #[test]
    fn exclusion_basis_display() {
        assert_eq!(
            KidsExclusionBasis::AdjudicatedPredation.to_string(),
            "adjudicated_predation"
        );
        assert_eq!(
            KidsExclusionBasis::CrossCommunityMinorSafety.to_string(),
            "cross_community_minor_safety"
        );
        assert_eq!(
            KidsExclusionBasis::ChildSafetyProtocolTrigger.to_string(),
            "child_safety_protocol_trigger"
        );
    }

    // ── KidsAdjudicationRecord ──────────────────────────────────────────

    #[test]
    fn adjudication_record_builder() {
        let record = KidsAdjudicationRecord::new("Predatory behavior confirmed", true)
            .with_dispute(Uuid::new_v4())
            .with_flag(Uuid::new_v4())
            .with_community("community_1")
            .with_community("community_2")
            .with_adjudicator("adj_1")
            .with_adjudicator("adj_2");

        assert_eq!(record.dispute_ids.len(), 1);
        assert_eq!(record.flag_ids.len(), 1);
        assert_eq!(record.communities_involved.len(), 2);
        assert_eq!(record.adjudicators.len(), 2);
        assert!(record.unanimous);
    }

    // ── KidsSphereExclusion ─────────────────────────────────────────────

    #[test]
    fn exclusion_permanent_when_unanimous_and_predation() {
        let record = make_adjudication_record(true, 1);
        let exclusion = KidsSphereExclusion::new(
            "predator_pubkey",
            KidsExclusionBasis::AdjudicatedPredation,
            record,
        )
        .unwrap();
        assert!(exclusion.is_permanent);
    }

    #[test]
    fn exclusion_not_permanent_when_not_unanimous() {
        let record = make_adjudication_record(false, 1);
        let exclusion = KidsSphereExclusion::new(
            "predator_pubkey",
            KidsExclusionBasis::AdjudicatedPredation,
            record,
        )
        .unwrap();
        assert!(!exclusion.is_permanent);
    }

    #[test]
    fn exclusion_not_permanent_when_wrong_basis() {
        let record = make_adjudication_record(true, 3);
        let exclusion = KidsSphereExclusion::new(
            "offender_pubkey",
            KidsExclusionBasis::CrossCommunityMinorSafety,
            record,
        )
        .unwrap();
        assert!(!exclusion.is_permanent);
    }

    #[test]
    fn exclusion_scope_always_all_kids_sphere() {
        let record = make_adjudication_record(true, 1);
        let exclusion = KidsSphereExclusion::new(
            "pubkey",
            KidsExclusionBasis::AdjudicatedPredation,
            record,
        )
        .unwrap();
        assert_eq!(exclusion.scope, KidsSphereExclusionScope::AllKidsSphere);
    }

    #[test]
    fn exclusion_requires_evidence() {
        let record = KidsAdjudicationRecord::new("No evidence", true)
            .with_adjudicator("adj_1");
        let result = KidsSphereExclusion::new(
            "pubkey",
            KidsExclusionBasis::AdjudicatedPredation,
            record,
        );
        assert!(result.is_err());
    }

    #[test]
    fn exclusion_requires_adjudicator() {
        let record = KidsAdjudicationRecord::new("No adjudicator", true)
            .with_dispute(Uuid::new_v4());
        let result = KidsSphereExclusion::new(
            "pubkey",
            KidsExclusionBasis::AdjudicatedPredation,
            record,
        );
        assert!(result.is_err());
    }

    #[test]
    fn cross_community_requires_three_communities() {
        let record = make_adjudication_record(true, 2); // only 2
        let result = KidsSphereExclusion::new(
            "pubkey",
            KidsExclusionBasis::CrossCommunityMinorSafety,
            record,
        );
        assert!(result.is_err());
    }

    #[test]
    fn cross_community_with_three_communities_succeeds() {
        let record = make_adjudication_record(true, 3);
        let result = KidsSphereExclusion::new(
            "pubkey",
            KidsExclusionBasis::CrossCommunityMinorSafety,
            record,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn exclusion_review_schedule_is_star_court() {
        let record = make_adjudication_record(true, 1);
        let exclusion = KidsSphereExclusion::new(
            "pubkey",
            KidsExclusionBasis::AdjudicatedPredation,
            record,
        )
        .unwrap();
        assert_eq!(exclusion.review_schedule.review_type, KidsReviewType::StarCourt);
    }

    #[test]
    fn exclusion_review_interval_is_two_years() {
        let record = make_adjudication_record(true, 1);
        let exclusion = KidsSphereExclusion::new(
            "pubkey",
            KidsExclusionBasis::AdjudicatedPredation,
            record,
        )
        .unwrap();
        assert_eq!(exclusion.review_schedule.review_interval_days, 730);
    }

    // ── KidsSphereExclusionScope ────────────────────────────────────────

    #[test]
    fn scope_display() {
        assert_eq!(
            KidsSphereExclusionScope::AllKidsSphere.to_string(),
            "all_kids_sphere"
        );
    }

    // ── KidsReviewSchedule ──────────────────────────────────────────────

    #[test]
    fn review_schedule_creation() {
        let schedule = KidsReviewSchedule::new(730, KidsReviewType::StarCourt);
        assert_eq!(schedule.review_interval_days, 730);
        assert_eq!(schedule.reviews_completed, 0);
        assert!(!schedule.is_overdue());
    }

    #[test]
    fn review_schedule_overdue() {
        let mut schedule = KidsReviewSchedule::new(730, KidsReviewType::StarCourt);
        schedule.next_review = Utc::now() - Duration::seconds(1);
        assert!(schedule.is_overdue());
    }

    #[test]
    fn review_schedule_advance() {
        let mut schedule = KidsReviewSchedule::new(730, KidsReviewType::StarCourt);
        schedule.advance();
        assert_eq!(schedule.reviews_completed, 1);
        assert!(!schedule.is_overdue());
    }

    // ── KidsSphereExclusionRegistry ─────────────────────────────────────

    #[test]
    fn empty_registry() {
        let registry = KidsSphereExclusionRegistry::new();
        assert_eq!(registry.count(), 0);
        assert!(!registry.is_excluded("anyone"));
    }

    #[test]
    fn exclude_and_check() {
        let registry = KidsSphereExclusionRegistry::new();
        assert!(registry.exclude("predator_pubkey"));
        assert!(registry.is_excluded("predator_pubkey"));
        assert!(!registry.is_excluded("innocent_pubkey"));
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn double_exclude_returns_false() {
        let registry = KidsSphereExclusionRegistry::new();
        assert!(registry.exclude("pubkey"));
        assert!(!registry.exclude("pubkey")); // already excluded
    }

    #[test]
    fn registry_snapshot() {
        let registry = KidsSphereExclusionRegistry::new();
        registry.exclude("a");
        registry.exclude("b");
        let snapshot = registry.snapshot();
        assert_eq!(snapshot.len(), 2);
        assert!(snapshot.contains("a"));
        assert!(snapshot.contains("b"));
    }

    #[test]
    fn registry_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<KidsSphereExclusionRegistry>();
    }

    #[test]
    fn registry_default() {
        let registry = KidsSphereExclusionRegistry::default();
        assert_eq!(registry.count(), 0);
    }

    // ── Access Check ────────────────────────────────────────────────────

    #[test]
    fn access_denied_when_excluded() {
        let registry = KidsSphereExclusionRegistry::new();
        registry.exclude("bad_actor");
        let approval = make_valid_approval("bad_actor", "community_1");

        let result = check_kids_sphere_access(
            "bad_actor",
            "community_1",
            &registry,
            Some(&approval),
        );
        assert_eq!(result, KidsSphereAccessResult::DeniedExcluded);
        assert!(result.is_denied());
    }

    #[test]
    fn access_denied_when_no_approval() {
        let registry = KidsSphereExclusionRegistry::new();

        let result = check_kids_sphere_access(
            "new_person",
            "community_1",
            &registry,
            None,
        );
        assert_eq!(result, KidsSphereAccessResult::DeniedNoApproval);
    }

    #[test]
    fn access_denied_when_wrong_community() {
        let registry = KidsSphereExclusionRegistry::new();
        let approval = make_valid_approval("candidate", "community_1");

        // Checking access for community_2 but approval is for community_1.
        let result = check_kids_sphere_access(
            "candidate",
            "community_2",
            &registry,
            Some(&approval),
        );
        assert_eq!(result, KidsSphereAccessResult::DeniedNoApproval);
    }

    #[test]
    fn access_denied_when_wrong_candidate() {
        let registry = KidsSphereExclusionRegistry::new();
        let approval = make_valid_approval("candidate_a", "community_1");

        // Checking access for candidate_b but approval is for candidate_a.
        let result = check_kids_sphere_access(
            "candidate_b",
            "community_1",
            &registry,
            Some(&approval),
        );
        assert_eq!(result, KidsSphereAccessResult::DeniedNoApproval);
    }

    #[test]
    fn access_denied_when_expired() {
        let registry = KidsSphereExclusionRegistry::new();
        let mut approval = make_valid_approval("candidate", "community_1");
        approval.expires_at = Utc::now() - Duration::seconds(1);

        let result = check_kids_sphere_access(
            "candidate",
            "community_1",
            &registry,
            Some(&approval),
        );
        assert_eq!(result, KidsSphereAccessResult::DeniedExpired);
    }

    #[test]
    fn access_suspended_when_revoked() {
        let registry = KidsSphereExclusionRegistry::new();
        let mut approval = make_valid_approval("candidate", "community_1");
        approval.revoke();

        let result = check_kids_sphere_access(
            "candidate",
            "community_1",
            &registry,
            Some(&approval),
        );
        assert_eq!(result, KidsSphereAccessResult::Suspended);
    }

    #[test]
    fn access_allowed_when_all_checks_pass() {
        let registry = KidsSphereExclusionRegistry::new();
        let approval = make_valid_approval("candidate", "community_1");

        let result = check_kids_sphere_access(
            "candidate",
            "community_1",
            &registry,
            Some(&approval),
        );
        assert_eq!(result, KidsSphereAccessResult::Allowed);
        assert!(result.is_allowed());
    }

    #[test]
    fn exclusion_takes_priority_over_valid_approval() {
        // Even with a valid approval, exclusion wins.
        let registry = KidsSphereExclusionRegistry::new();
        registry.exclude("candidate");
        let approval = make_valid_approval("candidate", "community_1");

        let result = check_kids_sphere_access(
            "candidate",
            "community_1",
            &registry,
            Some(&approval),
        );
        assert_eq!(result, KidsSphereAccessResult::DeniedExcluded);
    }

    #[test]
    fn access_result_display() {
        assert_eq!(KidsSphereAccessResult::Allowed.to_string(), "allowed");
        assert_eq!(KidsSphereAccessResult::DeniedExcluded.to_string(), "denied_excluded");
        assert_eq!(KidsSphereAccessResult::DeniedNoApproval.to_string(), "denied_no_approval");
        assert_eq!(KidsSphereAccessResult::DeniedExpired.to_string(), "denied_expired");
        assert_eq!(KidsSphereAccessResult::Suspended.to_string(), "suspended");
    }

    // ── Identity Rebirth Defense (via approval requirement) ─────────────

    #[test]
    fn identity_rebirth_requires_new_approval() {
        // If a predator creates a new identity, they have NO approval
        // for any community — they must go through the full collective
        // meeting process again. This test verifies that a new identity
        // with no approval is denied.
        let registry = KidsSphereExclusionRegistry::new();
        // Old identity is excluded.
        registry.exclude("old_identity");

        // New identity has no approval.
        let result = check_kids_sphere_access(
            "new_identity",
            "community_1",
            &registry,
            None,
        );
        assert_eq!(result, KidsSphereAccessResult::DeniedNoApproval);
    }

    // ── Integration: Exclusion + Approval are both mandatory ────────────

    #[test]
    fn both_checks_mandatory_not_just_approval() {
        // A person with no exclusion but no approval is still denied.
        let registry = KidsSphereExclusionRegistry::new();
        let result = check_kids_sphere_access(
            "person",
            "community_1",
            &registry,
            None,
        );
        assert!(result.is_denied());
    }

    #[test]
    fn both_checks_mandatory_not_just_exclusion() {
        // A person not excluded but with an expired approval is still denied.
        let registry = KidsSphereExclusionRegistry::new();
        let mut approval = make_valid_approval("person", "community_1");
        approval.expires_at = Utc::now() - Duration::seconds(1);

        let result = check_kids_sphere_access(
            "person",
            "community_1",
            &registry,
            Some(&approval),
        );
        assert_eq!(result, KidsSphereAccessResult::DeniedExpired);
    }

    // ── Child safety protocol trigger basis ─────────────────────────────

    #[test]
    fn child_safety_protocol_trigger_exclusion() {
        let record = make_adjudication_record(true, 1);
        let exclusion = KidsSphereExclusion::new(
            "pubkey",
            KidsExclusionBasis::ChildSafetyProtocolTrigger,
            record,
        )
        .unwrap();
        // ChildSafetyProtocolTrigger is not AdjudicatedPredation, so not permanent
        // even if unanimous.
        assert!(!exclusion.is_permanent);
    }

    // ── KidsReviewType ──────────────────────────────────────────────────

    #[test]
    fn review_type_display() {
        assert_eq!(KidsReviewType::StarCourt.to_string(), "star_court");
    }

    // ── Federation Scope ──────────────────────────────────────────────────

    #[test]
    fn visible_community_count_unrestricted() {
        let record = KidsAdjudicationRecord::new("test", true)
            .with_community("alpha")
            .with_community("beta")
            .with_community("gamma")
            .with_adjudicator("adj1")
            .with_flag(Uuid::new_v4());

        assert_eq!(
            record.visible_community_count(&FederationScope::new()),
            3
        );
    }

    #[test]
    fn visible_community_count_scoped() {
        let record = KidsAdjudicationRecord::new("test", true)
            .with_community("alpha")
            .with_community("beta")
            .with_community("gamma")
            .with_community("delta")
            .with_adjudicator("adj1")
            .with_flag(Uuid::new_v4());

        // Only alpha and gamma in scope.
        let scope = FederationScope::from_communities(["alpha", "gamma"]);
        assert_eq!(record.visible_community_count(&scope), 2);
    }

    #[test]
    fn cross_community_threshold_within_federation() {
        // 4 communities total, but only 2 visible in federation.
        // CrossCommunityMinorSafety requires 3+ — so it's insufficient.
        let record = KidsAdjudicationRecord::new("test", true)
            .with_community("alpha")
            .with_community("beta")
            .with_community("gamma")
            .with_community("delta")
            .with_adjudicator("adj1")
            .with_flag(Uuid::new_v4());

        let scope = FederationScope::from_communities(["alpha", "beta"]);
        let visible = record.visible_community_count(&scope);
        assert!(
            visible < MINIMUM_CROSS_COMMUNITY_COUNT,
            "Only {visible} visible communities, need {MINIMUM_CROSS_COMMUNITY_COUNT}"
        );

        // Expand scope to include gamma — now meets threshold.
        let scope = FederationScope::from_communities(["alpha", "beta", "gamma"]);
        let visible = record.visible_community_count(&scope);
        assert!(visible >= MINIMUM_CROSS_COMMUNITY_COUNT);
    }
}
