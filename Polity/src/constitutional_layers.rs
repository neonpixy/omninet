//! # Constitutional Layers — Three-Layer Covenant (R1A)
//!
//! Implements the three-layer constitutional structure from Master Plan v6:
//!
//! - **Layer 1: Axioms** — Dignity, Sovereignty, Consent. Immutable compile-time constants.
//!   Already exists as [`ImmutableFoundation::AXIOMS`].
//!
//! - **Layer 1b: Core & Commons** — The articles of Parts 01-02. Reconstitutable through
//!   extraordinary process (90% + Star Court unanimous + 2-year deliberation + axiom alignment).
//!
//! - **Layer 2: Constitutional Clauses** — Parts 03-09. Amendable through tiered thresholds
//!   (60-75% depending on part, 6-9 month deliberation).
//!
//! - **Layer 3: Interpretive Precedent** — Living, community-generated interpretations of
//!   Covenant principles. Voluntary, adoptable, supersedable.
//!
//! **Axiom guard:** Both `ClauseRegistry::amend()` and `ClauseRegistry::reconstitute()` validate
//! proposed text against [`ConstitutionalReviewer::review()`]. If the change weakens any axiom,
//! it is rejected. You can strengthen protections. You cannot weaken them.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

use crate::duties::DutyCategory;
use crate::error::PolityError;
use crate::immutable::ImmutableFoundation;
use crate::protections::{ActionDescription, ProhibitionType};
use crate::review::ConstitutionalReviewer;
use crate::rights::RightCategory;

// ---------------------------------------------------------------------------
// Layer 1 constants — the three axioms
// ---------------------------------------------------------------------------

/// The three axioms from which everything derives. Constants, not data.
/// Already present as `ImmutableFoundation::AXIOMS`; re-exported here for clarity.
pub const AXIOMS: [&str; 3] = ["Dignity", "Sovereignty", "Consent"];

// ---------------------------------------------------------------------------
// Layer 1b: Reconstitution (Core & Commons)
// ---------------------------------------------------------------------------

/// A proposal to reconstitute a Core or Commons article.
///
/// Reconstitution is the highest bar in the system: 90% of communities,
/// Star Court unanimous approval, 2-year minimum deliberation, and demonstrated
/// axiom alignment. It exists for the narrow case where the articles' language
/// unintentionally enables harm the axioms were meant to prevent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReconstitutionProposal {
    pub id: Uuid,
    pub trigger: ReconstitutionTrigger,
    pub affected_articles: Vec<String>,
    pub current_text: String,
    pub proposed_text: String,
    pub rationale: String,
    pub axiom_alignment: AxiomAlignment,
    pub proposer_communities: Vec<String>,
    pub status: ReconstitutionStatus,
}

impl ReconstitutionProposal {
    /// Create a new reconstitution proposal.
    ///
    /// The proposal starts in `Proposed` status and must advance through
    /// Deliberation, CovenantCourtReview, and CommunityVote before ratification.
    pub fn new(
        trigger: ReconstitutionTrigger,
        affected_articles: Vec<String>,
        current_text: impl Into<String>,
        proposed_text: impl Into<String>,
        rationale: impl Into<String>,
        axiom_alignment: AxiomAlignment,
        proposer_communities: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            trigger,
            affected_articles,
            current_text: current_text.into(),
            proposed_text: proposed_text.into(),
            rationale: rationale.into(),
            axiom_alignment,
            proposer_communities,
            status: ReconstitutionStatus::Proposed,
        }
    }
}

/// What triggered the reconstitution process.
///
/// From Covenant Continuum Art. 2 — five triggers for constitutional evolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ReconstitutionTrigger {
    /// Two articles within Core/Commons contradict each other.
    InternalContradiction(String, String),
    /// The law failed to prevent what it was meant to prevent.
    SustainedBreach(Vec<Uuid>),
    /// Specific language is ambiguous and causing divergent interpretation.
    InterpretiveAmbiguity(String),
    /// Material, relational, or technological reality has fundamentally changed.
    FundamentalTransformation(String),
    /// Communities formally declare the need for review.
    PublicInvocation(Vec<String>),
}

/// Demonstrates that a reconstitution strengthens (never weakens) the three axioms.
///
/// All three fields must be populated — the change must make every axiom MORE
/// protected, not less. Empty alignment is a rejection signal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AxiomAlignment {
    /// How the change better serves Dignity.
    pub serves_dignity: String,
    /// How the change better serves Sovereignty.
    pub serves_sovereignty: String,
    /// How the change better serves Consent.
    pub serves_consent: String,
}

impl AxiomAlignment {
    /// Returns `true` if all three axiom alignments are demonstrated (non-empty).
    pub fn is_complete(&self) -> bool {
        !self.serves_dignity.is_empty()
            && !self.serves_sovereignty.is_empty()
            && !self.serves_consent.is_empty()
    }
}

/// Lifecycle of a reconstitution proposal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ReconstitutionStatus {
    /// Just proposed, awaiting deliberation.
    Proposed,
    /// Under deliberation (minimum 2 years).
    Deliberation,
    /// Covenant Court (Star Court) reviewing for axiom alignment.
    CovenantCourtReview,
    /// Community vote underway.
    CommunityVote,
    /// Ratified — the reconstitution is enacted.
    Ratified,
    /// Rejected — the reconstitution failed.
    Rejected,
}

/// The threshold for reconstitution: the highest bar in the system.
#[derive(Debug, Clone, Copy)]
pub struct ReconstitutionThreshold;

impl ReconstitutionThreshold {
    /// 90% of communities must approve.
    pub const COMMUNITY_APPROVAL: f64 = 0.90;
    /// Star Court must be unanimous.
    pub const STAR_COURT_UNANIMOUS: bool = true;
    /// Minimum 2 years of deliberation.
    pub const MIN_DELIBERATION_DAYS: u64 = 730;

    /// Validate whether a ratification record meets the reconstitution threshold.
    pub fn is_met(record: &RatificationRecord, star_court_unanimous: bool) -> bool {
        let ratio = if record.communities_total == 0 {
            0.0
        } else {
            record.communities_for as f64 / record.communities_total as f64
        };

        let deliberation_days = (record.deliberation_ended - record.deliberation_started)
            .num_days()
            .unsigned_abs();

        ratio >= Self::COMMUNITY_APPROVAL
            && star_court_unanimous
            && deliberation_days >= Self::MIN_DELIBERATION_DAYS
    }
}

/// Validates that reconstituted text strengthens (never weakens) the axioms.
///
/// Uses both heuristic detection (`ImmutableFoundation::would_violate`) and
/// full constitutional review (`ConstitutionalReviewer::review`).
pub struct ReconstitutionGuard;

impl ReconstitutionGuard {
    /// Validate a reconstitution proposal against the axioms.
    ///
    /// Returns `Ok(())` if the proposed text strengthens the axioms.
    /// Returns `Err` if it weakens any axiom or fails review.
    pub fn validate(
        proposal: &ReconstitutionProposal,
        reviewer: &ConstitutionalReviewer<'_>,
    ) -> Result<(), PolityError> {
        // Check axiom alignment is complete
        if !proposal.axiom_alignment.is_complete() {
            return Err(PolityError::ReconstitutionAxiomAlignmentIncomplete);
        }

        // Heuristic check against immutable foundations
        if ImmutableFoundation::would_violate(&proposal.proposed_text) {
            return Err(PolityError::ReconstitutionWeakensAxiom(
                "proposed text contains signals that weaken immutable foundations".into(),
            ));
        }

        // Full constitutional review of the proposed text
        let action = ActionDescription {
            description: proposal.proposed_text.clone(),
            actor: "reconstitution_process".into(),
            violates: vec![],
        };
        let review = reviewer.review(&action);
        if review.result.is_breach() {
            return Err(PolityError::ReconstitutionWeakensAxiom(format!(
                "proposed text fails constitutional review: {} violation(s)",
                review.result.violations().len()
            )));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Layer 2: Constitutional Clauses (amendable)
// ---------------------------------------------------------------------------

/// The ten parts of the Covenant.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CovenantPart {
    /// Part 00 — Preamble
    Preamble,
    /// Part 01 — Core (rights, duties, protections)
    Core,
    /// Part 02 — Commons (shared inheritance)
    Commons,
    /// Part 03 — Coexistence (Earth, ecology)
    Coexistence,
    /// Part 04 — Conjunction (unions, labor, economy)
    Conjunction,
    /// Part 05 — Consortium (communities, governance)
    Consortium,
    /// Part 06 — Constellation (federation, networks)
    Constellation,
    /// Part 07 — Convocation (assemblies, collective action)
    Convocation,
    /// Part 08 — Continuum (evolution, amendment, reconstitution)
    Continuum,
    /// Part 09 — Compact (ratification, entry into force)
    Compact,
}

impl CovenantPart {
    /// Whether this part requires the reconstitution process (Core/Commons).
    pub fn requires_reconstitution(&self) -> bool {
        matches!(self, CovenantPart::Core | CovenantPart::Commons)
    }
}

/// A single clause in the Covenant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConstitutionalClause {
    pub id: Uuid,
    pub part: CovenantPart,
    pub article: String,
    pub section: String,
    pub text: String,
    pub enacted_at: DateTime<Utc>,
    pub amended_history: Vec<ClauseAmendment>,
}

impl ConstitutionalClause {
    /// Create a new constitutional clause with a fresh UUID and empty amendment history.
    pub fn new(
        part: CovenantPart,
        article: impl Into<String>,
        section: impl Into<String>,
        text: impl Into<String>,
        enacted_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            part,
            article: article.into(),
            section: section.into(),
            text: text.into(),
            enacted_at,
            amended_history: Vec::new(),
        }
    }
}

/// A record of a clause amendment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClauseAmendment {
    pub id: Uuid,
    pub clause_id: Uuid,
    pub old_text: String,
    pub new_text: String,
    pub rationale: String,
    pub proposer: String,
    pub ratified_at: DateTime<Utc>,
    pub ratification_record: RatificationRecord,
}

impl ClauseAmendment {
    /// Create a record of a clause amendment, capturing old and new text plus ratification details.
    pub fn new(
        clause_id: Uuid,
        old_text: impl Into<String>,
        new_text: impl Into<String>,
        rationale: impl Into<String>,
        proposer: impl Into<String>,
        ratified_at: DateTime<Utc>,
        ratification_record: RatificationRecord,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            clause_id,
            old_text: old_text.into(),
            new_text: new_text.into(),
            rationale: rationale.into(),
            proposer: proposer.into(),
            ratified_at,
            ratification_record,
        }
    }
}

/// Record of a ratification vote — captures the community tally and deliberation period.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RatificationRecord {
    /// Number of communities that voted in favor.
    pub communities_for: usize,
    /// Number of communities that voted against.
    pub communities_against: usize,
    /// Total eligible communities.
    pub communities_total: usize,
    /// Whether the threshold was met at the time of recording.
    pub threshold_met: bool,
    /// When deliberation began.
    pub deliberation_started: DateTime<Utc>,
    /// When deliberation ended and the vote was taken.
    pub deliberation_ended: DateTime<Utc>,
}

/// Amendment threshold hierarchy — different parts require different thresholds.
///
/// - Core/Commons (Parts 01-02): Reconstitution process (90% + 2yr).
/// - Coexistence/Conjunction (Parts 03-04): 75% + 9-month deliberation.
/// - Consortium through Compact (Parts 05-09): 60% + 6-month deliberation.
#[derive(Debug, Clone, Copy)]
pub struct AmendmentThreshold;

impl AmendmentThreshold {
    /// Threshold for Core and Commons (handled by reconstitution).
    pub const CORE_COMMONS_RATIO: f64 = 0.90;
    /// Minimum deliberation for Core/Commons (days).
    pub const CORE_COMMONS_DELIBERATION_DAYS: u64 = 730;

    /// Threshold for Coexistence and Conjunction (Parts 03-04).
    pub const PARTS_03_04_RATIO: f64 = 0.75;
    /// Minimum deliberation for Parts 03-04 (days, ~9 months).
    pub const PARTS_03_04_DELIBERATION_DAYS: u64 = 270;

    /// Threshold for Parts 05-09.
    pub const PARTS_05_09_RATIO: f64 = 0.60;
    /// Minimum deliberation for Parts 05-09 (days, ~6 months).
    pub const PARTS_05_09_DELIBERATION_DAYS: u64 = 180;

    /// Get the required approval ratio for a given Covenant part.
    pub fn required_ratio(part: CovenantPart) -> f64 {
        match part {
            CovenantPart::Preamble => 1.0, // effectively unamendable
            CovenantPart::Core | CovenantPart::Commons => Self::CORE_COMMONS_RATIO,
            CovenantPart::Coexistence | CovenantPart::Conjunction => Self::PARTS_03_04_RATIO,
            CovenantPart::Consortium
            | CovenantPart::Constellation
            | CovenantPart::Convocation
            | CovenantPart::Continuum
            | CovenantPart::Compact => Self::PARTS_05_09_RATIO,
        }
    }

    /// Get the minimum deliberation period in days for a given Covenant part.
    pub fn min_deliberation_days(part: CovenantPart) -> u64 {
        match part {
            CovenantPart::Preamble => u64::MAX,
            CovenantPart::Core | CovenantPart::Commons => Self::CORE_COMMONS_DELIBERATION_DAYS,
            CovenantPart::Coexistence | CovenantPart::Conjunction => {
                Self::PARTS_03_04_DELIBERATION_DAYS
            }
            CovenantPart::Consortium
            | CovenantPart::Constellation
            | CovenantPart::Convocation
            | CovenantPart::Continuum
            | CovenantPart::Compact => Self::PARTS_05_09_DELIBERATION_DAYS,
        }
    }

    /// Validate whether a ratification record meets the threshold for a given part.
    pub fn is_met(part: CovenantPart, record: &RatificationRecord) -> bool {
        let ratio = if record.communities_total == 0 {
            0.0
        } else {
            record.communities_for as f64 / record.communities_total as f64
        };

        let deliberation_days = (record.deliberation_ended - record.deliberation_started)
            .num_days()
            .unsigned_abs();

        ratio >= Self::required_ratio(part)
            && deliberation_days >= Self::min_deliberation_days(part)
    }
}

/// Thread-safe registry of all constitutional clauses.
///
/// Provides registration, lookup, amendment, and reconstitution with axiom guards.
#[derive(Debug)]
pub struct ClauseRegistry {
    clauses: RwLock<HashMap<Uuid, ConstitutionalClause>>,
}

impl ClauseRegistry {
    /// Create an empty clause registry.
    pub fn new() -> Self {
        Self {
            clauses: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new constitutional clause.
    pub fn register(&self, clause: ConstitutionalClause) -> Result<Uuid, PolityError> {
        let id = clause.id;
        let mut clauses = self.clauses.write().map_err(|_| {
            PolityError::ClauseRegistryPoisoned
        })?;
        if clauses.contains_key(&id) {
            return Err(PolityError::DuplicateClause(id));
        }
        clauses.insert(id, clause);
        Ok(id)
    }

    /// Get all clauses for a given Covenant part.
    pub fn by_part(&self, part: CovenantPart) -> Result<Vec<ConstitutionalClause>, PolityError> {
        let clauses = self.clauses.read().map_err(|_| {
            PolityError::ClauseRegistryPoisoned
        })?;
        Ok(clauses
            .values()
            .filter(|c| c.part == part)
            .cloned()
            .collect())
    }

    /// Get all registered clauses.
    pub fn all(&self) -> Result<Vec<ConstitutionalClause>, PolityError> {
        let clauses = self.clauses.read().map_err(|_| {
            PolityError::ClauseRegistryPoisoned
        })?;
        Ok(clauses.values().cloned().collect())
    }

    /// Get a single clause by ID.
    pub fn get(&self, id: &Uuid) -> Result<Option<ConstitutionalClause>, PolityError> {
        let clauses = self.clauses.read().map_err(|_| {
            PolityError::ClauseRegistryPoisoned
        })?;
        Ok(clauses.get(id).cloned())
    }

    /// Number of registered clauses.
    pub fn len(&self) -> Result<usize, PolityError> {
        let clauses = self.clauses.read().map_err(|_| {
            PolityError::ClauseRegistryPoisoned
        })?;
        Ok(clauses.len())
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> Result<bool, PolityError> {
        Ok(self.len()? == 0)
    }

    /// Amend a clause (Parts 03-09 only — Core/Commons must use `reconstitute`).
    ///
    /// Validates the amendment against the axiom guard: if the new text weakens any
    /// axiom (Dignity, Sovereignty, Consent), the amendment is rejected.
    pub fn amend(
        &self,
        clause_id: Uuid,
        amendment: ClauseAmendment,
        reviewer: &ConstitutionalReviewer<'_>,
    ) -> Result<(), PolityError> {
        let mut clauses = self.clauses.write().map_err(|_| {
            PolityError::ClauseRegistryPoisoned
        })?;

        let clause = clauses
            .get(&clause_id)
            .ok_or(PolityError::ClauseNotFound(clause_id))?;

        // Core/Commons must use reconstitute, not amend
        if clause.part.requires_reconstitution() {
            return Err(PolityError::ClauseRequiresReconstitution(clause_id));
        }

        // Validate threshold for this part
        if !AmendmentThreshold::is_met(clause.part, &amendment.ratification_record) {
            return Err(PolityError::ThresholdNotMet {
                required: AmendmentThreshold::required_ratio(clause.part),
                actual: if amendment.ratification_record.communities_total == 0 {
                    0.0
                } else {
                    amendment.ratification_record.communities_for as f64
                        / amendment.ratification_record.communities_total as f64
                },
            });
        }

        // Axiom guard — heuristic check
        if ImmutableFoundation::would_violate(&amendment.new_text) {
            return Err(PolityError::AmendmentContradictsFoundation(
                "amendment weakens immutable foundations".into(),
            ));
        }

        // Axiom guard — full constitutional review
        let action = ActionDescription {
            description: amendment.new_text.clone(),
            actor: "amendment_process".into(),
            violates: vec![],
        };
        let review = reviewer.review(&action);
        if review.result.is_breach() {
            return Err(PolityError::AmendmentContradictsFoundation(format!(
                "amendment fails constitutional review: {} violation(s)",
                review.result.violations().len()
            )));
        }

        // Apply amendment
        let clause = clauses.get_mut(&clause_id).expect("clause existence already verified");
        clause.text = amendment.new_text.clone();
        clause.amended_history.push(amendment);
        Ok(())
    }

    /// Reconstitute a Core or Commons clause through the extraordinary process.
    ///
    /// This is the highest bar in the system. The reconstitution must:
    /// 1. Target a Core or Commons clause
    /// 2. Demonstrate axiom alignment (all three)
    /// 3. Pass the axiom guard (no weakening of Dignity, Sovereignty, or Consent)
    /// 4. Meet the reconstitution threshold (90% + 2yr + Star Court unanimous)
    pub fn reconstitute(
        &self,
        clause_id: Uuid,
        proposal: &ReconstitutionProposal,
        reviewer: &ConstitutionalReviewer<'_>,
        ratification: &RatificationRecord,
        star_court_unanimous: bool,
    ) -> Result<(), PolityError> {
        let mut clauses = self.clauses.write().map_err(|_| {
            PolityError::ClauseRegistryPoisoned
        })?;

        let clause = clauses
            .get(&clause_id)
            .ok_or(PolityError::ClauseNotFound(clause_id))?;

        // Only Core/Commons can be reconstituted
        if !clause.part.requires_reconstitution() {
            return Err(PolityError::ClauseDoesNotRequireReconstitution(clause_id));
        }

        // Validate reconstitution threshold
        if !ReconstitutionThreshold::is_met(ratification, star_court_unanimous) {
            return Err(PolityError::ReconstitutionThresholdNotMet);
        }

        // Axiom guard via ReconstitutionGuard (releases the write lock temporarily
        // is not possible, so we do the checks inline here)
        if !proposal.axiom_alignment.is_complete() {
            return Err(PolityError::ReconstitutionAxiomAlignmentIncomplete);
        }

        if ImmutableFoundation::would_violate(&proposal.proposed_text) {
            return Err(PolityError::ReconstitutionWeakensAxiom(
                "proposed text contains signals that weaken immutable foundations".into(),
            ));
        }

        let action = ActionDescription {
            description: proposal.proposed_text.clone(),
            actor: "reconstitution_process".into(),
            violates: vec![],
        };
        let review = reviewer.review(&action);
        if review.result.is_breach() {
            return Err(PolityError::ReconstitutionWeakensAxiom(format!(
                "proposed text fails constitutional review: {} violation(s)",
                review.result.violations().len()
            )));
        }

        // Apply reconstitution
        let clause = clauses.get_mut(&clause_id).expect("clause existence already verified");
        let amendment = ClauseAmendment::new(
            clause_id,
            clause.text.clone(),
            proposal.proposed_text.clone(),
            proposal.rationale.clone(),
            proposal.proposer_communities.join(", "),
            Utc::now(),
            ratification.clone(),
        );
        clause.text = proposal.proposed_text.clone();
        clause.amended_history.push(amendment);
        Ok(())
    }
}

impl Default for ClauseRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Layer 3: Interpretive Precedent
// ---------------------------------------------------------------------------

/// A reference to which Covenant principle is being interpreted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PrincipleReference {
    /// One of the three axioms (Dignity, Sovereignty, Consent).
    Axiom(String),
    /// A right category from Core Art. 2.
    Right(RightCategory),
    /// A duty category from Core Art. 4.
    Duty(DutyCategory),
    /// A prohibition type from Core Art. 5.
    Prohibition(ProhibitionType),
    /// A specific constitutional clause by ID.
    Clause(Uuid),
}

/// An interpretive precedent — a community's novel interpretation of a Covenant principle.
///
/// When Jail's Dispute or Kingdom's Challenge resolves, the adjudicators can optionally
/// record a `CovenantPrecedent` if their decision interprets a Covenant principle in a
/// novel way. Over time, a body of interpretive law develops organically.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CovenantPrecedent {
    pub id: Uuid,
    /// The Jail Dispute or Kingdom Challenge that generated this precedent.
    pub case_id: Uuid,
    /// Which axiom, right, duty, or prohibition is being interpreted.
    pub principle_interpreted: PrincipleReference,
    /// The interpretation itself.
    pub interpretation: String,
    /// The reasoning behind the interpretation.
    pub reasoning: String,
    /// The community that established this precedent.
    pub community_id: String,
    /// Pubkeys of the adjudicators who decided.
    pub adjudicators: Vec<String>,
    /// When the precedent was established.
    pub established_at: DateTime<Utc>,
    /// How many communities have adopted this precedent.
    pub adoption_count: usize,
    /// If this precedent has been superseded by a newer one.
    pub superseded_by: Option<Uuid>,
}

impl CovenantPrecedent {
    /// Create a new precedent. The establishing community is automatically its first adopter.
    pub fn new(
        case_id: Uuid,
        principle_interpreted: PrincipleReference,
        interpretation: impl Into<String>,
        reasoning: impl Into<String>,
        community_id: impl Into<String>,
        adjudicators: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            case_id,
            principle_interpreted,
            interpretation: interpretation.into(),
            reasoning: reasoning.into(),
            community_id: community_id.into(),
            adjudicators,
            established_at: Utc::now(),
            adoption_count: 1, // the establishing community adopts it
            superseded_by: None,
        }
    }

    /// Whether this precedent is still active (not superseded).
    pub fn is_active(&self) -> bool {
        self.superseded_by.is_none()
    }
}

/// Search parameters for finding precedents. Build with the fluent API and pass to
/// [`PrecedentRegistry::search`].
#[derive(Debug, Clone, Default)]
pub struct PrecedentSearch {
    /// Filter by which principle is being interpreted.
    pub principle: Option<PrincipleReference>,
    /// Filter by establishing or adopting community.
    pub community_id: Option<String>,
    /// Filter by when the precedent was established (inclusive range).
    pub date_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    /// Filter by minimum number of community adoptions.
    pub adoption_count_min: Option<usize>,
}

impl PrecedentSearch {
    /// Create a search with no filters (matches all precedents).
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter results to precedents interpreting a specific principle.
    pub fn by_principle(mut self, principle: PrincipleReference) -> Self {
        self.principle = Some(principle);
        self
    }

    /// Filter results to precedents established by or adopted in a specific community.
    pub fn by_community(mut self, community_id: impl Into<String>) -> Self {
        self.community_id = Some(community_id.into());
        self
    }

    /// Filter results to precedents established within a date range.
    pub fn by_date_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.date_range = Some((start, end));
        self
    }

    /// Filter results to precedents adopted by at least `min` communities.
    pub fn by_adoption_count_min(mut self, min: usize) -> Self {
        self.adoption_count_min = Some(min);
        self
    }
}

/// Thread-safe registry of interpretive precedents.
#[derive(Debug)]
pub struct PrecedentRegistry {
    precedents: RwLock<HashMap<Uuid, CovenantPrecedent>>,
    /// Tracks which communities adopted which precedents.
    adoptions: RwLock<HashMap<Uuid, Vec<String>>>,
}

impl PrecedentRegistry {
    /// Create an empty precedent registry.
    pub fn new() -> Self {
        Self {
            precedents: RwLock::new(HashMap::new()),
            adoptions: RwLock::new(HashMap::new()),
        }
    }

    /// Record a new precedent.
    pub fn record(&self, precedent: CovenantPrecedent) -> Result<Uuid, PolityError> {
        let id = precedent.id;
        let community = precedent.community_id.clone();

        let mut precedents = self.precedents.write().map_err(|_| {
            PolityError::PrecedentRegistryPoisoned
        })?;
        let mut adoptions = self.adoptions.write().map_err(|_| {
            PolityError::PrecedentRegistryPoisoned
        })?;

        precedents.insert(id, precedent);
        adoptions.entry(id).or_default().push(community);
        Ok(id)
    }

    /// Find all precedents interpreting a given principle.
    pub fn for_principle(
        &self,
        reference: &PrincipleReference,
    ) -> Result<Vec<CovenantPrecedent>, PolityError> {
        let precedents = self.precedents.read().map_err(|_| {
            PolityError::PrecedentRegistryPoisoned
        })?;
        Ok(precedents
            .values()
            .filter(|p| &p.principle_interpreted == reference)
            .cloned()
            .collect())
    }

    /// Find all precedents adopted by a specific community.
    pub fn adopted_by(
        &self,
        community_id: &str,
    ) -> Result<Vec<CovenantPrecedent>, PolityError> {
        let precedents = self.precedents.read().map_err(|_| {
            PolityError::PrecedentRegistryPoisoned
        })?;
        let adoptions = self.adoptions.read().map_err(|_| {
            PolityError::PrecedentRegistryPoisoned
        })?;

        let adopted_ids: Vec<Uuid> = adoptions
            .iter()
            .filter(|(_, communities)| communities.contains(&community_id.to_string()))
            .map(|(id, _)| *id)
            .collect();

        Ok(adopted_ids
            .iter()
            .filter_map(|id| precedents.get(id).cloned())
            .collect())
    }

    /// Adopt a precedent in a new community.
    pub fn adopt(
        &self,
        precedent_id: Uuid,
        community_id: impl Into<String>,
    ) -> Result<(), PolityError> {
        let community = community_id.into();

        let mut precedents = self.precedents.write().map_err(|_| {
            PolityError::PrecedentRegistryPoisoned
        })?;
        let mut adoptions = self.adoptions.write().map_err(|_| {
            PolityError::PrecedentRegistryPoisoned
        })?;

        let precedent = precedents
            .get_mut(&precedent_id)
            .ok_or(PolityError::PrecedentNotFound(precedent_id))?;

        let adopters = adoptions.entry(precedent_id).or_default();
        if !adopters.contains(&community) {
            adopters.push(community);
            precedent.adoption_count = adopters.len();
        }

        Ok(())
    }

    /// Supersede an old precedent with a new one.
    pub fn supersede(
        &self,
        old_id: Uuid,
        new_id: Uuid,
    ) -> Result<(), PolityError> {
        let mut precedents = self.precedents.write().map_err(|_| {
            PolityError::PrecedentRegistryPoisoned
        })?;

        // Verify both exist
        if !precedents.contains_key(&new_id) {
            return Err(PolityError::PrecedentNotFound(new_id));
        }

        let old = precedents
            .get_mut(&old_id)
            .ok_or(PolityError::PrecedentNotFound(old_id))?;

        old.superseded_by = Some(new_id);
        Ok(())
    }

    /// Get the most-adopted precedents, up to `n`.
    pub fn most_adopted(&self, n: usize) -> Result<Vec<CovenantPrecedent>, PolityError> {
        let precedents = self.precedents.read().map_err(|_| {
            PolityError::PrecedentRegistryPoisoned
        })?;

        let mut sorted: Vec<_> = precedents
            .values()
            .filter(|p| p.is_active())
            .cloned()
            .collect();
        sorted.sort_by(|a, b| b.adoption_count.cmp(&a.adoption_count));
        sorted.truncate(n);
        Ok(sorted)
    }

    /// Search precedents by multiple criteria.
    pub fn search(&self, query: &PrecedentSearch) -> Result<Vec<CovenantPrecedent>, PolityError> {
        let precedents = self.precedents.read().map_err(|_| {
            PolityError::PrecedentRegistryPoisoned
        })?;
        let adoptions = self.adoptions.read().map_err(|_| {
            PolityError::PrecedentRegistryPoisoned
        })?;

        let results: Vec<_> = precedents
            .values()
            .filter(|p| {
                // Filter by principle
                if let Some(ref principle) = query.principle {
                    if &p.principle_interpreted != principle {
                        return false;
                    }
                }

                // Filter by community (established or adopted)
                if let Some(ref community) = query.community_id {
                    let adopted = adoptions
                        .get(&p.id)
                        .map(|c| c.contains(community))
                        .unwrap_or(false);
                    if p.community_id != *community && !adopted {
                        return false;
                    }
                }

                // Filter by date range
                if let Some((start, end)) = query.date_range {
                    if p.established_at < start || p.established_at > end {
                        return false;
                    }
                }

                // Filter by minimum adoption count
                if let Some(min) = query.adoption_count_min {
                    if p.adoption_count < min {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect();

        Ok(results)
    }

    /// Get a single precedent by ID.
    pub fn get(&self, id: &Uuid) -> Result<Option<CovenantPrecedent>, PolityError> {
        let precedents = self.precedents.read().map_err(|_| {
            PolityError::PrecedentRegistryPoisoned
        })?;
        Ok(precedents.get(id).cloned())
    }

    /// Number of recorded precedents.
    pub fn len(&self) -> Result<usize, PolityError> {
        let precedents = self.precedents.read().map_err(|_| {
            PolityError::PrecedentRegistryPoisoned
        })?;
        Ok(precedents.len())
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> Result<bool, PolityError> {
        Ok(self.len()? == 0)
    }
}

impl Default for PrecedentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;

    use crate::protections::ProtectionsRegistry;
    use crate::rights::RightsRegistry;

    // -- Helpers --

    fn make_reviewer() -> (RightsRegistry, ProtectionsRegistry) {
        (RightsRegistry::default(), ProtectionsRegistry::default())
    }

    fn make_ratification_met(part: CovenantPart) -> RatificationRecord {
        let ratio = AmendmentThreshold::required_ratio(part);
        let days = AmendmentThreshold::min_deliberation_days(part);
        let total = 100;
        let f = (ratio * total as f64).ceil() as usize;
        let started = Utc::now() - TimeDelta::days(days as i64 + 1);
        RatificationRecord {
            communities_for: f,
            communities_against: total - f,
            communities_total: total,
            threshold_met: true,
            deliberation_started: started,
            deliberation_ended: Utc::now(),
        }
    }

    fn make_reconstitution_ratification() -> RatificationRecord {
        let started = Utc::now() - TimeDelta::days(731);
        RatificationRecord {
            communities_for: 95,
            communities_against: 5,
            communities_total: 100,
            threshold_met: true,
            deliberation_started: started,
            deliberation_ended: Utc::now(),
        }
    }

    fn make_clause(part: CovenantPart) -> ConstitutionalClause {
        ConstitutionalClause::new(
            part,
            "Art. 1",
            "Section 1",
            "Original clause text about protecting dignity.",
            Utc::now(),
        )
    }

    fn make_axiom_alignment() -> AxiomAlignment {
        AxiomAlignment {
            serves_dignity: "Strengthens protection of inherent worth".into(),
            serves_sovereignty: "Expands self-determination".into(),
            serves_consent: "Requires explicit informed consent".into(),
        }
    }

    // -----------------------------------------------------------------------
    // Layer 1: Axioms
    // -----------------------------------------------------------------------

    #[test]
    fn axiom_constants_match_foundation() {
        assert_eq!(AXIOMS.len(), 3);
        assert_eq!(AXIOMS[0], "Dignity");
        assert_eq!(AXIOMS[1], "Sovereignty");
        assert_eq!(AXIOMS[2], "Consent");
    }

    // -----------------------------------------------------------------------
    // Layer 1b: Reconstitution
    // -----------------------------------------------------------------------

    #[test]
    fn reconstitution_proposal_creation() {
        let proposal = ReconstitutionProposal::new(
            ReconstitutionTrigger::InternalContradiction(
                "Core Art. 2 Section 4".into(),
                "Core Art. 5 Section 3".into(),
            ),
            vec!["Core Art. 2 Section 4".into()],
            "Original text with implicit gap",
            "Revised text closing the gap",
            "The current language allows harm the axioms prevent",
            make_axiom_alignment(),
            vec!["community_a".into(), "community_b".into()],
        );

        assert_eq!(proposal.status, ReconstitutionStatus::Proposed);
        assert_eq!(proposal.affected_articles.len(), 1);
        assert_eq!(proposal.proposer_communities.len(), 2);
    }

    #[test]
    fn axiom_alignment_complete_check() {
        let complete = make_axiom_alignment();
        assert!(complete.is_complete());

        let incomplete = AxiomAlignment {
            serves_dignity: "yes".into(),
            serves_sovereignty: String::new(),
            serves_consent: "yes".into(),
        };
        assert!(!incomplete.is_complete());
    }

    #[test]
    fn reconstitution_threshold_met() {
        let record = make_reconstitution_ratification();
        assert!(ReconstitutionThreshold::is_met(&record, true));
    }

    #[test]
    fn reconstitution_threshold_fails_low_ratio() {
        let started = Utc::now() - TimeDelta::days(731);
        let record = RatificationRecord {
            communities_for: 80,
            communities_against: 20,
            communities_total: 100,
            threshold_met: false,
            deliberation_started: started,
            deliberation_ended: Utc::now(),
        };
        assert!(!ReconstitutionThreshold::is_met(&record, true));
    }

    #[test]
    fn reconstitution_threshold_fails_no_star_court() {
        let record = make_reconstitution_ratification();
        assert!(!ReconstitutionThreshold::is_met(&record, false));
    }

    #[test]
    fn reconstitution_threshold_fails_short_deliberation() {
        let started = Utc::now() - TimeDelta::days(365); // only 1 year
        let record = RatificationRecord {
            communities_for: 95,
            communities_against: 5,
            communities_total: 100,
            threshold_met: true,
            deliberation_started: started,
            deliberation_ended: Utc::now(),
        };
        assert!(!ReconstitutionThreshold::is_met(&record, true));
    }

    #[test]
    fn reconstitution_guard_accepts_strengthening() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);

        let proposal = ReconstitutionProposal::new(
            ReconstitutionTrigger::InterpretiveAmbiguity("vague language".into()),
            vec!["Core Art. 2 Section 5".into()],
            "Original privacy protection",
            "Strengthened privacy protection with explicit algorithmic safeguards",
            "Current language does not address algorithmic harms explicitly",
            make_axiom_alignment(),
            vec!["privacy_advocates".into()],
        );

        assert!(ReconstitutionGuard::validate(&proposal, &reviewer).is_ok());
    }

    #[test]
    fn reconstitution_guard_rejects_weakening() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);

        let proposal = ReconstitutionProposal::new(
            ReconstitutionTrigger::PublicInvocation(vec!["bad_actors".into()]),
            vec!["Core Art. 5".into()],
            "No surveillance shall be permitted",
            "Permit surveillance during declared emergencies",
            "We need surveillance sometimes",
            make_axiom_alignment(),
            vec!["fearful_community".into()],
        );

        let result = ReconstitutionGuard::validate(&proposal, &reviewer);
        assert!(matches!(
            result,
            Err(PolityError::ReconstitutionWeakensAxiom(_))
        ));
    }

    #[test]
    fn reconstitution_guard_rejects_incomplete_alignment() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);

        let proposal = ReconstitutionProposal::new(
            ReconstitutionTrigger::InterpretiveAmbiguity("vague".into()),
            vec!["Core Art. 2".into()],
            "Original",
            "Proposed",
            "Rationale",
            AxiomAlignment {
                serves_dignity: "yes".into(),
                serves_sovereignty: String::new(),
                serves_consent: "yes".into(),
            },
            vec!["community".into()],
        );

        let result = ReconstitutionGuard::validate(&proposal, &reviewer);
        assert!(matches!(
            result,
            Err(PolityError::ReconstitutionAxiomAlignmentIncomplete)
        ));
    }

    // -----------------------------------------------------------------------
    // Layer 2: Clause Registry
    // -----------------------------------------------------------------------

    #[test]
    fn clause_registration() {
        let registry = ClauseRegistry::new();
        let clause = make_clause(CovenantPart::Coexistence);
        let id = registry.register(clause).unwrap();

        let stored = registry.get(&id).unwrap().unwrap();
        assert_eq!(stored.part, CovenantPart::Coexistence);
        assert_eq!(stored.article, "Art. 1");
        assert_eq!(registry.len().unwrap(), 1);
    }

    #[test]
    fn clause_by_part() {
        let registry = ClauseRegistry::new();
        registry
            .register(make_clause(CovenantPart::Coexistence))
            .unwrap();
        registry
            .register(make_clause(CovenantPart::Coexistence))
            .unwrap();
        registry
            .register(make_clause(CovenantPart::Conjunction))
            .unwrap();

        let coexistence = registry.by_part(CovenantPart::Coexistence).unwrap();
        assert_eq!(coexistence.len(), 2);

        let conjunction = registry.by_part(CovenantPart::Conjunction).unwrap();
        assert_eq!(conjunction.len(), 1);

        let core = registry.by_part(CovenantPart::Core).unwrap();
        assert!(core.is_empty());
    }

    #[test]
    fn clause_all() {
        let registry = ClauseRegistry::new();
        registry
            .register(make_clause(CovenantPart::Core))
            .unwrap();
        registry
            .register(make_clause(CovenantPart::Consortium))
            .unwrap();

        assert_eq!(registry.all().unwrap().len(), 2);
    }

    #[test]
    fn amend_clause_with_valid_threshold() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);
        let registry = ClauseRegistry::new();

        let clause = make_clause(CovenantPart::Consortium);
        let clause_id = clause.id;
        registry.register(clause).unwrap();

        let amendment = ClauseAmendment::new(
            clause_id,
            "Original clause text about protecting dignity.",
            "Strengthened clause text with additional protections for dignity and consent.",
            "Original text was ambiguous about consent requirements",
            "reform_council",
            Utc::now(),
            make_ratification_met(CovenantPart::Consortium),
        );

        registry.amend(clause_id, amendment, &reviewer).unwrap();

        let updated = registry.get(&clause_id).unwrap().unwrap();
        assert!(updated
            .text
            .contains("Strengthened clause text"));
        assert_eq!(updated.amended_history.len(), 1);
    }

    #[test]
    fn amend_clause_rejects_axiom_violation() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);
        let registry = ClauseRegistry::new();

        let clause = make_clause(CovenantPart::Consortium);
        let clause_id = clause.id;
        registry.register(clause).unwrap();

        let amendment = ClauseAmendment::new(
            clause_id,
            "Original text",
            "Allow surveillance during crisis periods",
            "Emergency powers",
            "fearful_council",
            Utc::now(),
            make_ratification_met(CovenantPart::Consortium),
        );

        let result = registry.amend(clause_id, amendment, &reviewer);
        assert!(matches!(
            result,
            Err(PolityError::AmendmentContradictsFoundation(_))
        ));
    }

    #[test]
    fn amend_core_clause_rejected() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);
        let registry = ClauseRegistry::new();

        let clause = make_clause(CovenantPart::Core);
        let clause_id = clause.id;
        registry.register(clause).unwrap();

        let amendment = ClauseAmendment::new(
            clause_id,
            "Original",
            "Strengthened protections",
            "Improvement",
            "council",
            Utc::now(),
            make_ratification_met(CovenantPart::Core),
        );

        let result = registry.amend(clause_id, amendment, &reviewer);
        assert!(matches!(
            result,
            Err(PolityError::ClauseRequiresReconstitution(_))
        ));
    }

    #[test]
    fn amend_clause_rejects_insufficient_threshold() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);
        let registry = ClauseRegistry::new();

        let clause = make_clause(CovenantPart::Coexistence); // needs 75%
        let clause_id = clause.id;
        registry.register(clause).unwrap();

        // Only 50% approval
        let started = Utc::now() - TimeDelta::days(300);
        let weak_ratification = RatificationRecord {
            communities_for: 50,
            communities_against: 50,
            communities_total: 100,
            threshold_met: false,
            deliberation_started: started,
            deliberation_ended: Utc::now(),
        };

        let amendment = ClauseAmendment::new(
            clause_id,
            "Original",
            "Better protections for ecological systems",
            "Strengthen ecology provisions",
            "ecology_council",
            Utc::now(),
            weak_ratification,
        );

        let result = registry.amend(clause_id, amendment, &reviewer);
        assert!(matches!(result, Err(PolityError::ThresholdNotMet { .. })));
    }

    #[test]
    fn reconstitute_core_clause() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);
        let registry = ClauseRegistry::new();

        let clause = make_clause(CovenantPart::Core);
        let clause_id = clause.id;
        registry.register(clause).unwrap();

        let proposal = ReconstitutionProposal::new(
            ReconstitutionTrigger::InterpretiveAmbiguity("vague language in Section 5".into()),
            vec!["Core Art. 2 Section 5".into()],
            "Original clause text about protecting dignity.",
            "Strengthened text with explicit algorithmic safeguards for dignity and consent",
            "Current language does not address algorithmic harms",
            make_axiom_alignment(),
            vec!["privacy_council".into(), "tech_assembly".into()],
        );

        let ratification = make_reconstitution_ratification();

        registry
            .reconstitute(clause_id, &proposal, &reviewer, &ratification, true)
            .unwrap();

        let updated = registry.get(&clause_id).unwrap().unwrap();
        assert!(updated.text.contains("algorithmic safeguards"));
        assert_eq!(updated.amended_history.len(), 1);
    }

    #[test]
    fn reconstitute_non_core_clause_rejected() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);
        let registry = ClauseRegistry::new();

        let clause = make_clause(CovenantPart::Consortium);
        let clause_id = clause.id;
        registry.register(clause).unwrap();

        let proposal = ReconstitutionProposal::new(
            ReconstitutionTrigger::InterpretiveAmbiguity("vague".into()),
            vec!["Art. 1".into()],
            "Original",
            "Proposed",
            "Rationale",
            make_axiom_alignment(),
            vec!["community".into()],
        );

        let ratification = make_reconstitution_ratification();
        let result =
            registry.reconstitute(clause_id, &proposal, &reviewer, &ratification, true);
        assert!(matches!(
            result,
            Err(PolityError::ClauseDoesNotRequireReconstitution(_))
        ));
    }

    #[test]
    fn reconstitute_fails_without_star_court() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);
        let registry = ClauseRegistry::new();

        let clause = make_clause(CovenantPart::Core);
        let clause_id = clause.id;
        registry.register(clause).unwrap();

        let proposal = ReconstitutionProposal::new(
            ReconstitutionTrigger::InterpretiveAmbiguity("vague".into()),
            vec!["Core Art. 2".into()],
            "Original",
            "Strengthened protections for consent and dignity",
            "Rationale",
            make_axiom_alignment(),
            vec!["community".into()],
        );

        let ratification = make_reconstitution_ratification();
        let result =
            registry.reconstitute(clause_id, &proposal, &reviewer, &ratification, false);
        assert!(matches!(
            result,
            Err(PolityError::ReconstitutionThresholdNotMet)
        ));
    }

    #[test]
    fn reconstitute_rejects_weakening_proposal() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);
        let registry = ClauseRegistry::new();

        let clause = make_clause(CovenantPart::Core);
        let clause_id = clause.id;
        registry.register(clause).unwrap();

        let proposal = ReconstitutionProposal::new(
            ReconstitutionTrigger::PublicInvocation(vec!["bad_community".into()]),
            vec!["Core Art. 5".into()],
            "No surveillance permitted",
            "Permit surveillance during declared emergencies",
            "We need surveillance",
            make_axiom_alignment(),
            vec!["fearful_community".into()],
        );

        let ratification = make_reconstitution_ratification();
        let result =
            registry.reconstitute(clause_id, &proposal, &reviewer, &ratification, true);
        assert!(matches!(
            result,
            Err(PolityError::ReconstitutionWeakensAxiom(_))
        ));
    }

    // -----------------------------------------------------------------------
    // Amendment Threshold hierarchy
    // -----------------------------------------------------------------------

    #[test]
    fn amendment_threshold_ratios() {
        assert_eq!(
            AmendmentThreshold::required_ratio(CovenantPart::Core),
            0.90
        );
        assert_eq!(
            AmendmentThreshold::required_ratio(CovenantPart::Commons),
            0.90
        );
        assert_eq!(
            AmendmentThreshold::required_ratio(CovenantPart::Coexistence),
            0.75
        );
        assert_eq!(
            AmendmentThreshold::required_ratio(CovenantPart::Conjunction),
            0.75
        );
        assert_eq!(
            AmendmentThreshold::required_ratio(CovenantPart::Consortium),
            0.60
        );
        assert_eq!(
            AmendmentThreshold::required_ratio(CovenantPart::Compact),
            0.60
        );
    }

    #[test]
    fn amendment_threshold_deliberation_days() {
        assert_eq!(
            AmendmentThreshold::min_deliberation_days(CovenantPart::Core),
            730
        );
        assert_eq!(
            AmendmentThreshold::min_deliberation_days(CovenantPart::Coexistence),
            270
        );
        assert_eq!(
            AmendmentThreshold::min_deliberation_days(CovenantPart::Consortium),
            180
        );
    }

    #[test]
    fn amendment_threshold_met_for_parts_05_09() {
        let record = make_ratification_met(CovenantPart::Consortium);
        assert!(AmendmentThreshold::is_met(CovenantPart::Consortium, &record));
    }

    #[test]
    fn amendment_threshold_not_met_insufficient_time() {
        let started = Utc::now() - TimeDelta::days(30); // way too short
        let record = RatificationRecord {
            communities_for: 80,
            communities_against: 20,
            communities_total: 100,
            threshold_met: true,
            deliberation_started: started,
            deliberation_ended: Utc::now(),
        };
        assert!(!AmendmentThreshold::is_met(
            CovenantPart::Coexistence,
            &record
        ));
    }

    // -----------------------------------------------------------------------
    // CovenantPart
    // -----------------------------------------------------------------------

    #[test]
    fn covenant_part_requires_reconstitution() {
        assert!(CovenantPart::Core.requires_reconstitution());
        assert!(CovenantPart::Commons.requires_reconstitution());
        assert!(!CovenantPart::Coexistence.requires_reconstitution());
        assert!(!CovenantPart::Consortium.requires_reconstitution());
        assert!(!CovenantPart::Compact.requires_reconstitution());
    }

    // -----------------------------------------------------------------------
    // Layer 3: Precedent Registry
    // -----------------------------------------------------------------------

    #[test]
    fn record_precedent() {
        let registry = PrecedentRegistry::new();
        let precedent = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Dignity".into()),
            "Algorithmic systems must not rank human worth",
            "Dignity is inherent; systems that compute relative worth violate it",
            "tech_community",
            vec!["judge_a".into(), "judge_b".into()],
        );

        let id = registry.record(precedent).unwrap();
        let stored = registry.get(&id).unwrap().unwrap();
        assert_eq!(stored.adoption_count, 1);
        assert!(stored.is_active());
    }

    #[test]
    fn precedent_for_principle() {
        let registry = PrecedentRegistry::new();
        let principle = PrincipleReference::Right(RightCategory::Privacy);

        let p1 = CovenantPrecedent::new(
            Uuid::new_v4(),
            principle.clone(),
            "Metadata is personal data",
            "Pattern analysis of metadata reveals private information",
            "privacy_community",
            vec!["judge_a".into()],
        );
        let p2 = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Consent".into()),
            "Different principle",
            "Reasoning",
            "other_community",
            vec!["judge_b".into()],
        );

        registry.record(p1).unwrap();
        registry.record(p2).unwrap();

        let privacy_precedents = registry.for_principle(&principle).unwrap();
        assert_eq!(privacy_precedents.len(), 1);
        assert!(privacy_precedents[0]
            .interpretation
            .contains("Metadata"));
    }

    #[test]
    fn adopt_precedent() {
        let registry = PrecedentRegistry::new();
        let precedent = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Sovereignty".into()),
            "Data portability is a sovereignty right",
            "People must be able to leave platforms with their data",
            "tech_community",
            vec!["judge_a".into()],
        );

        let id = registry.record(precedent).unwrap();
        assert_eq!(registry.get(&id).unwrap().unwrap().adoption_count, 1);

        registry.adopt(id, "neighbor_community").unwrap();
        assert_eq!(registry.get(&id).unwrap().unwrap().adoption_count, 2);

        registry.adopt(id, "far_community").unwrap();
        assert_eq!(registry.get(&id).unwrap().unwrap().adoption_count, 3);
    }

    #[test]
    fn adopt_idempotent() {
        let registry = PrecedentRegistry::new();
        let precedent = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Dignity".into()),
            "Interp",
            "Reasoning",
            "community_a",
            vec!["judge".into()],
        );
        let id = registry.record(precedent).unwrap();

        registry.adopt(id, "community_a").unwrap();
        assert_eq!(registry.get(&id).unwrap().unwrap().adoption_count, 1);
    }

    #[test]
    fn adopt_nonexistent_precedent_fails() {
        let registry = PrecedentRegistry::new();
        let result = registry.adopt(Uuid::new_v4(), "community");
        assert!(matches!(result, Err(PolityError::PrecedentNotFound(_))));
    }

    #[test]
    fn supersede_precedent() {
        let registry = PrecedentRegistry::new();
        let old = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Consent".into()),
            "Old interpretation",
            "Old reasoning",
            "community_a",
            vec!["judge_a".into()],
        );
        let new = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Consent".into()),
            "Updated interpretation",
            "Better reasoning",
            "community_a",
            vec!["judge_a".into(), "judge_b".into()],
        );

        let old_id = registry.record(old).unwrap();
        let new_id = registry.record(new).unwrap();

        registry.supersede(old_id, new_id).unwrap();

        let old_stored = registry.get(&old_id).unwrap().unwrap();
        assert_eq!(old_stored.superseded_by, Some(new_id));
        assert!(!old_stored.is_active());

        let new_stored = registry.get(&new_id).unwrap().unwrap();
        assert!(new_stored.is_active());
    }

    #[test]
    fn supersede_nonexistent_fails() {
        let registry = PrecedentRegistry::new();
        let p = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Dignity".into()),
            "Interp",
            "Reasoning",
            "community",
            vec!["judge".into()],
        );
        let id = registry.record(p).unwrap();

        let result = registry.supersede(id, Uuid::new_v4());
        assert!(matches!(result, Err(PolityError::PrecedentNotFound(_))));
    }

    #[test]
    fn most_adopted() {
        let registry = PrecedentRegistry::new();

        let p1 = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Dignity".into()),
            "Dignity interpretation",
            "Reasoning",
            "community_a",
            vec!["judge".into()],
        );
        let p2 = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Consent".into()),
            "Consent interpretation",
            "Reasoning",
            "community_b",
            vec!["judge".into()],
        );

        let id1 = registry.record(p1).unwrap();
        let id2 = registry.record(p2).unwrap();

        // p1 gets more adoptions
        registry.adopt(id1, "community_c").unwrap();
        registry.adopt(id1, "community_d").unwrap();
        registry.adopt(id2, "community_e").unwrap();

        let top = registry.most_adopted(2).unwrap();
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].id, id1); // 3 adoptions
        assert_eq!(top[1].id, id2); // 2 adoptions
    }

    #[test]
    fn most_adopted_excludes_superseded() {
        let registry = PrecedentRegistry::new();

        let old = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Dignity".into()),
            "Old",
            "Reasoning",
            "community",
            vec!["judge".into()],
        );
        let new = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Dignity".into()),
            "New",
            "Better reasoning",
            "community",
            vec!["judge".into()],
        );

        let old_id = registry.record(old).unwrap();
        // Adopt old many times
        registry.adopt(old_id, "c1").unwrap();
        registry.adopt(old_id, "c2").unwrap();
        registry.adopt(old_id, "c3").unwrap();

        let new_id = registry.record(new).unwrap();
        registry.supersede(old_id, new_id).unwrap();

        let top = registry.most_adopted(5).unwrap();
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].id, new_id);
    }

    #[test]
    fn adopted_by_community() {
        let registry = PrecedentRegistry::new();

        let p1 = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Dignity".into()),
            "Interp 1",
            "Reasoning",
            "community_a",
            vec!["judge".into()],
        );
        let p2 = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Sovereignty".into()),
            "Interp 2",
            "Reasoning",
            "community_b",
            vec!["judge".into()],
        );

        let id1 = registry.record(p1).unwrap();
        let _id2 = registry.record(p2).unwrap();

        // community_x adopts p1
        registry.adopt(id1, "community_x").unwrap();

        let adopted = registry.adopted_by("community_x").unwrap();
        assert_eq!(adopted.len(), 1);
        assert_eq!(adopted[0].id, id1);

        // community_a is the originator so it auto-adopted
        let by_a = registry.adopted_by("community_a").unwrap();
        assert_eq!(by_a.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Precedent Search
    // -----------------------------------------------------------------------

    #[test]
    fn search_by_principle() {
        let registry = PrecedentRegistry::new();
        let target = PrincipleReference::Prohibition(ProhibitionType::Surveillance);

        let p1 = CovenantPrecedent::new(
            Uuid::new_v4(),
            target.clone(),
            "Metadata surveillance counts",
            "Reasoning",
            "community",
            vec!["judge".into()],
        );
        let p2 = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Dignity".into()),
            "Other",
            "Reasoning",
            "community",
            vec!["judge".into()],
        );

        registry.record(p1).unwrap();
        registry.record(p2).unwrap();

        let query = PrecedentSearch::new().by_principle(target);
        let results = registry.search(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].interpretation.contains("Metadata"));
    }

    #[test]
    fn search_by_adoption_count() {
        let registry = PrecedentRegistry::new();

        let p1 = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Dignity".into()),
            "Well adopted",
            "Reasoning",
            "community_a",
            vec!["judge".into()],
        );
        let p2 = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Axiom("Consent".into()),
            "Less adopted",
            "Reasoning",
            "community_b",
            vec!["judge".into()],
        );

        let id1 = registry.record(p1).unwrap();
        let _id2 = registry.record(p2).unwrap();

        registry.adopt(id1, "c1").unwrap();
        registry.adopt(id1, "c2").unwrap();

        let query = PrecedentSearch::new().by_adoption_count_min(3);
        let results = registry.search(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].interpretation.contains("Well adopted"));
    }

    #[test]
    fn search_combined_criteria() {
        let registry = PrecedentRegistry::new();
        let principle = PrincipleReference::Axiom("Dignity".into());

        let p1 = CovenantPrecedent::new(
            Uuid::new_v4(),
            principle.clone(),
            "Dignity in algorithms",
            "Reasoning",
            "tech_community",
            vec!["judge".into()],
        );
        let p2 = CovenantPrecedent::new(
            Uuid::new_v4(),
            principle.clone(),
            "Dignity in education",
            "Reasoning",
            "education_community",
            vec!["judge".into()],
        );

        let id1 = registry.record(p1).unwrap();
        let _id2 = registry.record(p2).unwrap();

        registry.adopt(id1, "neighbor").unwrap();
        registry.adopt(id1, "another").unwrap();

        let query = PrecedentSearch::new()
            .by_principle(principle)
            .by_adoption_count_min(2);
        let results = registry.search(&query).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].interpretation.contains("algorithms"));
    }

    // -----------------------------------------------------------------------
    // Serialization
    // -----------------------------------------------------------------------

    #[test]
    fn reconstitution_proposal_serialization() {
        let proposal = ReconstitutionProposal::new(
            ReconstitutionTrigger::SustainedBreach(vec![Uuid::new_v4()]),
            vec!["Core Art. 2".into()],
            "Original",
            "Proposed",
            "Rationale",
            make_axiom_alignment(),
            vec!["community".into()],
        );

        let json = serde_json::to_string(&proposal).unwrap();
        let restored: ReconstitutionProposal = serde_json::from_str(&json).unwrap();
        assert_eq!(proposal.id, restored.id);
        assert_eq!(proposal.proposed_text, restored.proposed_text);
        assert_eq!(proposal.axiom_alignment, restored.axiom_alignment);
    }

    #[test]
    fn clause_serialization() {
        let clause = make_clause(CovenantPart::Coexistence);
        let json = serde_json::to_string(&clause).unwrap();
        let restored: ConstitutionalClause = serde_json::from_str(&json).unwrap();
        assert_eq!(clause.id, restored.id);
        assert_eq!(clause.part, restored.part);
        assert_eq!(clause.text, restored.text);
    }

    #[test]
    fn precedent_serialization() {
        let precedent = CovenantPrecedent::new(
            Uuid::new_v4(),
            PrincipleReference::Duty(DutyCategory::UpholdDignity),
            "Interpretation of uphold-dignity duty",
            "Reasoning about proactive obligation",
            "justice_community",
            vec!["judge_a".into(), "judge_b".into()],
        );

        let json = serde_json::to_string(&precedent).unwrap();
        let restored: CovenantPrecedent = serde_json::from_str(&json).unwrap();
        assert_eq!(precedent.id, restored.id);
        assert_eq!(precedent.interpretation, restored.interpretation);
        assert_eq!(
            precedent.principle_interpreted,
            restored.principle_interpreted
        );
    }

    #[test]
    fn ratification_record_serialization() {
        let record = make_reconstitution_ratification();
        let json = serde_json::to_string(&record).unwrap();
        let restored: RatificationRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.communities_for, restored.communities_for);
        assert_eq!(record.communities_total, restored.communities_total);
    }

    // -----------------------------------------------------------------------
    // Thread safety
    // -----------------------------------------------------------------------

    #[test]
    fn clause_registry_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ClauseRegistry>();
    }

    #[test]
    fn precedent_registry_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PrecedentRegistry>();
    }
}
