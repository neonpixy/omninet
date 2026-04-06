//! Sustained Exclusion — above GraduatedResponse, for repeat offenders.
//!
//! For cases where ProtectiveExclusion's mandatory review keeps reinstating
//! someone who reoffends. This is the last escalation: a long-duration exclusion
//! with annual (not quarterly) reviews, requiring multi-community consensus.
//!
//! From Constellation Art. 7 §12: "No enforcement action shall... create
//! permanent castes of excluded persons or communities."
//!
//! Even sustained exclusion is not permanent. Annual reviews are mandatory.
//! Crown identity, Vault data, Fortune balance, and the ability to create
//! one's own community are NEVER affected. Exclusion constrains access,
//! not existence.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::JailError;
use crate::rights::AccusedRights;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A sustained exclusion — long-duration separation for repeat offenders.
///
/// Requires multi-community affirmation, substantial evidence spanning 90+ days,
/// and annual mandatory review. Scope NEVER includes Crown, Vault, Fortune, or
/// the ability to create one's own community.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SustainedExclusion {
    /// Unique exclusion identifier.
    pub id: Uuid,
    /// The excluded person's public key.
    pub excluded_pubkey: String,
    /// Why this sustained exclusion was established.
    pub basis: SustainedExclusionBasis,
    /// Chain of evidence from multiple sources.
    pub evidence_chain: Vec<ExclusionEvidence>,
    /// Communities that affirmed this exclusion (minimum 3).
    pub communities_affirming: Vec<CommunityAffirmation>,
    /// When the sustained exclusion was established.
    pub established_at: DateTime<Utc>,
    /// What access is constrained.
    pub scope: ExclusionScope,
    /// Mandatory review schedule (default annual).
    pub review_schedule: SustainedReviewSchedule,
    /// History of reviews.
    pub reviews: Vec<SustainedReview>,
    /// When lifted (if ever).
    pub lifted_at: Option<DateTime<Utc>>,
    /// The excluded person's rights — always on.
    pub accused_rights: AccusedRights,
}

impl SustainedExclusion {
    /// Whether the exclusion is still active.
    pub fn is_active(&self) -> bool {
        self.lifted_at.is_none()
    }

    /// Whether a review is overdue.
    pub fn is_review_overdue(&self) -> bool {
        self.is_active() && self.review_schedule.is_overdue()
    }

    /// Record a review of this sustained exclusion.
    pub fn record_review(&mut self, review: SustainedReview) {
        match &review.finding {
            SustainedReviewFinding::Lift => {
                self.lifted_at = Some(review.reviewed_at);
            }
            SustainedReviewFinding::ModifyScope(new_scope) => {
                self.scope = new_scope.clone();
            }
            SustainedReviewFinding::Maintain => {}
        }
        self.review_schedule.advance();
        self.reviews.push(review);
    }

    /// Validate that the exclusion's scope respects sovereign rights.
    ///
    /// Scope NEVER includes Crown identity, Vault data, Fortune balance,
    /// or the ability to create one's own community.
    pub fn validate_scope(&self) -> bool {
        self.scope.respects_sovereign_rights()
    }

    /// Validate that accused rights are preserved.
    pub fn validate_rights(&self) -> bool {
        self.accused_rights.validate()
    }

    /// List the inalienable rights that are always retained regardless of exclusion.
    pub fn inalienable_rights() -> &'static [&'static str] {
        &[
            "Crown identity (always retained)",
            "Vault data (always retained)",
            "Fortune balance (always retained)",
            "Ability to create own community (always retained)",
        ]
    }
}

// ---------------------------------------------------------------------------
// Basis
// ---------------------------------------------------------------------------

/// Why a sustained exclusion was established.
///
/// Each basis has strict evidence requirements to prevent weaponization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SustainedExclusionBasis {
    /// Excluded, reviewed, reinstated, reoffended, re-excluded — at least twice.
    ///
    /// Requires at least 2 full cycles of:
    /// ProtectiveExclusion -> Review -> Reinstatement -> Reoffense
    RepeatedProtectiveExclusion {
        /// Number of completed exclusion-reinstatement-reoffense cycles.
        cycle_count: usize,
    },

    /// Accountability flags from 3+ independent communities with no shared
    /// founding members between flag sources (prevents coordinated false flagging).
    CrossCommunityPattern {
        /// Number of independent communities reporting.
        community_count: usize,
    },

    /// Jail Dispute resolved with finding of serious harm.
    /// Only `Grave` or `Existential` severity qualifies.
    AdjudicatedHarm {
        /// The severity finding from the adjudication.
        severity: AdjudicatedSeverity,
        /// The dispute ID that produced this finding.
        dispute_id: Uuid,
    },
}

impl std::fmt::Display for SustainedExclusionBasis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RepeatedProtectiveExclusion { cycle_count } => {
                write!(f, "repeated_protective_exclusion ({cycle_count} cycles)")
            }
            Self::CrossCommunityPattern { community_count } => {
                write!(f, "cross_community_pattern ({community_count} communities)")
            }
            Self::AdjudicatedHarm { severity, .. } => {
                write!(f, "adjudicated_harm ({severity})")
            }
        }
    }
}

/// Severity levels that qualify for AdjudicatedHarm basis.
///
/// Mirrors Polity's `BreachSeverity` for the two levels that qualify.
/// Jail does not depend on Polity — this is an intentional local equivalent
/// for the sustained exclusion context only.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AdjudicatedSeverity {
    /// Widespread harm, structural cause.
    Grave,
    /// Foundational — threatens the Core or Commons.
    Existential,
}

impl std::fmt::Display for AdjudicatedSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Grave => write!(f, "grave"),
            Self::Existential => write!(f, "existential"),
        }
    }
}

// ---------------------------------------------------------------------------
// Evidence
// ---------------------------------------------------------------------------

/// A piece of evidence supporting a sustained exclusion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExclusionEvidence {
    /// Where this evidence came from.
    pub source_type: EvidenceSource,
    /// The ID of the source record (flag, dispute, exclusion, or report).
    pub source_id: Uuid,
    /// The community where this evidence originated.
    pub community_id: String,
    /// Human-readable summary of the evidence.
    pub summary: String,
    /// Content-addressed hashes of supporting material.
    pub evidence_hashes: Vec<String>,
    /// When this evidence was submitted.
    pub submitted_at: DateTime<Utc>,
}

/// Where a piece of exclusion evidence came from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EvidenceSource {
    /// An AccountabilityFlag.
    Flag,
    /// A resolved Dispute.
    Dispute,
    /// A previous ProtectiveExclusion record.
    ExclusionRecord,
    /// A cross-community report (DutyToWarn pathway).
    CrossCommunityReport,
}

impl std::fmt::Display for EvidenceSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Flag => write!(f, "flag"),
            Self::Dispute => write!(f, "dispute"),
            Self::ExclusionRecord => write!(f, "exclusion_record"),
            Self::CrossCommunityReport => write!(f, "cross_community_report"),
        }
    }
}

// ---------------------------------------------------------------------------
// Community Affirmation
// ---------------------------------------------------------------------------

/// A community's formal affirmation of a sustained exclusion.
///
/// Must be backed by a Kingdom Proposal (governance decision).
/// Minimum 3 independent communities required.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommunityAffirmation {
    /// The affirming community.
    pub community_id: String,
    /// The Kingdom Proposal ID that approved participation.
    pub decision_id: Uuid,
    /// When the affirmation was recorded.
    pub affirmed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Scope
// ---------------------------------------------------------------------------

/// What access a sustained exclusion constrains.
///
/// **Covenant guarantee:** Scope NEVER includes Crown identity, Vault data,
/// Fortune balance, or the ability to create one's own community.
/// Exclusion constrains access, not existence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExclusionScope {
    /// Excluded from specific named communities only.
    SpecificCommunities(Vec<String>),
    /// Excluded from ALL communities with KidsSphere enabled (see R2B).
    AllKidsSphere,
    /// Excluded from all communities that affirmed the exclusion.
    AllAffirming,
}

impl ExclusionScope {
    /// Validates that this scope respects sovereign rights.
    ///
    /// Always returns true for the defined variants because the type system
    /// prevents representing scopes that touch Crown/Vault/Fortune/community-creation.
    /// This method exists as a runtime safety check and documentation.
    pub fn respects_sovereign_rights(&self) -> bool {
        // The enum variants can only express community-level access constraints.
        // Crown identity, Vault data, Fortune balance, and community creation
        // are architecturally unreachable from this type.
        true
    }

    /// Human-readable description of what this scope constrains.
    pub fn description(&self) -> String {
        match self {
            Self::SpecificCommunities(ids) => {
                format!("Excluded from {} specific communities", ids.len())
            }
            Self::AllKidsSphere => {
                "Excluded from all KidsSphere-enabled communities".to_string()
            }
            Self::AllAffirming => {
                "Excluded from all affirming communities".to_string()
            }
        }
    }
}

impl std::fmt::Display for ExclusionScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpecificCommunities(ids) => {
                write!(f, "specific_communities({})", ids.len())
            }
            Self::AllKidsSphere => write!(f, "all_kidssphere"),
            Self::AllAffirming => write!(f, "all_affirming"),
        }
    }
}

// ---------------------------------------------------------------------------
// Review Schedule & Review
// ---------------------------------------------------------------------------

/// Mandatory review schedule for a sustained exclusion.
///
/// Default interval is 365 days (annual), much longer than ProtectiveExclusion's
/// 90-day cycle. This reflects the sustained nature while still ensuring
/// no exclusion lasts forever without re-evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SustainedReviewSchedule {
    /// Days between reviews (default 365).
    pub review_interval_days: u64,
    /// When the next review is due.
    pub next_review: DateTime<Utc>,
    /// How many reviews have been completed.
    pub reviews_completed: usize,
    /// What kind of review is required.
    pub review_type: SustainedReviewType,
}

impl SustainedReviewSchedule {
    /// Create a new review schedule with the given interval and type.
    pub fn new(interval_days: u64, review_type: SustainedReviewType) -> Self {
        Self {
            review_interval_days: interval_days,
            next_review: Utc::now() + Duration::days(interval_days as i64),
            reviews_completed: 0,
            review_type,
        }
    }

    /// Default annual review with Standard type.
    pub fn annual() -> Self {
        Self::new(365, SustainedReviewType::Standard)
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
pub enum SustainedReviewType {
    /// Review panel drawn from affirming communities.
    Standard,
    /// Review by Star Court (R1B) — for KidsSphere and cross-community patterns.
    CovenantCourt,
}

impl std::fmt::Display for SustainedReviewType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standard => write!(f, "standard"),
            Self::CovenantCourt => write!(f, "covenant_court"),
        }
    }
}

/// A completed review of a sustained exclusion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SustainedReview {
    /// Unique review identifier.
    pub id: Uuid,
    /// What kind of review was conducted.
    pub review_type: SustainedReviewType,
    /// Adjudicator public keys on the review panel.
    pub panel: Vec<String>,
    /// The panel's finding.
    pub finding: SustainedReviewFinding,
    /// Written reasoning for the finding.
    pub reasoning: String,
    /// When the review was completed.
    pub reviewed_at: DateTime<Utc>,
}

/// Outcome of a sustained exclusion review.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SustainedReviewFinding {
    /// Maintain the exclusion as-is.
    Maintain,
    /// Modify the scope (e.g., narrow from AllAffirming to SpecificCommunities).
    ModifyScope(ExclusionScope),
    /// Lift the exclusion entirely.
    Lift,
}

impl std::fmt::Display for SustainedReviewFinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Maintain => write!(f, "maintain"),
            Self::ModifyScope(scope) => write!(f, "modify_scope({scope})"),
            Self::Lift => write!(f, "lift"),
        }
    }
}

// ---------------------------------------------------------------------------
// Builder: SustainedExclusionRequest
// ---------------------------------------------------------------------------

/// Minimum number of community affirmations required.
const MIN_AFFIRMATIONS: usize = 3;

/// Minimum number of days evidence must span (anti-weaponization).
const MIN_EVIDENCE_SPAN_DAYS: i64 = 90;

/// Minimum number of exclusion-reinstatement-reoffense cycles for
/// RepeatedProtectiveExclusion basis.
const MIN_REPEATED_CYCLES: usize = 2;

/// Minimum independent communities for CrossCommunityPattern basis.
const MIN_CROSS_COMMUNITY_COUNT: usize = 3;

/// Builder for constructing and validating a sustained exclusion request.
///
/// # Example
///
/// ```
/// use jail::sustained_exclusion::*;
/// use uuid::Uuid;
/// use chrono::Utc;
///
/// let request = SustainedExclusionRequest::new("offender_pubkey")
///     .with_basis(SustainedExclusionBasis::RepeatedProtectiveExclusion { cycle_count: 2 })
///     .with_evidence(ExclusionEvidence {
///         source_type: EvidenceSource::ExclusionRecord,
///         source_id: Uuid::new_v4(),
///         community_id: "comm_a".into(),
///         summary: "First exclusion cycle".into(),
///         evidence_hashes: vec!["hash_1".into()],
///         submitted_at: Utc::now() - chrono::Duration::days(120),
///     })
///     .with_evidence(ExclusionEvidence {
///         source_type: EvidenceSource::ExclusionRecord,
///         source_id: Uuid::new_v4(),
///         community_id: "comm_b".into(),
///         summary: "Second exclusion cycle".into(),
///         evidence_hashes: vec!["hash_2".into()],
///         submitted_at: Utc::now(),
///     })
///     .with_affirmation(CommunityAffirmation {
///         community_id: "comm_a".into(),
///         decision_id: Uuid::new_v4(),
///         affirmed_at: Utc::now(),
///     })
///     .with_affirmation(CommunityAffirmation {
///         community_id: "comm_b".into(),
///         decision_id: Uuid::new_v4(),
///         affirmed_at: Utc::now(),
///     })
///     .with_affirmation(CommunityAffirmation {
///         community_id: "comm_c".into(),
///         decision_id: Uuid::new_v4(),
///         affirmed_at: Utc::now(),
///     })
///     .with_scope(ExclusionScope::AllAffirming);
///
/// assert!(request.validate().is_ok());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SustainedExclusionRequest {
    /// The person to be excluded.
    pub excluded_pubkey: String,
    /// The basis for exclusion.
    pub basis: Option<SustainedExclusionBasis>,
    /// Collected evidence.
    pub evidence: Vec<ExclusionEvidence>,
    /// Community affirmations.
    pub affirmations: Vec<CommunityAffirmation>,
    /// Requested scope.
    pub scope: Option<ExclusionScope>,
    /// Founding members of each affirming community — for independence checks.
    /// Maps community_id -> set of founding member pubkeys.
    pub community_founders: HashMap<String, HashSet<String>>,
}

impl SustainedExclusionRequest {
    /// Begin building a sustained exclusion request.
    pub fn new(excluded_pubkey: impl Into<String>) -> Self {
        Self {
            excluded_pubkey: excluded_pubkey.into(),
            basis: None,
            evidence: Vec::new(),
            affirmations: Vec::new(),
            scope: None,
            community_founders: HashMap::new(),
        }
    }

    /// Set the basis for exclusion.
    pub fn with_basis(mut self, basis: SustainedExclusionBasis) -> Self {
        self.basis = Some(basis);
        self
    }

    /// Add a piece of evidence.
    pub fn with_evidence(mut self, evidence: ExclusionEvidence) -> Self {
        self.evidence.push(evidence);
        self
    }

    /// Add a community affirmation.
    pub fn with_affirmation(mut self, affirmation: CommunityAffirmation) -> Self {
        self.affirmations.push(affirmation);
        self
    }

    /// Set the requested scope.
    pub fn with_scope(mut self, scope: ExclusionScope) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Register the founding members of a community (for independence validation).
    pub fn with_community_founders(
        mut self,
        community_id: impl Into<String>,
        founders: HashSet<String>,
    ) -> Self {
        self.community_founders.insert(community_id.into(), founders);
        self
    }

    /// Validate the request against all requirements.
    ///
    /// Checks:
    /// - Basis is set and meets its specific requirements
    /// - Minimum 3 community affirmations
    /// - Evidence spans at least 90 days
    /// - Community affirmations come from independent communities (no shared founders)
    /// - Scope is set and respects sovereign rights
    pub fn validate(&self) -> Result<(), JailError> {
        // Basis must be set
        let basis = self.basis.as_ref().ok_or_else(|| {
            JailError::SustainedExclusionInvalid("basis is required".into())
        })?;

        // Validate basis-specific requirements
        self.validate_basis(basis)?;

        // Minimum affirmations
        if self.affirmations.len() < MIN_AFFIRMATIONS {
            return Err(JailError::SustainedExclusionInvalid(format!(
                "requires at least {MIN_AFFIRMATIONS} community affirmations, have {}",
                self.affirmations.len()
            )));
        }

        // Evidence must span at least 90 days
        self.validate_evidence_span()?;

        // Community independence check (anti-weaponization)
        self.validate_community_independence()?;

        // Scope must be set
        let scope = self.scope.as_ref().ok_or_else(|| {
            JailError::SustainedExclusionInvalid("scope is required".into())
        })?;

        // Scope must respect sovereign rights
        if !scope.respects_sovereign_rights() {
            return Err(JailError::RightsViolation(
                "exclusion scope violates sovereign rights (Crown/Vault/Fortune/community creation)".into(),
            ));
        }

        Ok(())
    }

    /// Build the sustained exclusion after validation.
    ///
    /// Returns `Err` if validation fails.
    pub fn build(self) -> Result<SustainedExclusion, JailError> {
        self.validate()?;

        // Determine review type based on scope and basis
        let review_type = match (&self.scope, &self.basis) {
            (Some(ExclusionScope::AllKidsSphere), _) => SustainedReviewType::CovenantCourt,
            (_, Some(SustainedExclusionBasis::CrossCommunityPattern { .. })) => {
                SustainedReviewType::CovenantCourt
            }
            _ => SustainedReviewType::Standard,
        };

        Ok(SustainedExclusion {
            id: Uuid::new_v4(),
            excluded_pubkey: self.excluded_pubkey,
            basis: self.basis.expect("validated above"),
            evidence_chain: self.evidence,
            communities_affirming: self.affirmations,
            established_at: Utc::now(),
            scope: self.scope.expect("validated above"),
            review_schedule: SustainedReviewSchedule::new(365, review_type),
            reviews: Vec::new(),
            lifted_at: None,
            accused_rights: AccusedRights::always(),
        })
    }

    // --- Private validation helpers ---

    fn validate_basis(&self, basis: &SustainedExclusionBasis) -> Result<(), JailError> {
        match basis {
            SustainedExclusionBasis::RepeatedProtectiveExclusion { cycle_count } => {
                if *cycle_count < MIN_REPEATED_CYCLES {
                    return Err(JailError::SustainedExclusionInvalid(format!(
                        "RepeatedProtectiveExclusion requires at least {MIN_REPEATED_CYCLES} full cycles, have {cycle_count}"
                    )));
                }
            }
            SustainedExclusionBasis::CrossCommunityPattern { community_count } => {
                if *community_count < MIN_CROSS_COMMUNITY_COUNT {
                    return Err(JailError::SustainedExclusionInvalid(format!(
                        "CrossCommunityPattern requires at least {MIN_CROSS_COMMUNITY_COUNT} independent communities, have {community_count}"
                    )));
                }
            }
            SustainedExclusionBasis::AdjudicatedHarm { .. } => {
                // Severity is enforced by the type system — only Grave and Existential
                // exist in AdjudicatedSeverity.
            }
        }
        Ok(())
    }

    fn validate_evidence_span(&self) -> Result<(), JailError> {
        if self.evidence.len() < 2 {
            return Err(JailError::SustainedExclusionInvalid(
                "at least 2 pieces of evidence required".into(),
            ));
        }

        let earliest = self
            .evidence
            .iter()
            .map(|e| e.submitted_at)
            .min()
            .expect("checked len >= 2 above");
        let latest = self
            .evidence
            .iter()
            .map(|e| e.submitted_at)
            .max()
            .expect("checked len >= 2 above");

        let span = latest - earliest;
        if span < Duration::days(MIN_EVIDENCE_SPAN_DAYS) {
            return Err(JailError::SustainedExclusionInvalid(format!(
                "evidence must span at least {MIN_EVIDENCE_SPAN_DAYS} days, spans {} days",
                span.num_days()
            )));
        }

        Ok(())
    }

    fn validate_community_independence(&self) -> Result<(), JailError> {
        // If no founder data is provided, skip this check (callers may not
        // have founder data at request-build time; enforcement happens at
        // the governance layer). When founder data IS provided, verify
        // that no two affirming communities share founding members.
        if self.community_founders.is_empty() {
            return Ok(());
        }

        let affirming_ids: Vec<&str> = self
            .affirmations
            .iter()
            .map(|a| a.community_id.as_str())
            .collect();

        for (i, id_a) in affirming_ids.iter().enumerate() {
            for id_b in affirming_ids.iter().skip(i + 1) {
                if let (Some(founders_a), Some(founders_b)) = (
                    self.community_founders.get(*id_a),
                    self.community_founders.get(*id_b),
                ) {
                    let shared: Vec<&String> =
                        founders_a.intersection(founders_b).collect();
                    if !shared.is_empty() {
                        return Err(JailError::SustainedExclusionInvalid(format!(
                            "communities '{id_a}' and '{id_b}' share founding members: anti-weaponization check failed"
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Integration: escalate from GraduatedResponse
// ---------------------------------------------------------------------------

/// Minimum full cycles (exclusion -> review -> reinstatement -> reoffense)
/// before escalation to sustained exclusion is available.
const MIN_ESCALATION_CYCLES: usize = 1;

/// Check whether a graduated response history qualifies for escalation
/// to sustained exclusion.
///
/// Requires:
/// - Current level is ProtectiveExclusion
/// - At least one full cycle of exclusion -> de-escalation -> re-escalation
pub fn can_escalate_to_sustained(
    response: &crate::response::GraduatedResponse,
) -> bool {
    use crate::response::ResponseLevel;

    if response.current_level != ResponseLevel::ProtectiveExclusion {
        return false;
    }

    if response.resolved_at.is_some() {
        return false;
    }

    // Count full cycles: sequences where we went to ProtectiveExclusion,
    // then de-escalated (or resolved), then re-escalated to ProtectiveExclusion.
    let exclusion_entries: Vec<usize> = response
        .history
        .iter()
        .enumerate()
        .filter(|(_, r)| r.level == ResponseLevel::ProtectiveExclusion)
        .map(|(i, _)| i)
        .collect();

    // Each pair of ProtectiveExclusion entries with a lower level between them
    // constitutes a full cycle.
    let mut cycles = 0usize;
    for window in exclusion_entries.windows(2) {
        let between = &response.history[window[0] + 1..window[1]];
        if between.iter().any(|r| r.level < ResponseLevel::ProtectiveExclusion) {
            cycles += 1;
        }
    }

    cycles >= MIN_ESCALATION_CYCLES
}

/// Create a `SustainedExclusionRequest` from a `GraduatedResponse` that has
/// completed at least one full exclusion cycle.
///
/// Returns `Err` if the response is not eligible for escalation.
pub fn escalate_to_sustained(
    response: &crate::response::GraduatedResponse,
) -> Result<SustainedExclusionRequest, JailError> {
    if !can_escalate_to_sustained(response) {
        return Err(JailError::InvalidResponseTransition {
            from: response.current_level.to_string(),
            to: "sustained exclusion requires at least 1 full exclusion cycle".into(),
        });
    }

    Ok(SustainedExclusionRequest::new(&response.target_pubkey).with_basis(
        SustainedExclusionBasis::RepeatedProtectiveExclusion {
            cycle_count: count_exclusion_cycles(response),
        },
    ))
}

/// Count the number of full exclusion-reinstatement-reoffense cycles.
fn count_exclusion_cycles(response: &crate::response::GraduatedResponse) -> usize {
    use crate::response::ResponseLevel;

    let exclusion_entries: Vec<usize> = response
        .history
        .iter()
        .enumerate()
        .filter(|(_, r)| r.level == ResponseLevel::ProtectiveExclusion)
        .map(|(i, _)| i)
        .collect();

    let mut cycles = 0usize;
    for window in exclusion_entries.windows(2) {
        let between = &response.history[window[0] + 1..window[1]];
        if between
            .iter()
            .any(|r| r.level < ResponseLevel::ProtectiveExclusion)
        {
            cycles += 1;
        }
    }

    cycles
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::response::GraduatedResponse;

    // --- Helper factories ---

    fn make_evidence(community: &str, days_ago: i64) -> ExclusionEvidence {
        ExclusionEvidence {
            source_type: EvidenceSource::ExclusionRecord,
            source_id: Uuid::new_v4(),
            community_id: community.into(),
            summary: format!("Evidence from {community}"),
            evidence_hashes: vec!["hash_1".into()],
            submitted_at: Utc::now() - Duration::days(days_ago),
        }
    }

    fn make_affirmation(community: &str) -> CommunityAffirmation {
        CommunityAffirmation {
            community_id: community.into(),
            decision_id: Uuid::new_v4(),
            affirmed_at: Utc::now(),
        }
    }

    fn valid_request() -> SustainedExclusionRequest {
        SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::RepeatedProtectiveExclusion { cycle_count: 2 })
            .with_evidence(make_evidence("comm_a", 120))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"))
            .with_scope(ExclusionScope::AllAffirming)
    }

    /// Build a GraduatedResponse that has gone through N full exclusion cycles.
    fn response_with_cycles(cycles: usize) -> GraduatedResponse {
        let mut resp = GraduatedResponse::begin("offender", "initial", "moderator");
        // First, escalate to ProtectiveExclusion
        resp.escalate("escalating", "moderator").unwrap();
        resp.escalate("escalating", "moderator").unwrap();
        resp.escalate("escalating", "moderator").unwrap();
        resp.escalate("escalating", "moderator").unwrap();
        for _ in 0..cycles {
            // De-escalate (reinstatement)
            resp.de_escalate("improvement shown", "moderator").unwrap();
            // Re-escalate back to ProtectiveExclusion (reoffense)
            resp.escalate("reoffended", "moderator").unwrap();
        }
        resp
    }

    // --- Basis validation ---

    #[test]
    fn basis_repeated_requires_min_cycles() {
        let request = SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::RepeatedProtectiveExclusion { cycle_count: 1 })
            .with_evidence(make_evidence("comm_a", 120))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"))
            .with_scope(ExclusionScope::AllAffirming);
        let err = request.validate().unwrap_err();
        assert!(err.to_string().contains("2 full cycles"));
    }

    #[test]
    fn basis_repeated_accepts_min_cycles() {
        let request = valid_request();
        assert!(request.validate().is_ok());
    }

    #[test]
    fn basis_cross_community_requires_min_communities() {
        let request = SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::CrossCommunityPattern { community_count: 2 })
            .with_evidence(make_evidence("comm_a", 120))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"))
            .with_scope(ExclusionScope::AllAffirming);
        let err = request.validate().unwrap_err();
        assert!(err.to_string().contains("3 independent communities"));
    }

    #[test]
    fn basis_cross_community_accepts_min() {
        let request = SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::CrossCommunityPattern { community_count: 3 })
            .with_evidence(make_evidence("comm_a", 120))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"))
            .with_scope(ExclusionScope::AllAffirming);
        assert!(request.validate().is_ok());
    }

    #[test]
    fn basis_adjudicated_harm_grave() {
        let request = SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::AdjudicatedHarm {
                severity: AdjudicatedSeverity::Grave,
                dispute_id: Uuid::new_v4(),
            })
            .with_evidence(make_evidence("comm_a", 120))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"))
            .with_scope(ExclusionScope::AllAffirming);
        assert!(request.validate().is_ok());
    }

    #[test]
    fn basis_adjudicated_harm_existential() {
        let request = SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::AdjudicatedHarm {
                severity: AdjudicatedSeverity::Existential,
                dispute_id: Uuid::new_v4(),
            })
            .with_evidence(make_evidence("comm_a", 120))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"))
            .with_scope(ExclusionScope::AllAffirming);
        assert!(request.validate().is_ok());
    }

    #[test]
    fn adjudicated_severity_ordering() {
        assert!(AdjudicatedSeverity::Grave < AdjudicatedSeverity::Existential);
    }

    // --- Affirmation validation ---

    #[test]
    fn rejects_insufficient_affirmations() {
        let request = SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::RepeatedProtectiveExclusion { cycle_count: 2 })
            .with_evidence(make_evidence("comm_a", 120))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_scope(ExclusionScope::AllAffirming);
        let err = request.validate().unwrap_err();
        assert!(err.to_string().contains("3 community affirmations"));
    }

    // --- Evidence span validation ---

    #[test]
    fn rejects_insufficient_evidence_span() {
        let request = SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::RepeatedProtectiveExclusion { cycle_count: 2 })
            .with_evidence(make_evidence("comm_a", 30))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"))
            .with_scope(ExclusionScope::AllAffirming);
        let err = request.validate().unwrap_err();
        assert!(err.to_string().contains("90 days"));
    }

    #[test]
    fn rejects_single_evidence() {
        let request = SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::RepeatedProtectiveExclusion { cycle_count: 2 })
            .with_evidence(make_evidence("comm_a", 120))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"))
            .with_scope(ExclusionScope::AllAffirming);
        let err = request.validate().unwrap_err();
        assert!(err.to_string().contains("2 pieces of evidence"));
    }

    // --- Anti-weaponization: community independence ---

    #[test]
    fn rejects_shared_founding_members() {
        let mut founders_a = HashSet::new();
        founders_a.insert("alice".to_string());
        founders_a.insert("bob".to_string());

        let mut founders_b = HashSet::new();
        founders_b.insert("bob".to_string()); // shared with comm_a
        founders_b.insert("carol".to_string());

        let founders_c = HashSet::from(["dave".to_string(), "eve".to_string()]);

        let request = SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::RepeatedProtectiveExclusion { cycle_count: 2 })
            .with_evidence(make_evidence("comm_a", 120))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"))
            .with_community_founders("comm_a", founders_a)
            .with_community_founders("comm_b", founders_b)
            .with_community_founders("comm_c", founders_c)
            .with_scope(ExclusionScope::AllAffirming);

        let err = request.validate().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("shared founding members") || msg.contains("share founding members"),
            "expected shared founding members error, got: {msg}");
    }

    #[test]
    fn accepts_independent_communities() {
        let founders_a = HashSet::from(["alice".to_string(), "bob".to_string()]);
        let founders_b = HashSet::from(["carol".to_string(), "dave".to_string()]);
        let founders_c = HashSet::from(["eve".to_string(), "frank".to_string()]);

        let request = SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::RepeatedProtectiveExclusion { cycle_count: 2 })
            .with_evidence(make_evidence("comm_a", 120))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"))
            .with_community_founders("comm_a", founders_a)
            .with_community_founders("comm_b", founders_b)
            .with_community_founders("comm_c", founders_c)
            .with_scope(ExclusionScope::AllAffirming);

        assert!(request.validate().is_ok());
    }

    #[test]
    fn skips_independence_check_without_founder_data() {
        // When no founder data is provided, the check is deferred to the governance layer.
        let request = valid_request();
        assert!(request.validate().is_ok());
    }

    // --- Scope enforcement ---

    #[test]
    fn scope_specific_communities_respects_sovereign_rights() {
        let scope = ExclusionScope::SpecificCommunities(vec!["comm_a".into()]);
        assert!(scope.respects_sovereign_rights());
    }

    #[test]
    fn scope_all_kidssphere_respects_sovereign_rights() {
        assert!(ExclusionScope::AllKidsSphere.respects_sovereign_rights());
    }

    #[test]
    fn scope_all_affirming_respects_sovereign_rights() {
        assert!(ExclusionScope::AllAffirming.respects_sovereign_rights());
    }

    #[test]
    fn inalienable_rights_always_listed() {
        let rights = SustainedExclusion::inalienable_rights();
        assert_eq!(rights.len(), 4);
        assert!(rights.iter().any(|r| r.contains("Crown")));
        assert!(rights.iter().any(|r| r.contains("Vault")));
        assert!(rights.iter().any(|r| r.contains("Fortune")));
        assert!(rights.iter().any(|r| r.contains("community")));
    }

    // --- Review scheduling ---

    #[test]
    fn review_schedule_default_annual() {
        let schedule = SustainedReviewSchedule::annual();
        assert_eq!(schedule.review_interval_days, 365);
        assert_eq!(schedule.reviews_completed, 0);
        assert_eq!(schedule.review_type, SustainedReviewType::Standard);
        assert!(!schedule.is_overdue());
    }

    #[test]
    fn review_schedule_advance() {
        let mut schedule = SustainedReviewSchedule::annual();
        assert_eq!(schedule.reviews_completed, 0);
        schedule.advance();
        assert_eq!(schedule.reviews_completed, 1);
        assert!(!schedule.is_overdue());
    }

    #[test]
    fn review_schedule_covenant_court_for_kidssphere() {
        let request = SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::RepeatedProtectiveExclusion { cycle_count: 2 })
            .with_evidence(make_evidence("comm_a", 120))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"))
            .with_scope(ExclusionScope::AllKidsSphere);

        let exclusion = request.build().unwrap();
        assert_eq!(
            exclusion.review_schedule.review_type,
            SustainedReviewType::CovenantCourt
        );
    }

    #[test]
    fn review_schedule_covenant_court_for_cross_community() {
        let request = SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::CrossCommunityPattern { community_count: 3 })
            .with_evidence(make_evidence("comm_a", 120))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"))
            .with_scope(ExclusionScope::AllAffirming);

        let exclusion = request.build().unwrap();
        assert_eq!(
            exclusion.review_schedule.review_type,
            SustainedReviewType::CovenantCourt
        );
    }

    #[test]
    fn review_schedule_standard_for_repeated() {
        let request = SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::RepeatedProtectiveExclusion { cycle_count: 2 })
            .with_evidence(make_evidence("comm_a", 120))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"))
            .with_scope(ExclusionScope::SpecificCommunities(vec!["comm_a".into()]));

        let exclusion = request.build().unwrap();
        assert_eq!(
            exclusion.review_schedule.review_type,
            SustainedReviewType::Standard
        );
    }

    // --- Review recording ---

    #[test]
    fn review_maintain_keeps_exclusion_active() {
        let mut exclusion = valid_request().build().unwrap();
        assert!(exclusion.is_active());

        exclusion.record_review(SustainedReview {
            id: Uuid::new_v4(),
            review_type: SustainedReviewType::Standard,
            panel: vec!["judge_a".into(), "judge_b".into()],
            finding: SustainedReviewFinding::Maintain,
            reasoning: "Safety concern persists".into(),
            reviewed_at: Utc::now(),
        });

        assert!(exclusion.is_active());
        assert_eq!(exclusion.reviews.len(), 1);
        assert_eq!(exclusion.review_schedule.reviews_completed, 1);
    }

    #[test]
    fn review_lift_deactivates_exclusion() {
        let mut exclusion = valid_request().build().unwrap();
        assert!(exclusion.is_active());

        let review_time = Utc::now();
        exclusion.record_review(SustainedReview {
            id: Uuid::new_v4(),
            review_type: SustainedReviewType::Standard,
            panel: vec!["judge_a".into()],
            finding: SustainedReviewFinding::Lift,
            reasoning: "Conditions met, safe to return".into(),
            reviewed_at: review_time,
        });

        assert!(!exclusion.is_active());
        assert_eq!(exclusion.lifted_at, Some(review_time));
    }

    #[test]
    fn review_modify_scope_changes_scope() {
        let mut exclusion = valid_request().build().unwrap();
        let original_scope = exclusion.scope.clone();
        assert_eq!(original_scope, ExclusionScope::AllAffirming);

        let new_scope = ExclusionScope::SpecificCommunities(vec!["comm_a".into()]);
        exclusion.record_review(SustainedReview {
            id: Uuid::new_v4(),
            review_type: SustainedReviewType::Standard,
            panel: vec!["judge_a".into()],
            finding: SustainedReviewFinding::ModifyScope(new_scope.clone()),
            reasoning: "Narrowing scope".into(),
            reviewed_at: Utc::now(),
        });

        assert!(exclusion.is_active());
        assert_eq!(exclusion.scope, new_scope);
    }

    // --- Escalation from GraduatedResponse ---

    #[test]
    fn cannot_escalate_without_exclusion_level() {
        let resp = GraduatedResponse::begin("offender", "initial", "moderator");
        assert!(!can_escalate_to_sustained(&resp));
        assert!(escalate_to_sustained(&resp).is_err());
    }

    #[test]
    fn cannot_escalate_without_full_cycle() {
        let resp = response_with_cycles(0);
        assert!(!can_escalate_to_sustained(&resp));
    }

    #[test]
    fn can_escalate_with_one_cycle() {
        let resp = response_with_cycles(1);
        assert!(can_escalate_to_sustained(&resp));
    }

    #[test]
    fn escalation_produces_correct_basis() {
        let resp = response_with_cycles(2);
        let request = escalate_to_sustained(&resp).unwrap();
        assert_eq!(
            request.basis,
            Some(SustainedExclusionBasis::RepeatedProtectiveExclusion { cycle_count: 2 })
        );
        assert_eq!(request.excluded_pubkey, "offender");
    }

    #[test]
    fn cannot_escalate_resolved_response() {
        let mut resp = response_with_cycles(1);
        resp.resolve("matter closed");
        assert!(!can_escalate_to_sustained(&resp));
    }

    // --- AccusedRights always on ---

    #[test]
    fn accused_rights_always_on_in_sustained_exclusion() {
        let exclusion = valid_request().build().unwrap();
        assert!(exclusion.accused_rights.validate());
        assert!(exclusion.validate_rights());
    }

    // --- Build validation ---

    #[test]
    fn build_fails_without_basis() {
        let request = SustainedExclusionRequest::new("offender")
            .with_evidence(make_evidence("comm_a", 120))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"))
            .with_scope(ExclusionScope::AllAffirming);
        let err = request.build().unwrap_err();
        assert!(err.to_string().contains("basis is required"));
    }

    #[test]
    fn build_fails_without_scope() {
        let request = SustainedExclusionRequest::new("offender")
            .with_basis(SustainedExclusionBasis::RepeatedProtectiveExclusion { cycle_count: 2 })
            .with_evidence(make_evidence("comm_a", 120))
            .with_evidence(make_evidence("comm_b", 10))
            .with_affirmation(make_affirmation("comm_a"))
            .with_affirmation(make_affirmation("comm_b"))
            .with_affirmation(make_affirmation("comm_c"));
        let err = request.build().unwrap_err();
        assert!(err.to_string().contains("scope is required"));
    }

    #[test]
    fn build_succeeds_with_valid_request() {
        let exclusion = valid_request().build().unwrap();
        assert!(exclusion.is_active());
        assert!(exclusion.validate_scope());
        assert!(exclusion.validate_rights());
        assert_eq!(exclusion.excluded_pubkey, "offender");
        assert_eq!(exclusion.evidence_chain.len(), 2);
        assert_eq!(exclusion.communities_affirming.len(), 3);
        assert_eq!(exclusion.review_schedule.review_interval_days, 365);
        assert!(exclusion.reviews.is_empty());
        assert!(exclusion.lifted_at.is_none());
    }

    // --- Serialization ---

    #[test]
    fn sustained_exclusion_serialization_roundtrip() {
        let exclusion = valid_request().build().unwrap();
        let json = serde_json::to_string(&exclusion).unwrap();
        let deserialized: SustainedExclusion = serde_json::from_str(&json).unwrap();
        assert_eq!(exclusion, deserialized);
    }

    #[test]
    fn request_serialization_roundtrip() {
        let request = valid_request();
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: SustainedExclusionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, deserialized);
    }

    #[test]
    fn basis_display() {
        let basis = SustainedExclusionBasis::RepeatedProtectiveExclusion { cycle_count: 3 };
        assert!(basis.to_string().contains("3 cycles"));

        let basis = SustainedExclusionBasis::CrossCommunityPattern { community_count: 5 };
        assert!(basis.to_string().contains("5 communities"));

        let basis = SustainedExclusionBasis::AdjudicatedHarm {
            severity: AdjudicatedSeverity::Grave,
            dispute_id: Uuid::new_v4(),
        };
        assert!(basis.to_string().contains("grave"));
    }

    #[test]
    fn scope_display() {
        let scope = ExclusionScope::SpecificCommunities(vec!["a".into(), "b".into()]);
        assert!(scope.to_string().contains("2"));

        assert_eq!(ExclusionScope::AllKidsSphere.to_string(), "all_kidssphere");
        assert_eq!(ExclusionScope::AllAffirming.to_string(), "all_affirming");
    }

    #[test]
    fn scope_description() {
        let scope = ExclusionScope::SpecificCommunities(vec!["a".into(), "b".into()]);
        assert!(scope.description().contains("2 specific"));

        assert!(ExclusionScope::AllKidsSphere.description().contains("KidsSphere"));
        assert!(ExclusionScope::AllAffirming.description().contains("affirming"));
    }

    #[test]
    fn evidence_source_display() {
        assert_eq!(EvidenceSource::Flag.to_string(), "flag");
        assert_eq!(EvidenceSource::Dispute.to_string(), "dispute");
        assert_eq!(EvidenceSource::ExclusionRecord.to_string(), "exclusion_record");
        assert_eq!(EvidenceSource::CrossCommunityReport.to_string(), "cross_community_report");
    }

    #[test]
    fn review_type_display() {
        assert_eq!(SustainedReviewType::Standard.to_string(), "standard");
        assert_eq!(SustainedReviewType::CovenantCourt.to_string(), "covenant_court");
    }

    #[test]
    fn review_finding_display() {
        assert_eq!(SustainedReviewFinding::Maintain.to_string(), "maintain");
        assert_eq!(SustainedReviewFinding::Lift.to_string(), "lift");
        let scope = ExclusionScope::AllKidsSphere;
        let finding = SustainedReviewFinding::ModifyScope(scope);
        assert!(finding.to_string().contains("all_kidssphere"));
    }
}
