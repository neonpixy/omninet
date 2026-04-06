//! # Covenant Court — Cross-Community Constitutional Interpretation
//!
//! The Star Court: a body for disputes about what the Covenant means. Not a
//! supreme court with enforcement power — an interpretation body whose decisions
//! become Layer 3 precedent.
//!
//! From the Covenant's design: coercive central authority is what the Covenant
//! exists to prevent. The Court interprets. Communities implement. If a community
//! ignores a Court interpretation, the precedent still exists for other communities
//! to adopt. The Court has moral authority, not coercive power.
//!
//! Dissents are preserved, never hidden. Minority interpretations matter.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::KingdomError;

// ---------------------------------------------------------------------------
// Core enums
// ---------------------------------------------------------------------------

/// What the Court may hear.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CourtJurisdiction {
    /// Dispute between two or more communities about Covenant meaning.
    InterCommunityDispute,
    /// A question about how a Covenant article should be interpreted.
    CovenantInterpretation,
    /// Review of a proposed Covenant amendment for axiom compatibility.
    AmendmentReview,
    /// Appeal of an exclusion decision (from R2A sustained exclusion).
    ExclusionAppeal,
}

/// How adjudicators are chosen for a given court.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AdjudicatorSelection {
    /// Random selection from a qualified pool.
    RandomFromPool(AdjudicatorPool),
    /// Rotating panel of a fixed size, drawn from the pool in order.
    RotatingPanel(usize),
    /// Both parties nominate adjudicators directly.
    NominatedByParties,
}

/// Role a party plays in a case.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PartyRole {
    /// The party that files the case.
    Petitioner,
    /// The party the case is filed against.
    Respondent,
    /// A third party with a direct stake in the outcome.
    Intervenor,
    /// A friend of the court — provides perspective without being a party.
    AmicusCuriae,
}

/// Lifecycle of a court case.
///
/// State machine: Filed -> Accepted -> Hearing -> Deliberation -> Decided -> PrecedentRecorded
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CourtCaseStatus {
    /// Case has been filed but not yet accepted by the court.
    Filed,
    /// Court has accepted the case and is assembling adjudicators.
    Accepted,
    /// Submissions are being heard.
    Hearing,
    /// Adjudicators are deliberating.
    Deliberation,
    /// A decision has been rendered.
    Decided,
    /// The decision has been recorded as Layer 3 precedent.
    PrecedentRecorded,
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// Pool of eligible adjudicators with diversity and conflict-of-interest rules.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdjudicatorPool {
    /// Public keys of people eligible to serve.
    pub eligible_pubkeys: Vec<String>,
    /// Minimum number of distinct communities adjudicators must come from.
    pub community_diversity_min: usize,
    /// Public keys with known conflicts of interest, excluded from selection.
    pub exclusions: Vec<String>,
    /// Maximum time an adjudicator may serve before rotation, in seconds.
    pub tenure_limit_secs: Option<i64>,
}

impl AdjudicatorPool {
    pub fn new(community_diversity_min: usize) -> Self {
        Self {
            eligible_pubkeys: Vec::new(),
            community_diversity_min,
            exclusions: Vec::new(),
            tenure_limit_secs: None,
        }
    }

    /// Add an eligible pubkey to the pool.
    pub fn add_eligible(&mut self, pubkey: impl Into<String>) {
        self.eligible_pubkeys.push(pubkey.into());
    }

    /// Mark a pubkey as excluded (conflict of interest).
    pub fn exclude(&mut self, pubkey: impl Into<String>) {
        self.exclusions.push(pubkey.into());
    }

    /// Set a tenure limit for adjudicators drawn from this pool.
    ///
    /// `limit_secs` is the maximum service duration in seconds.
    pub fn with_tenure_limit_secs(mut self, limit_secs: i64) -> Self {
        self.tenure_limit_secs = Some(limit_secs);
        self
    }

    /// Check whether a pubkey is eligible (in pool and not excluded).
    pub fn is_eligible(&self, pubkey: &str) -> bool {
        self.eligible_pubkeys.contains(&pubkey.to_string())
            && !self.exclusions.contains(&pubkey.to_string())
    }
}

/// An adjudicator seated on a court.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CourtAdjudicator {
    /// Public key of the adjudicator.
    pub pubkey: String,
    /// Community the adjudicator belongs to.
    pub community_id: String,
    /// When this adjudicator was selected for service.
    pub selected_at: DateTime<Utc>,
    /// Number of cases this adjudicator has heard.
    pub cases_heard: usize,
    /// Case IDs this adjudicator recused themselves from.
    pub recusal_record: Vec<Uuid>,
}

impl CourtAdjudicator {
    pub fn new(pubkey: impl Into<String>, community_id: impl Into<String>) -> Self {
        Self {
            pubkey: pubkey.into(),
            community_id: community_id.into(),
            selected_at: Utc::now(),
            cases_heard: 0,
            recusal_record: Vec::new(),
        }
    }

    /// Check if this adjudicator has a conflict of interest with a case.
    ///
    /// An adjudicator must recuse if they are a member of either party's community.
    pub fn has_conflict(&self, case: &CourtCase) -> bool {
        if self.community_id == case.petitioner.community_id {
            return true;
        }
        if let Some(ref respondent) = case.respondent {
            if self.community_id == respondent.community_id {
                return true;
            }
        }
        false
    }

    /// Record a recusal from a specific case.
    pub fn recuse(&mut self, case_id: Uuid) {
        if !self.recusal_record.contains(&case_id) {
            self.recusal_record.push(case_id);
        }
    }

    /// Increment the count of cases heard.
    pub fn record_case_heard(&mut self) {
        self.cases_heard += 1;
    }
}

/// A party to a court case.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CourtParty {
    /// Public key of the party.
    pub pubkey: String,
    /// Community the party belongs to.
    pub community_id: String,
    /// Role in this case.
    pub role: PartyRole,
}

impl CourtParty {
    pub fn new(
        pubkey: impl Into<String>,
        community_id: impl Into<String>,
        role: PartyRole,
    ) -> Self {
        Self {
            pubkey: pubkey.into(),
            community_id: community_id.into(),
            role,
        }
    }
}

/// A written submission to the court from a party.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CourtSubmission {
    /// The party making this submission.
    pub party: CourtParty,
    /// The content of the submission.
    pub content: String,
    /// Hashes of evidence referenced in the submission.
    pub evidence_hashes: Vec<String>,
    /// When the submission was filed.
    pub submitted_at: DateTime<Utc>,
}

impl CourtSubmission {
    pub fn new(party: CourtParty, content: impl Into<String>) -> Self {
        Self {
            party,
            content: content.into(),
            evidence_hashes: Vec::new(),
            submitted_at: Utc::now(),
        }
    }

    /// Attach evidence hashes to the submission.
    pub fn with_evidence(mut self, hashes: Vec<String>) -> Self {
        self.evidence_hashes = hashes;
        self
    }
}

/// The court's decision on a case.
///
/// Dissents are preserved, never hidden. Minority interpretations matter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CourtDecision {
    /// Brief summary of the decision.
    pub summary: String,
    /// Full reasoning behind the decision.
    pub reasoning: String,
    /// The actual constitutional interpretation — the binding precedent text.
    pub interpretation: String,
    /// Whether the decision was unanimous.
    pub unanimous: bool,
    /// Dissenting opinions. Preserved as a matter of principle.
    pub dissents: Vec<CourtDissent>,
    /// If recorded as precedent, the ID linking to Polity's PrecedentRegistry.
    pub precedent_id: Option<Uuid>,
    /// When the decision was rendered.
    pub decided_at: DateTime<Utc>,
}

impl CourtDecision {
    pub fn new(
        summary: impl Into<String>,
        reasoning: impl Into<String>,
        interpretation: impl Into<String>,
    ) -> Self {
        Self {
            summary: summary.into(),
            reasoning: reasoning.into(),
            interpretation: interpretation.into(),
            unanimous: true,
            dissents: Vec::new(),
            precedent_id: None,
            decided_at: Utc::now(),
        }
    }

    /// Add a dissenting opinion. Automatically marks the decision as non-unanimous.
    pub fn add_dissent(&mut self, dissent: CourtDissent) {
        self.dissents.push(dissent);
        self.unanimous = false;
    }

    /// Record this decision as Layer 3 precedent.
    pub fn record_as_precedent(&mut self) -> Uuid {
        let id = Uuid::new_v4();
        self.precedent_id = Some(id);
        id
    }
}

/// A dissenting opinion from an adjudicator who disagrees with the majority.
///
/// Dissents are never hidden. They represent legitimate alternative interpretations
/// of the Covenant that future courts may draw upon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CourtDissent {
    /// Public key of the dissenting adjudicator.
    pub adjudicator_pubkey: String,
    /// The reasoning behind the dissent.
    pub reasoning: String,
}

impl CourtDissent {
    pub fn new(adjudicator_pubkey: impl Into<String>, reasoning: impl Into<String>) -> Self {
        Self {
            adjudicator_pubkey: adjudicator_pubkey.into(),
            reasoning: reasoning.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// CourtCase — the central type
// ---------------------------------------------------------------------------

/// A case before the Covenant Court.
///
/// Cases follow a strict lifecycle: Filed -> Accepted -> Hearing ->
/// Deliberation -> Decided -> PrecedentRecorded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CourtCase {
    pub id: Uuid,
    /// What category of question this case addresses.
    pub jurisdiction: CourtJurisdiction,
    /// The party that filed the case.
    pub petitioner: CourtParty,
    /// The party the case is filed against (if applicable).
    pub respondent: Option<CourtParty>,
    /// The constitutional question being asked.
    pub question: String,
    /// Written submissions from all parties.
    pub submissions: Vec<CourtSubmission>,
    /// Current status in the lifecycle.
    pub status: CourtCaseStatus,
    /// The court's decision, once rendered.
    pub decision: Option<CourtDecision>,
    /// When the case was filed.
    pub filed_at: DateTime<Utc>,
}

impl CourtCase {
    /// File a new case. It starts in `Filed` status.
    pub fn file(
        jurisdiction: CourtJurisdiction,
        petitioner: CourtParty,
        question: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            jurisdiction,
            petitioner,
            respondent: None,
            question: question.into(),
            submissions: Vec::new(),
            status: CourtCaseStatus::Filed,
            decision: None,
            filed_at: Utc::now(),
        }
    }

    /// File a case against a specific respondent.
    pub fn file_against(
        jurisdiction: CourtJurisdiction,
        petitioner: CourtParty,
        respondent: CourtParty,
        question: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            jurisdiction,
            petitioner,
            respondent: Some(respondent),
            question: question.into(),
            submissions: Vec::new(),
            status: CourtCaseStatus::Filed,
            decision: None,
            filed_at: Utc::now(),
        }
    }

    /// Accept the case for hearing.
    pub fn accept(&mut self) -> Result<(), KingdomError> {
        if self.status != CourtCaseStatus::Filed {
            return Err(KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Accepted".into(),
            });
        }
        self.status = CourtCaseStatus::Accepted;
        Ok(())
    }

    /// Move to hearing phase (accepting submissions).
    pub fn begin_hearing(&mut self) -> Result<(), KingdomError> {
        if self.status != CourtCaseStatus::Accepted {
            return Err(KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Hearing".into(),
            });
        }
        self.status = CourtCaseStatus::Hearing;
        Ok(())
    }

    /// Add a submission during the hearing phase.
    pub fn add_submission(&mut self, submission: CourtSubmission) -> Result<(), KingdomError> {
        if self.status != CourtCaseStatus::Hearing {
            return Err(KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Hearing (for submission)".into(),
            });
        }
        self.submissions.push(submission);
        Ok(())
    }

    /// Close submissions and begin deliberation.
    pub fn begin_deliberation(&mut self) -> Result<(), KingdomError> {
        if self.status != CourtCaseStatus::Hearing {
            return Err(KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Deliberation".into(),
            });
        }
        self.status = CourtCaseStatus::Deliberation;
        Ok(())
    }

    /// Render a decision. Moves status to `Decided`.
    pub fn decide(&mut self, decision: CourtDecision) -> Result<(), KingdomError> {
        if self.status != CourtCaseStatus::Deliberation {
            return Err(KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Decided".into(),
            });
        }
        self.decision = Some(decision);
        self.status = CourtCaseStatus::Decided;
        Ok(())
    }

    /// Record the decision as Layer 3 precedent. Final state.
    pub fn record_precedent(&mut self) -> Result<Uuid, KingdomError> {
        if self.status != CourtCaseStatus::Decided {
            return Err(KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "PrecedentRecorded".into(),
            });
        }
        let decision = self.decision.as_mut().ok_or_else(|| {
            KingdomError::InvalidTransition {
                current: "Decided (no decision)".into(),
                target: "PrecedentRecorded".into(),
            }
        })?;
        let precedent_id = decision.record_as_precedent();
        self.status = CourtCaseStatus::PrecedentRecorded;
        Ok(precedent_id)
    }

    /// Whether the case is still in progress (not yet decided or recorded).
    pub fn is_active(&self) -> bool {
        !matches!(
            self.status,
            CourtCaseStatus::Decided | CourtCaseStatus::PrecedentRecorded
        )
    }

    /// All community IDs involved in the case (petitioner + respondent).
    pub fn involved_community_ids(&self) -> Vec<&str> {
        let mut ids = vec![self.petitioner.community_id.as_str()];
        if let Some(ref r) = self.respondent {
            ids.push(r.community_id.as_str());
        }
        ids
    }
}

// ---------------------------------------------------------------------------
// CovenantCourt — the court itself
// ---------------------------------------------------------------------------

/// The Covenant Court — a cross-community body for interpreting the Covenant.
///
/// No enforcement power. Interpretation only. Decisions become Layer 3 precedent
/// through Polity's PrecedentRegistry. Communities implement on their own terms.
/// If a community ignores a Court interpretation, the precedent still exists for
/// other communities to adopt.
///
/// This is by design — coercive central authority is what the Covenant exists
/// to prevent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CovenantCourt {
    pub id: Uuid,
    /// What categories of questions this court may hear.
    pub jurisdiction: CourtJurisdiction,
    /// Currently seated adjudicators.
    pub adjudicators: Vec<CourtAdjudicator>,
    /// Cases filed with this court.
    pub cases: Vec<CourtCase>,
    /// How adjudicators are selected for this court.
    pub selection_method: AdjudicatorSelection,
}

impl CovenantCourt {
    /// Create a new court with a given jurisdiction and selection method.
    pub fn new(jurisdiction: CourtJurisdiction, selection_method: AdjudicatorSelection) -> Self {
        Self {
            id: Uuid::new_v4(),
            jurisdiction,
            adjudicators: Vec::new(),
            cases: Vec::new(),
            selection_method,
        }
    }

    /// Seat an adjudicator on the court.
    ///
    /// Returns an error if the adjudicator is not eligible per the selection method
    /// (when using `RandomFromPool`).
    pub fn seat_adjudicator(
        &mut self,
        adjudicator: CourtAdjudicator,
    ) -> Result<(), KingdomError> {
        // If selection is pool-based, verify eligibility.
        if let AdjudicatorSelection::RandomFromPool(ref pool) = self.selection_method {
            if !pool.is_eligible(&adjudicator.pubkey) {
                return Err(KingdomError::AdjudicatorNotAvailable(
                    adjudicator.pubkey.clone(),
                ));
            }
        }
        self.adjudicators.push(adjudicator);
        Ok(())
    }

    /// Check whether the currently seated adjudicators satisfy the community
    /// diversity requirement (when using `RandomFromPool`).
    pub fn meets_diversity_requirement(&self) -> bool {
        match self.selection_method {
            AdjudicatorSelection::RandomFromPool(ref pool) => {
                let unique_communities: std::collections::HashSet<&str> = self
                    .adjudicators
                    .iter()
                    .map(|a| a.community_id.as_str())
                    .collect();
                unique_communities.len() >= pool.community_diversity_min
            }
            // Rotating panels and nominated adjudicators have their own diversity
            // guarantees outside this check.
            AdjudicatorSelection::RotatingPanel(_) | AdjudicatorSelection::NominatedByParties => {
                true
            }
        }
    }

    /// File a case with this court. Validates jurisdiction match.
    pub fn file_case(&mut self, case: CourtCase) -> Result<Uuid, KingdomError> {
        if case.jurisdiction != self.jurisdiction {
            return Err(KingdomError::InvalidTransition {
                current: format!("{:?}", case.jurisdiction),
                target: format!("{:?} (court jurisdiction)", self.jurisdiction),
            });
        }
        let id = case.id;
        self.cases.push(case);
        Ok(id)
    }

    /// Get a reference to a case by ID.
    pub fn get_case(&self, case_id: Uuid) -> Option<&CourtCase> {
        self.cases.iter().find(|c| c.id == case_id)
    }

    /// Get a mutable reference to a case by ID.
    pub fn get_case_mut(&mut self, case_id: Uuid) -> Option<&mut CourtCase> {
        self.cases.iter_mut().find(|c| c.id == case_id)
    }

    /// Find adjudicators who have no conflict of interest with a given case.
    pub fn eligible_adjudicators_for_case(&self, case: &CourtCase) -> Vec<&CourtAdjudicator> {
        self.adjudicators
            .iter()
            .filter(|a| !a.has_conflict(case))
            .collect()
    }

    /// Validate that enough conflict-free adjudicators from diverse communities
    /// are available to hear a case. Returns the eligible adjudicators if
    /// the diversity minimum is met, or an error if not.
    pub fn validate_panel_for_case(
        &self,
        case: &CourtCase,
    ) -> Result<Vec<&CourtAdjudicator>, KingdomError> {
        let eligible = self.eligible_adjudicators_for_case(case);

        if let AdjudicatorSelection::RandomFromPool(ref pool) = self.selection_method {
            let unique_communities: std::collections::HashSet<&str> =
                eligible.iter().map(|a| a.community_id.as_str()).collect();
            if unique_communities.len() < pool.community_diversity_min {
                return Err(KingdomError::InvalidTransition {
                    current: format!(
                        "{} unique communities among eligible adjudicators",
                        unique_communities.len()
                    ),
                    target: format!(
                        "minimum {} required",
                        pool.community_diversity_min
                    ),
                });
            }
        }

        Ok(eligible)
    }

    /// All precedent IDs produced by this court's decided cases.
    pub fn precedent_ids(&self) -> Vec<Uuid> {
        self.cases
            .iter()
            .filter_map(|c| c.decision.as_ref())
            .filter_map(|d| d.precedent_id)
            .collect()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_pool() -> AdjudicatorPool {
        let mut pool = AdjudicatorPool::new(2);
        pool.add_eligible("adj_alpha");
        pool.add_eligible("adj_beta");
        pool.add_eligible("adj_gamma");
        pool.add_eligible("adj_delta");
        pool
    }

    fn make_court() -> CovenantCourt {
        let pool = make_pool();
        let mut court = CovenantCourt::new(
            CourtJurisdiction::InterCommunityDispute,
            AdjudicatorSelection::RandomFromPool(pool),
        );
        // Seat adjudicators from different communities.
        court
            .seat_adjudicator(CourtAdjudicator::new("adj_alpha", "community_1"))
            .unwrap();
        court
            .seat_adjudicator(CourtAdjudicator::new("adj_beta", "community_2"))
            .unwrap();
        court
            .seat_adjudicator(CourtAdjudicator::new("adj_gamma", "community_3"))
            .unwrap();
        court
    }

    fn make_petitioner() -> CourtParty {
        CourtParty::new("alice", "community_a", PartyRole::Petitioner)
    }

    fn make_respondent() -> CourtParty {
        CourtParty::new("bob", "community_b", PartyRole::Respondent)
    }

    fn make_case() -> CourtCase {
        CourtCase::file_against(
            CourtJurisdiction::InterCommunityDispute,
            make_petitioner(),
            make_respondent(),
            "Does Article 3 §2 permit cross-community data sharing without individual consent?",
        )
    }

    // -----------------------------------------------------------------------
    // Case filing
    // -----------------------------------------------------------------------

    #[test]
    fn case_filing_sets_initial_state() {
        let case = make_case();
        assert_eq!(case.status, CourtCaseStatus::Filed);
        assert!(case.decision.is_none());
        assert!(case.submissions.is_empty());
        assert_eq!(case.petitioner.role, PartyRole::Petitioner);
        assert!(case.respondent.is_some());
    }

    #[test]
    fn file_case_without_respondent() {
        let case = CourtCase::file(
            CourtJurisdiction::CovenantInterpretation,
            make_petitioner(),
            "How should Article 1 §4 be interpreted regarding digital communities?",
        );
        assert!(case.respondent.is_none());
        assert_eq!(case.jurisdiction, CourtJurisdiction::CovenantInterpretation);
    }

    #[test]
    fn court_rejects_wrong_jurisdiction() {
        let mut court = make_court();
        let case = CourtCase::file(
            CourtJurisdiction::AmendmentReview,
            make_petitioner(),
            "Is this amendment axiom-compatible?",
        );
        let result = court.file_case(case);
        assert!(result.is_err());
    }

    #[test]
    fn court_accepts_correct_jurisdiction() {
        let mut court = make_court();
        let case = make_case();
        let result = court.file_case(case);
        assert!(result.is_ok());
        assert_eq!(court.cases.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Case lifecycle
    // -----------------------------------------------------------------------

    #[test]
    fn full_case_lifecycle() {
        let mut case = make_case();

        // Filed -> Accepted
        case.accept().unwrap();
        assert_eq!(case.status, CourtCaseStatus::Accepted);

        // Accepted -> Hearing
        case.begin_hearing().unwrap();
        assert_eq!(case.status, CourtCaseStatus::Hearing);

        // Add submissions
        let sub = CourtSubmission::new(make_petitioner(), "We argue Article 3 §2 requires consent.");
        case.add_submission(sub).unwrap();
        assert_eq!(case.submissions.len(), 1);

        let sub2 = CourtSubmission::new(
            make_respondent(),
            "Community consent suffices per Article 1 §1.",
        );
        case.add_submission(sub2).unwrap();
        assert_eq!(case.submissions.len(), 2);

        // Hearing -> Deliberation
        case.begin_deliberation().unwrap();
        assert_eq!(case.status, CourtCaseStatus::Deliberation);

        // Decide
        let decision = CourtDecision::new(
            "Individual consent required",
            "Article 3 §2 is clear: consent is individual",
            "Cross-community data sharing requires individual, informed consent per Article 3 §2.",
        );
        case.decide(decision).unwrap();
        assert_eq!(case.status, CourtCaseStatus::Decided);
        assert!(!case.is_active());

        // Record as precedent
        let precedent_id = case.record_precedent().unwrap();
        assert_eq!(case.status, CourtCaseStatus::PrecedentRecorded);
        assert_eq!(
            case.decision.as_ref().unwrap().precedent_id,
            Some(precedent_id)
        );
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        let mut case = make_case();

        // Can't go to hearing from Filed (must accept first).
        assert!(case.begin_hearing().is_err());

        // Can't deliberate from Filed.
        assert!(case.begin_deliberation().is_err());

        // Can't decide from Filed.
        let decision = CourtDecision::new("x", "y", "z");
        assert!(case.decide(decision).is_err());

        // Can't record precedent from Filed.
        assert!(case.record_precedent().is_err());
    }

    #[test]
    fn cannot_add_submission_outside_hearing() {
        let mut case = make_case();
        let sub = CourtSubmission::new(make_petitioner(), "Premature submission");
        assert!(case.add_submission(sub).is_err());

        case.accept().unwrap();
        let sub2 = CourtSubmission::new(make_petitioner(), "Still premature");
        assert!(case.add_submission(sub2).is_err());
    }

    #[test]
    fn cannot_accept_already_accepted_case() {
        let mut case = make_case();
        case.accept().unwrap();
        assert!(case.accept().is_err());
    }

    #[test]
    fn is_active_tracks_lifecycle() {
        let mut case = make_case();
        assert!(case.is_active());

        case.accept().unwrap();
        assert!(case.is_active());

        case.begin_hearing().unwrap();
        assert!(case.is_active());

        case.begin_deliberation().unwrap();
        assert!(case.is_active());

        let decision = CourtDecision::new("s", "r", "i");
        case.decide(decision).unwrap();
        assert!(!case.is_active());
    }

    // -----------------------------------------------------------------------
    // Adjudicator selection & diversity
    // -----------------------------------------------------------------------

    #[test]
    fn pool_eligibility_checks() {
        let mut pool = AdjudicatorPool::new(2);
        pool.add_eligible("alice");
        pool.add_eligible("bob");
        pool.exclude("bob");

        assert!(pool.is_eligible("alice"));
        assert!(!pool.is_eligible("bob")); // excluded
        assert!(!pool.is_eligible("eve")); // not in pool
    }

    #[test]
    fn pool_tenure_limit() {
        let one_year_secs = 365 * 24 * 60 * 60;
        let pool = AdjudicatorPool::new(2).with_tenure_limit_secs(one_year_secs);
        assert_eq!(pool.tenure_limit_secs, Some(one_year_secs));
    }

    #[test]
    fn seat_adjudicator_checks_pool_eligibility() {
        let pool = make_pool();
        let mut court = CovenantCourt::new(
            CourtJurisdiction::InterCommunityDispute,
            AdjudicatorSelection::RandomFromPool(pool),
        );

        // Eligible adjudicator.
        let result =
            court.seat_adjudicator(CourtAdjudicator::new("adj_alpha", "community_1"));
        assert!(result.is_ok());

        // Ineligible adjudicator (not in pool).
        let result =
            court.seat_adjudicator(CourtAdjudicator::new("stranger", "community_99"));
        assert!(result.is_err());
    }

    #[test]
    fn seat_adjudicator_rejects_excluded() {
        let mut pool = make_pool();
        pool.exclude("adj_beta");
        let mut court = CovenantCourt::new(
            CourtJurisdiction::CovenantInterpretation,
            AdjudicatorSelection::RandomFromPool(pool),
        );

        let result =
            court.seat_adjudicator(CourtAdjudicator::new("adj_beta", "community_2"));
        assert!(result.is_err());
    }

    #[test]
    fn diversity_requirement_met() {
        let court = make_court(); // 3 adjudicators from 3 communities, min 2
        assert!(court.meets_diversity_requirement());
    }

    #[test]
    fn diversity_requirement_not_met() {
        let pool = AdjudicatorPool::new(3); // require 3 unique communities
        let mut court = CovenantCourt::new(
            CourtJurisdiction::InterCommunityDispute,
            AdjudicatorSelection::RandomFromPool(pool),
        );
        // Seat two from the same community — only 1 unique community.
        // We bypass pool checks by using NominatedByParties, then swap back.
        // Instead, let's build a pool that includes them.
        let mut pool2 = AdjudicatorPool::new(3);
        pool2.add_eligible("adj_1");
        pool2.add_eligible("adj_2");
        court.selection_method = AdjudicatorSelection::RandomFromPool(pool2);
        court
            .seat_adjudicator(CourtAdjudicator::new("adj_1", "same_community"))
            .unwrap();
        court
            .seat_adjudicator(CourtAdjudicator::new("adj_2", "same_community"))
            .unwrap();

        // Only 1 unique community, need 3.
        assert!(!court.meets_diversity_requirement());
    }

    #[test]
    fn nominated_by_parties_always_meets_diversity() {
        let mut court = CovenantCourt::new(
            CourtJurisdiction::ExclusionAppeal,
            AdjudicatorSelection::NominatedByParties,
        );
        court
            .seat_adjudicator(CourtAdjudicator::new("adj_x", "same"))
            .unwrap();
        // NominatedByParties has its own diversity guarantees.
        assert!(court.meets_diversity_requirement());
    }

    #[test]
    fn validate_panel_enforces_diversity() {
        let mut pool = AdjudicatorPool::new(2);
        pool.add_eligible("adj_1");
        pool.add_eligible("adj_2");
        let mut court = CovenantCourt::new(
            CourtJurisdiction::InterCommunityDispute,
            AdjudicatorSelection::RandomFromPool(pool),
        );
        court
            .seat_adjudicator(CourtAdjudicator::new("adj_1", "only_one"))
            .unwrap();
        court
            .seat_adjudicator(CourtAdjudicator::new("adj_2", "only_one"))
            .unwrap();

        let case = make_case();
        // Need 2 diverse communities among eligible, but all are from "only_one".
        let result = court.validate_panel_for_case(&case);
        assert!(result.is_err());
    }

    #[test]
    fn validate_panel_passes_with_diversity() {
        let court = make_court();
        let case = make_case();
        let result = court.validate_panel_for_case(&case);
        assert!(result.is_ok());
        // All 3 adjudicators are from communities not involved in the case.
        assert_eq!(result.unwrap().len(), 3);
    }

    // -----------------------------------------------------------------------
    // Recusal & conflict of interest
    // -----------------------------------------------------------------------

    #[test]
    fn adjudicator_conflict_with_petitioner_community() {
        let adj = CourtAdjudicator::new("adj", "community_a"); // same as petitioner
        let case = make_case();
        assert!(adj.has_conflict(&case));
    }

    #[test]
    fn adjudicator_conflict_with_respondent_community() {
        let adj = CourtAdjudicator::new("adj", "community_b"); // same as respondent
        let case = make_case();
        assert!(adj.has_conflict(&case));
    }

    #[test]
    fn adjudicator_no_conflict_with_unrelated_community() {
        let adj = CourtAdjudicator::new("adj", "community_neutral");
        let case = make_case();
        assert!(!adj.has_conflict(&case));
    }

    #[test]
    fn adjudicator_no_conflict_when_no_respondent() {
        let adj = CourtAdjudicator::new("adj", "community_b");
        let case = CourtCase::file(
            CourtJurisdiction::CovenantInterpretation,
            make_petitioner(),
            "Pure interpretation question",
        );
        // community_b is not the petitioner's community.
        assert!(!adj.has_conflict(&case));
    }

    #[test]
    fn recusal_records_case_id() {
        let mut adj = CourtAdjudicator::new("adj", "community_a");
        let case_id = Uuid::new_v4();
        adj.recuse(case_id);
        assert_eq!(adj.recusal_record, vec![case_id]);
    }

    #[test]
    fn recusal_is_idempotent() {
        let mut adj = CourtAdjudicator::new("adj", "community_a");
        let case_id = Uuid::new_v4();
        adj.recuse(case_id);
        adj.recuse(case_id);
        assert_eq!(adj.recusal_record.len(), 1);
    }

    #[test]
    fn eligible_adjudicators_excludes_conflicted() {
        let mut court = make_court();
        // Add an adjudicator from community_a (petitioner's community).
        let mut pool = make_pool();
        pool.add_eligible("adj_conflicted");
        court.selection_method = AdjudicatorSelection::RandomFromPool(pool);
        court
            .seat_adjudicator(CourtAdjudicator::new("adj_conflicted", "community_a"))
            .unwrap();

        let case = make_case();
        let eligible = court.eligible_adjudicators_for_case(&case);

        // adj_conflicted should be excluded (from community_a = petitioner).
        let pubkeys: Vec<&str> = eligible.iter().map(|a| a.pubkey.as_str()).collect();
        assert!(!pubkeys.contains(&"adj_conflicted"));
        // The original 3 should remain (from communities 1, 2, 3).
        assert_eq!(eligible.len(), 3);
    }

    // -----------------------------------------------------------------------
    // Submissions
    // -----------------------------------------------------------------------

    #[test]
    fn submission_with_evidence_hashes() {
        let sub = CourtSubmission::new(make_petitioner(), "Our argument")
            .with_evidence(vec!["hash_abc123".into(), "hash_def456".into()]);
        assert_eq!(sub.evidence_hashes.len(), 2);
        assert_eq!(sub.evidence_hashes[0], "hash_abc123");
    }

    #[test]
    fn submission_party_roles() {
        let amicus = CourtParty::new("observer", "community_c", PartyRole::AmicusCuriae);
        let sub = CourtSubmission::new(amicus, "Amicus brief");
        assert_eq!(sub.party.role, PartyRole::AmicusCuriae);
    }

    // -----------------------------------------------------------------------
    // Decision & dissent
    // -----------------------------------------------------------------------

    #[test]
    fn decision_starts_unanimous() {
        let decision = CourtDecision::new("Summary", "Reasoning", "Interpretation");
        assert!(decision.unanimous);
        assert!(decision.dissents.is_empty());
    }

    #[test]
    fn adding_dissent_marks_non_unanimous() {
        let mut decision = CourtDecision::new("Summary", "Reasoning", "Interpretation");
        decision.add_dissent(CourtDissent::new(
            "adj_gamma",
            "I disagree because Article 5 §1 provides an exception.",
        ));
        assert!(!decision.unanimous);
        assert_eq!(decision.dissents.len(), 1);
        assert_eq!(decision.dissents[0].adjudicator_pubkey, "adj_gamma");
    }

    #[test]
    fn multiple_dissents_preserved() {
        let mut decision = CourtDecision::new("S", "R", "I");
        decision.add_dissent(CourtDissent::new("adj_1", "Reason A"));
        decision.add_dissent(CourtDissent::new("adj_2", "Reason B"));
        assert_eq!(decision.dissents.len(), 2);
        assert!(!decision.unanimous);
    }

    // -----------------------------------------------------------------------
    // Precedent recording
    // -----------------------------------------------------------------------

    #[test]
    fn precedent_recording_produces_id() {
        let mut decision = CourtDecision::new("S", "R", "I");
        assert!(decision.precedent_id.is_none());
        let id = decision.record_as_precedent();
        assert_eq!(decision.precedent_id, Some(id));
    }

    #[test]
    fn court_collects_precedent_ids() {
        let mut court = make_court();

        // File and fully resolve a case.
        let case = make_case();
        let case_id = case.id;
        court.file_case(case).unwrap();

        // Drive the court's copy through the lifecycle.
        let c = court.get_case_mut(case_id).unwrap();
        c.accept().unwrap();
        c.begin_hearing().unwrap();
        c.begin_deliberation().unwrap();
        let decision = CourtDecision::new("S", "R", "I");
        c.decide(decision).unwrap();
        c.record_precedent().unwrap();

        let ids = court.precedent_ids();
        assert_eq!(ids.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Jurisdiction validation
    // -----------------------------------------------------------------------

    #[test]
    fn all_jurisdiction_variants_are_distinct() {
        let variants = [
            CourtJurisdiction::InterCommunityDispute,
            CourtJurisdiction::CovenantInterpretation,
            CourtJurisdiction::AmendmentReview,
            CourtJurisdiction::ExclusionAppeal,
        ];
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn all_party_roles_are_distinct() {
        let variants = [
            PartyRole::Petitioner,
            PartyRole::Respondent,
            PartyRole::Intervenor,
            PartyRole::AmicusCuriae,
        ];
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn all_case_statuses_are_distinct() {
        let variants = [
            CourtCaseStatus::Filed,
            CourtCaseStatus::Accepted,
            CourtCaseStatus::Hearing,
            CourtCaseStatus::Deliberation,
            CourtCaseStatus::Decided,
            CourtCaseStatus::PrecedentRecorded,
        ];
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Serialization round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn court_case_serialization_round_trip() {
        let case = make_case();
        let json = serde_json::to_string(&case).unwrap();
        let restored: CourtCase = serde_json::from_str(&json).unwrap();
        assert_eq!(case, restored);
    }

    #[test]
    fn court_decision_serialization_round_trip() {
        let mut decision = CourtDecision::new("Summary", "Reasoning", "Interpretation text");
        decision.add_dissent(CourtDissent::new("adj_1", "My dissent"));
        decision.record_as_precedent();

        let json = serde_json::to_string(&decision).unwrap();
        let restored: CourtDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(decision, restored);
    }

    #[test]
    fn covenant_court_serialization_round_trip() {
        let court = make_court();
        let json = serde_json::to_string(&court).unwrap();
        let restored: CovenantCourt = serde_json::from_str(&json).unwrap();
        assert_eq!(court, restored);
    }

    // -----------------------------------------------------------------------
    // Send + Sync
    // -----------------------------------------------------------------------

    #[test]
    fn types_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CovenantCourt>();
        assert_send_sync::<CourtCase>();
        assert_send_sync::<CourtDecision>();
        assert_send_sync::<CourtAdjudicator>();
        assert_send_sync::<CourtParty>();
        assert_send_sync::<CourtSubmission>();
        assert_send_sync::<CourtDissent>();
        assert_send_sync::<AdjudicatorPool>();
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn adjudicator_cases_heard_tracks_count() {
        let mut adj = CourtAdjudicator::new("adj", "c1");
        assert_eq!(adj.cases_heard, 0);
        adj.record_case_heard();
        adj.record_case_heard();
        assert_eq!(adj.cases_heard, 2);
    }

    #[test]
    fn involved_community_ids_with_respondent() {
        let case = make_case();
        let ids = case.involved_community_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"community_a"));
        assert!(ids.contains(&"community_b"));
    }

    #[test]
    fn involved_community_ids_without_respondent() {
        let case = CourtCase::file(
            CourtJurisdiction::CovenantInterpretation,
            make_petitioner(),
            "A pure interpretation question",
        );
        let ids = case.involved_community_ids();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains(&"community_a"));
    }

    #[test]
    fn court_get_case_returns_none_for_unknown_id() {
        let court = make_court();
        assert!(court.get_case(Uuid::new_v4()).is_none());
    }
}
