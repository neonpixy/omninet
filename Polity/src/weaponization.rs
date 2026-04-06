//! # Anti-Weaponization Constraints (R1C)
//!
//! Specific architectural constraints that prevent bad-faith use of Covenant principles.
//! The Covenant's three axioms (Dignity, Sovereignty, Consent) are powerful — and like
//! any powerful tool, they can be turned against the people they were meant to protect.
//!
//! This module defines **when** a Covenant principle CANNOT be invoked, and provides
//! a check function that validates rights invocations against these constraints.
//!
//! ## The Four Constraints
//!
//! - **ConsentVetoLimit** — Consent applies to actions taken UPON you, not actions taken
//!   BY your community. A single person cannot veto a properly conducted collective decision
//!   that doesn't affect individual rights.
//!
//! - **SovereigntyInteropFloor** — You can choose not to participate, but you cannot break
//!   the protocol for others. Globe relay peering and Equipment message routing are
//!   non-negotiable protocol mechanics.
//!
//! - **DignitySpecificHarm** — "I find this undignified" is not a valid Dignity claim without
//!   identifying who is harmed and how. Prevents Dignity from becoming content censorship.
//!
//! - **RightsNotShields** — Rights protect individuals from power. They cannot be invoked by
//!   the powerful against the vulnerable (e.g., a leader using "rights" to dodge accountability).
//!
//! ## Integration
//!
//! `ConstitutionalReviewer::review()` can call `InvocationCheck::check()` when rights are
//! invoked. Weaponized invocations are rejected with a specific reason, not a generic denial.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::breach::BreachSeverity;
use crate::rights::RightCategory;

// ---------------------------------------------------------------------------
// Constraints
// ---------------------------------------------------------------------------

/// An architectural constraint on when a Covenant principle CANNOT be invoked.
///
/// Each variant encodes a specific anti-weaponization rule derived from the
/// Covenant's design intent: principles protect the vulnerable, not the powerful.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum InvocationConstraint {
    /// Consent cannot be invoked to unilaterally veto collective decisions that
    /// don't affect individual rights. Consent applies to actions taken UPON you,
    /// not actions taken BY your community.
    ConsentVetoLimit,

    /// Sovereignty cannot be invoked to refuse basic protocol interoperability
    /// (Globe relay peering, Equipment message routing). You can choose not to
    /// participate in a community, but you cannot break the protocol for others.
    SovereigntyInteropFloor,

    /// Dignity claims require identification of specific harm to a specific person.
    /// "I find this undignified" without identifying who is harmed and how is not
    /// a valid Dignity claim. Prevents Dignity from being used as content censorship.
    DignitySpecificHarm,

    /// Rights protect individuals from power. They cannot be invoked by the powerful
    /// against the vulnerable — e.g., a community leader invoking "rights" to avoid
    /// accountability for their actions in a position of power.
    RightsNotShields,
}

impl InvocationConstraint {
    /// All defined constraints, in evaluation order.
    pub const ALL: &[InvocationConstraint] = &[
        InvocationConstraint::ConsentVetoLimit,
        InvocationConstraint::SovereigntyInteropFloor,
        InvocationConstraint::DignitySpecificHarm,
        InvocationConstraint::RightsNotShields,
    ];

    /// Human-readable description of this constraint.
    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::ConsentVetoLimit => {
                "Consent cannot unilaterally veto collective decisions that don't affect individual rights"
            }
            Self::SovereigntyInteropFloor => {
                "Sovereignty cannot refuse basic protocol interoperability"
            }
            Self::DignitySpecificHarm => {
                "Dignity claims require specific harm to a specific person"
            }
            Self::RightsNotShields => {
                "Rights protect individuals from power, not powerful from accountability"
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Invocation types
// ---------------------------------------------------------------------------

/// A claim that a Covenant right is being invoked — the input to anti-weaponization checks.
///
/// When someone says "I invoke my right to X to prevent action Y," this struct captures
/// that claim so it can be validated against the anti-weaponization constraints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RightInvocation {
    /// The public key of the person invoking the right.
    pub invoker_pubkey: String,
    /// Which category of right is being invoked.
    pub right_category: RightCategory,
    /// What action the invoker wants to prevent, permit, or justify.
    pub target_action: String,
    /// The harm being claimed, if any. Required for Dignity claims.
    pub claimed_harm: Option<HarmClaim>,
    /// Context about where and how the invocation is happening.
    pub context: InvocationContext,
}

impl RightInvocation {
    /// Create a new right invocation.
    pub fn new(
        invoker_pubkey: impl Into<String>,
        right_category: RightCategory,
        target_action: impl Into<String>,
        context: InvocationContext,
    ) -> Self {
        Self {
            invoker_pubkey: invoker_pubkey.into(),
            right_category,
            target_action: target_action.into(),
            claimed_harm: None,
            context,
        }
    }

    /// Attach a harm claim to this invocation.
    #[must_use]
    pub fn with_harm(mut self, harm: HarmClaim) -> Self {
        self.claimed_harm = Some(harm);
        self
    }
}

/// A claim of specific harm, required for Dignity invocations and strengthening
/// evidence for other invocation types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarmClaim {
    /// The person or entity harmed. Must be specific — not "everyone" or "society."
    pub affected_party: String,
    /// Description of the harm experienced or threatened.
    pub harm_description: String,
    /// Hashes of evidence supporting the claim (can be empty for initial invocation).
    pub evidence_hashes: Vec<String>,
    /// How severe the claimed harm is.
    pub severity: BreachSeverity,
}

impl HarmClaim {
    /// Create a new harm claim.
    pub fn new(
        affected_party: impl Into<String>,
        harm_description: impl Into<String>,
        severity: BreachSeverity,
    ) -> Self {
        Self {
            affected_party: affected_party.into(),
            harm_description: harm_description.into(),
            evidence_hashes: Vec::new(),
            severity,
        }
    }

    /// Attach evidence hashes to this claim.
    #[must_use]
    pub fn with_evidence(mut self, hashes: Vec<String>) -> Self {
        self.evidence_hashes = hashes;
        self
    }

    /// Whether the affected party is vague or unspecified.
    fn is_vague_party(&self) -> bool {
        let lower = self.affected_party.to_lowercase();
        let vague_terms = [
            "everyone",
            "society",
            "the community",
            "the public",
            "all people",
            "general public",
            "nobody specific",
            "unspecified",
        ];
        vague_terms.iter().any(|term| lower == *term)
            || self.affected_party.trim().is_empty()
    }

    /// Whether the harm description is substantive (not empty or trivially vague).
    fn is_substantive_harm(&self) -> bool {
        let trimmed = self.harm_description.trim();
        if trimmed.is_empty() || trimmed.len() < 10 {
            return false;
        }
        let vague_only = [
            "i find this undignified",
            "this is undignified",
            "not dignified",
            "offensive",
            "i don't like it",
            "inappropriate",
            "distasteful",
        ];
        let lower = trimmed.to_lowercase();
        !vague_only.iter().any(|v| lower == *v)
    }
}

/// Context surrounding a rights invocation — used to detect power dynamics
/// and community-level decision patterns.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvocationContext {
    /// The community where this invocation is happening, if any.
    pub community_id: Option<String>,
    /// The specific decision being contested, if any.
    pub decision_id: Option<Uuid>,
    /// The role of the invoker in the relevant community. Used for
    /// `RightsNotShields` — leaders cannot use rights to dodge accountability.
    pub invoker_role: Option<String>,
}

impl InvocationContext {
    /// Create an empty context (no community, no decision, no role).
    pub fn empty() -> Self {
        Self {
            community_id: None,
            decision_id: None,
            invoker_role: None,
        }
    }

    /// Create a community context.
    pub fn community(community_id: impl Into<String>) -> Self {
        Self {
            community_id: Some(community_id.into()),
            decision_id: None,
            invoker_role: None,
        }
    }

    /// Attach a decision ID to this context.
    #[must_use]
    pub fn with_decision(mut self, decision_id: Uuid) -> Self {
        self.decision_id = Some(decision_id);
        self
    }

    /// Attach the invoker's role to this context.
    #[must_use]
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.invoker_role = Some(role.into());
        self
    }

    /// Whether the invoker holds a leadership or authority role.
    fn is_authority_role(&self) -> bool {
        match &self.invoker_role {
            None => false,
            Some(role) => {
                let lower = role.to_lowercase();
                let authority_signals = [
                    "admin",
                    "administrator",
                    "leader",
                    "moderator",
                    "founder",
                    "owner",
                    "steward",
                    "governor",
                    "chair",
                    "director",
                    "manager",
                    "authority",
                ];
                authority_signals.iter().any(|s| lower.contains(s))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------------

/// The outcome of an anti-weaponization check on a rights invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InvocationResult {
    /// The invocation is valid — the right is being properly exercised.
    Valid(RightCategory),
    /// The invocation is rejected — it triggers a specific anti-weaponization constraint.
    Rejected(WeaponizationReason),
    /// The invocation needs human review — the constraints detect a pattern but
    /// cannot make a definitive determination.
    NeedsReview(String),
}

impl InvocationResult {
    /// Whether the invocation was accepted as valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, InvocationResult::Valid(_))
    }

    /// Whether the invocation was rejected as weaponized.
    #[must_use]
    pub fn is_rejected(&self) -> bool {
        matches!(self, InvocationResult::Rejected(_))
    }

    /// Whether the invocation needs further review.
    #[must_use]
    pub fn is_needs_review(&self) -> bool {
        matches!(self, InvocationResult::NeedsReview(_))
    }
}

/// Why an invocation was rejected — maps 1:1 to the constraint that fired.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WeaponizationReason {
    /// Consent invoked to veto a collective decision that doesn't affect individual rights.
    VetoWithoutRightsImpact,
    /// Sovereignty invoked to refuse basic protocol interoperability.
    InteropRefusal,
    /// Dignity invoked without identifying specific harm to a specific person.
    NoSpecificHarm,
    /// Rights invoked by someone in power to shield themselves from accountability.
    PowerShielding,
}

impl WeaponizationReason {
    /// Human-readable explanation of why the invocation was rejected.
    #[must_use]
    pub fn explanation(&self) -> &'static str {
        match self {
            Self::VetoWithoutRightsImpact => {
                "Consent applies to actions taken upon you, not actions taken by your community. \
                 A single person cannot veto a properly conducted collective decision \
                 that does not affect individual rights."
            }
            Self::InteropRefusal => {
                "Sovereignty cannot be used to break the protocol for others. \
                 You may choose not to participate, but you cannot refuse basic \
                 protocol interoperability (relay peering, message routing)."
            }
            Self::NoSpecificHarm => {
                "Dignity claims require identification of specific harm to a specific person. \
                 General discomfort or aesthetic objection is not sufficient. \
                 Who is harmed, and how?"
            }
            Self::PowerShielding => {
                "Rights protect individuals from power, not the powerful from accountability. \
                 A person in an authority role cannot invoke rights to avoid legitimate \
                 scrutiny of their exercise of that authority."
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The check function
// ---------------------------------------------------------------------------

/// Anti-weaponization check engine.
///
/// Validates that a rights invocation is genuine — not a bad-faith use of Covenant
/// principles to block legitimate collective action, break protocol interoperability,
/// censor content, or shield the powerful from accountability.
pub struct InvocationCheck;

impl InvocationCheck {
    /// Check a rights invocation against all anti-weaponization constraints.
    ///
    /// Returns `Valid` if the invocation is a proper exercise of rights,
    /// `Rejected` if it triggers a specific constraint, or `NeedsReview`
    /// if the situation is ambiguous.
    ///
    /// # Examples
    ///
    /// ```
    /// use polity::weaponization::*;
    /// use polity::rights::RightCategory;
    /// use polity::breach::BreachSeverity;
    ///
    /// // A legitimate privacy invocation
    /// let invocation = RightInvocation::new(
    ///     "pubkey_alice",
    ///     RightCategory::Privacy,
    ///     "prevent data harvesting of my browsing history",
    ///     InvocationContext::empty(),
    /// ).with_harm(HarmClaim::new(
    ///     "pubkey_alice",
    ///     "My browsing data is being collected and sold without my knowledge",
    ///     BreachSeverity::Significant,
    /// ));
    ///
    /// let result = InvocationCheck::check(&invocation);
    /// assert!(result.is_valid());
    /// ```
    #[must_use]
    pub fn check(invocation: &RightInvocation) -> InvocationResult {
        // Check each constraint in order. First rejection wins.
        // RightsNotShields runs first — authority figures dodging accountability
        // is a stronger signal than consent veto patterns.

        // 1. RightsNotShields — Authority figure shielding from accountability?
        if let Some(result) = Self::check_rights_not_shields(invocation) {
            return result;
        }

        // 2. ConsentVetoLimit — Consent used to veto a collective decision?
        if let Some(result) = Self::check_consent_veto(invocation) {
            return result;
        }

        // 3. SovereigntyInteropFloor — Sovereignty used to break the protocol?
        if let Some(result) = Self::check_sovereignty_interop(invocation) {
            return result;
        }

        // 4. DignitySpecificHarm — Dignity invoked without specific harm?
        if let Some(result) = Self::check_dignity_specific_harm(invocation) {
            return result;
        }

        // All constraints passed — the invocation is valid.
        InvocationResult::Valid(invocation.right_category)
    }

    /// Check ConsentVetoLimit: is Consent being used to unilaterally veto a
    /// collective decision that doesn't affect individual rights?
    fn check_consent_veto(invocation: &RightInvocation) -> Option<InvocationResult> {
        // Only applies to Consent-category invocations (or Refusal used as consent veto).
        if !matches!(
            invocation.right_category,
            RightCategory::Refusal | RightCategory::Community
        ) {
            return None;
        }

        // Must be targeting a collective decision (decision_id present).
        invocation.context.decision_id?;

        // If the invoker claims specific personal harm, the veto may be legitimate.
        if let Some(ref harm) = invocation.claimed_harm {
            if !harm.is_vague_party() && harm.is_substantive_harm() {
                // They have a specific, substantive claim — allow it or flag for review.
                return None;
            }
        }

        // Check if the target action clearly affects individual rights.
        if Self::action_affects_individual_rights(&invocation.target_action) {
            return None;
        }

        // Consent/Refusal invoked against a collective decision without demonstrating
        // personal rights impact — this is a veto attempt.
        Some(InvocationResult::Rejected(
            WeaponizationReason::VetoWithoutRightsImpact,
        ))
    }

    /// Check SovereigntyInteropFloor: is Sovereignty being used to refuse
    /// basic protocol interoperability?
    fn check_sovereignty_interop(invocation: &RightInvocation) -> Option<InvocationResult> {
        // Only applies to Sovereignty-adjacent categories.
        if !matches!(
            invocation.right_category,
            RightCategory::Refusal | RightCategory::Community
        ) {
            return None;
        }

        // Check if the target action is a protocol-level interoperability action.
        if !Self::is_interop_action(&invocation.target_action) {
            return None;
        }

        // If there is a legitimate safety concern (e.g., refusing to peer with
        // a relay that distributes harmful content), flag for review rather than
        // outright rejection.
        if let Some(ref harm) = invocation.claimed_harm {
            if harm.severity >= BreachSeverity::Significant && harm.is_substantive_harm() {
                return Some(InvocationResult::NeedsReview(format!(
                    "Sovereignty invoked against interop action '{}' with safety claim: {}. \
                     Review whether the safety concern justifies interop exception.",
                    invocation.target_action, harm.harm_description
                )));
            }
        }

        Some(InvocationResult::Rejected(
            WeaponizationReason::InteropRefusal,
        ))
    }

    /// Check DignitySpecificHarm: is Dignity being invoked without identifying
    /// specific harm to a specific person?
    fn check_dignity_specific_harm(invocation: &RightInvocation) -> Option<InvocationResult> {
        // Only applies to Dignity-category invocations.
        if invocation.right_category != RightCategory::Dignity {
            return None;
        }

        match &invocation.claimed_harm {
            None => {
                // Dignity invoked with no harm claim at all.
                Some(InvocationResult::Rejected(
                    WeaponizationReason::NoSpecificHarm,
                ))
            }
            Some(harm) => {
                if harm.is_vague_party() {
                    // "Everyone" or empty affected party — not specific enough.
                    Some(InvocationResult::Rejected(
                        WeaponizationReason::NoSpecificHarm,
                    ))
                } else if !harm.is_substantive_harm() {
                    // "This is undignified" without substance — not enough.
                    Some(InvocationResult::Rejected(
                        WeaponizationReason::NoSpecificHarm,
                    ))
                } else {
                    // Specific person, substantive harm — this is a legitimate claim.
                    None
                }
            }
        }
    }

    /// Check RightsNotShields: is someone in a position of authority using rights
    /// to avoid accountability for their exercise of power?
    fn check_rights_not_shields(invocation: &RightInvocation) -> Option<InvocationResult> {
        // Must be someone in an authority role.
        if !invocation.context.is_authority_role() {
            return None;
        }

        // Check if the target action is about accountability for their authority.
        if !Self::is_accountability_action(&invocation.target_action) {
            return None;
        }

        // An authority figure invoking rights against accountability for their own
        // exercise of power — this is power shielding.
        //
        // Exception: if they're invoking Privacy against a fishing expedition that
        // goes beyond their role, that's legitimate. Flag for review.
        if invocation.right_category == RightCategory::Privacy {
            if let Some(ref harm) = invocation.claimed_harm {
                if harm.is_substantive_harm() && !harm.is_vague_party() {
                    return Some(InvocationResult::NeedsReview(format!(
                        "Authority figure invoking Privacy against accountability action '{}'. \
                         Review whether the privacy claim is about personal life (legitimate) \
                         or about exercise of authority (power shielding).",
                        invocation.target_action
                    )));
                }
            }
        }

        Some(InvocationResult::Rejected(
            WeaponizationReason::PowerShielding,
        ))
    }

    // -----------------------------------------------------------------------
    // Heuristic helpers
    // -----------------------------------------------------------------------

    /// Whether a target action description suggests it affects individual rights
    /// (which would make a consent veto potentially legitimate).
    fn action_affects_individual_rights(action: &str) -> bool {
        let lower = action.to_lowercase();
        let rights_signals = [
            "personal data",
            "private information",
            "individual property",
            "personal identity",
            "bodily",
            "medical record",
            "private message",
            "personal belief",
            "individual right",
            "my data",
            "my identity",
            "my privacy",
            "surveillance of",
            "track individual",
            "forced participation",
            "compel",
            "coerce",
        ];
        rights_signals.iter().any(|signal| lower.contains(signal))
    }

    /// Whether a target action is a protocol-level interoperability function.
    fn is_interop_action(action: &str) -> bool {
        let lower = action.to_lowercase();
        let interop_signals = [
            "relay peering",
            "message routing",
            "protocol interop",
            "globe relay",
            "equipment routing",
            "network peering",
            "protocol compliance",
            "relay connection",
            "message delivery",
            "protocol handshake",
            "relay federation",
            "pact routing",
        ];
        interop_signals.iter().any(|signal| lower.contains(signal))
    }

    /// Whether a target action is about accountability for exercise of authority.
    fn is_accountability_action(action: &str) -> bool {
        let lower = action.to_lowercase();
        let accountability_signals = [
            "audit",
            "review of decision",
            "accountability",
            "investigation",
            "transparency report",
            "governance review",
            "moderation review",
            "decision review",
            "conduct review",
            "power audit",
            "authority review",
            "leadership review",
            "impeach",
            "recall",
            "no confidence",
        ];
        accountability_signals.iter().any(|signal| lower.contains(signal))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::breach::BreachSeverity;
    use crate::rights::RightCategory;

    // ===== Helpers =====

    fn community_context() -> InvocationContext {
        InvocationContext::community("community_alpha")
    }

    fn community_with_decision() -> InvocationContext {
        InvocationContext::community("community_alpha").with_decision(Uuid::new_v4())
    }

    fn leader_context() -> InvocationContext {
        InvocationContext::community("community_alpha")
            .with_decision(Uuid::new_v4())
            .with_role("community_leader")
    }

    fn specific_harm() -> HarmClaim {
        HarmClaim::new(
            "pubkey_bob",
            "Bob's medical records were exposed in the community decision database",
            BreachSeverity::Grave,
        )
    }

    fn vague_harm() -> HarmClaim {
        HarmClaim::new(
            "everyone",
            "This is undignified",
            BreachSeverity::Minor,
        )
    }

    // ===== InvocationConstraint =====

    #[test]
    fn constraint_all_contains_four() {
        assert_eq!(InvocationConstraint::ALL.len(), 4);
    }

    #[test]
    fn constraint_descriptions_are_nonempty() {
        for c in InvocationConstraint::ALL {
            assert!(!c.description().is_empty());
        }
    }

    #[test]
    fn constraint_serialization_roundtrip() {
        for c in InvocationConstraint::ALL {
            let json = serde_json::to_string(c).unwrap();
            let restored: InvocationConstraint = serde_json::from_str(&json).unwrap();
            assert_eq!(*c, restored);
        }
    }

    // ===== ConsentVetoLimit =====

    #[test]
    fn consent_veto_rejected_when_no_personal_rights_impact() {
        // Someone invoking Refusal to block a community garden vote.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Refusal,
            "block community vote on park renovation budget",
            community_with_decision(),
        );

        let result = InvocationCheck::check(&invocation);
        assert_eq!(
            result,
            InvocationResult::Rejected(WeaponizationReason::VetoWithoutRightsImpact)
        );
    }

    #[test]
    fn consent_veto_valid_when_personal_rights_affected() {
        // Someone invoking Refusal because a community decision exposes their personal data.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Refusal,
            "block vote that requires sharing personal data of all members",
            community_with_decision(),
        )
        .with_harm(specific_harm());

        let result = InvocationCheck::check(&invocation);
        assert!(result.is_valid());
    }

    #[test]
    fn consent_veto_valid_when_action_affects_individual_rights() {
        // The target action itself signals individual rights impact.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Refusal,
            "block forced participation in community surveillance program",
            community_with_decision(),
        );

        let result = InvocationCheck::check(&invocation);
        assert!(result.is_valid());
    }

    #[test]
    fn consent_veto_ignored_for_non_consent_categories() {
        // Privacy invocation is never checked by ConsentVetoLimit.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Privacy,
            "block community vote on park renovation budget",
            community_with_decision(),
        );

        let result = InvocationCheck::check(&invocation);
        // Not rejected by ConsentVetoLimit — Privacy doesn't trigger it.
        assert!(result.is_valid());
    }

    #[test]
    fn consent_veto_not_triggered_without_decision_id() {
        // No decision_id means this isn't a collective decision veto.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Refusal,
            "refuse to accept new community policy",
            community_context(),
        );

        let result = InvocationCheck::check(&invocation);
        assert!(result.is_valid());
    }

    // ===== SovereigntyInteropFloor =====

    #[test]
    fn interop_refusal_rejected() {
        // Community invoking Refusal to block relay peering.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Refusal,
            "refuse globe relay peering with neighboring community",
            community_context(),
        );

        let result = InvocationCheck::check(&invocation);
        assert_eq!(
            result,
            InvocationResult::Rejected(WeaponizationReason::InteropRefusal)
        );
    }

    #[test]
    fn interop_refusal_needs_review_with_safety_concern() {
        // Community refusing relay peering with a relay distributing harmful content.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Refusal,
            "refuse globe relay peering with relay distributing exploitation material",
            community_context(),
        )
        .with_harm(HarmClaim::new(
            "children in the community",
            "The relay actively distributes child exploitation material and our community has minors",
            BreachSeverity::Existential,
        ));

        let result = InvocationCheck::check(&invocation);
        assert!(result.is_needs_review());
    }

    #[test]
    fn interop_refusal_valid_for_non_interop_actions() {
        // Refusing to join a community is fine — that's real sovereignty.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Refusal,
            "refuse to join community governance council",
            community_context(),
        );

        let result = InvocationCheck::check(&invocation);
        assert!(result.is_valid());
    }

    #[test]
    fn interop_message_routing_rejected() {
        // Blocking Equipment message routing is an interop violation.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Community,
            "block equipment routing for messages from outside community",
            community_context(),
        );

        let result = InvocationCheck::check(&invocation);
        assert_eq!(
            result,
            InvocationResult::Rejected(WeaponizationReason::InteropRefusal)
        );
    }

    // ===== DignitySpecificHarm =====

    #[test]
    fn dignity_rejected_without_harm_claim() {
        // Dignity invoked with no harm claim at all.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Dignity,
            "remove community artwork I find offensive",
            InvocationContext::empty(),
        );

        let result = InvocationCheck::check(&invocation);
        assert_eq!(
            result,
            InvocationResult::Rejected(WeaponizationReason::NoSpecificHarm)
        );
    }

    #[test]
    fn dignity_rejected_with_vague_harm() {
        // Dignity invoked with "everyone" as affected party and vague description.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Dignity,
            "censor community discussion board content",
            InvocationContext::empty(),
        )
        .with_harm(vague_harm());

        let result = InvocationCheck::check(&invocation);
        assert_eq!(
            result,
            InvocationResult::Rejected(WeaponizationReason::NoSpecificHarm)
        );
    }

    #[test]
    fn dignity_valid_with_specific_harm() {
        // Dignity invoked with a specific person and substantive harm description.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Dignity,
            "remove content that publicly shames a specific person by name",
            InvocationContext::empty(),
        )
        .with_harm(HarmClaim::new(
            "pubkey_bob",
            "Content uses Bob's real name and medical condition to humiliate him publicly",
            BreachSeverity::Grave,
        ));

        let result = InvocationCheck::check(&invocation);
        assert!(result.is_valid());
    }

    #[test]
    fn dignity_rejected_with_empty_harm_description() {
        // Specific party but trivially short description.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Dignity,
            "remove content",
            InvocationContext::empty(),
        )
        .with_harm(HarmClaim::new(
            "pubkey_bob",
            "bad",
            BreachSeverity::Minor,
        ));

        let result = InvocationCheck::check(&invocation);
        assert_eq!(
            result,
            InvocationResult::Rejected(WeaponizationReason::NoSpecificHarm)
        );
    }

    #[test]
    fn dignity_constraint_does_not_apply_to_other_categories() {
        // Safety invocation without harm claim should still pass (DignitySpecificHarm
        // only applies to Dignity category).
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Safety,
            "request protection from threatening messages",
            InvocationContext::empty(),
        );

        let result = InvocationCheck::check(&invocation);
        assert!(result.is_valid());
    }

    // ===== RightsNotShields =====

    #[test]
    fn power_shielding_rejected() {
        // Community leader invoking rights to block an audit of their decisions.
        let invocation = RightInvocation::new(
            "pubkey_leader",
            RightCategory::Refusal,
            "refuse governance audit of my moderation decisions",
            leader_context(),
        );

        let result = InvocationCheck::check(&invocation);
        assert_eq!(
            result,
            InvocationResult::Rejected(WeaponizationReason::PowerShielding)
        );
    }

    #[test]
    fn power_shielding_needs_review_for_privacy_with_substance() {
        // Leader invoking privacy against an investigation, but with a substantive
        // claim that the investigation is accessing their personal life.
        let invocation = RightInvocation::new(
            "pubkey_leader",
            RightCategory::Privacy,
            "refuse investigation into my personal messages",
            leader_context(),
        )
        .with_harm(HarmClaim::new(
            "pubkey_leader",
            "The investigation accessed my personal family conversations that have nothing to do with governance",
            BreachSeverity::Significant,
        ));

        let result = InvocationCheck::check(&invocation);
        assert!(result.is_needs_review());
    }

    #[test]
    fn power_shielding_rejected_for_privacy_without_substance() {
        // Leader invoking privacy against an audit but without substance.
        let invocation = RightInvocation::new(
            "pubkey_leader",
            RightCategory::Privacy,
            "refuse accountability review of governance decisions",
            leader_context(),
        );

        let result = InvocationCheck::check(&invocation);
        assert_eq!(
            result,
            InvocationResult::Rejected(WeaponizationReason::PowerShielding)
        );
    }

    #[test]
    fn non_authority_not_flagged_for_power_shielding() {
        // Regular member invoking rights against an accountability action is fine —
        // RightsNotShields only applies to authority figures.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Refusal,
            "refuse governance audit of community spending",
            InvocationContext::community("community_alpha")
                .with_decision(Uuid::new_v4())
                .with_role("member"),
        );

        let result = InvocationCheck::check(&invocation);
        // Not power shielding because "member" is not an authority role.
        // But check ConsentVetoLimit — this is Refusal + decision_id.
        // Target action doesn't signal individual rights, so it may hit ConsentVetoLimit.
        // Actually, since "governance audit" is not a "personal data" etc. signal, this
        // will be rejected by ConsentVetoLimit.
        assert!(result.is_rejected());
        // But specifically NOT for PowerShielding.
        assert_ne!(
            result,
            InvocationResult::Rejected(WeaponizationReason::PowerShielding)
        );
    }

    #[test]
    fn authority_not_flagged_for_non_accountability_action() {
        // Leader invoking a right against something that isn't about accountability
        // for their authority — that's a normal invocation.
        let invocation = RightInvocation::new(
            "pubkey_leader",
            RightCategory::Privacy,
            "protect my personal medical records from community database",
            InvocationContext::community("community_alpha").with_role("admin"),
        );

        let result = InvocationCheck::check(&invocation);
        assert!(result.is_valid());
    }

    // ===== Edge cases =====

    #[test]
    fn legitimate_consent_refusal_for_personal_action() {
        // Consent is genuinely about an action taken upon you — not a veto.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Refusal,
            "refuse mandatory data collection by my community",
            InvocationContext::empty(),
        );

        let result = InvocationCheck::check(&invocation);
        assert!(result.is_valid());
    }

    #[test]
    fn legitimate_sovereignty_refusal_to_participate() {
        // Not refusing interop — just refusing to participate.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Community,
            "withdraw from community and revoke membership",
            community_context(),
        );

        let result = InvocationCheck::check(&invocation);
        assert!(result.is_valid());
    }

    #[test]
    fn legitimate_dignity_claim_with_evidence() {
        // Real harm, real person, with evidence.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Dignity,
            "remove deepfake content targeting specific individual",
            InvocationContext::empty(),
        )
        .with_harm(
            HarmClaim::new(
                "pubkey_carol",
                "Non-consensual deepfake images of Carol distributed across the community",
                BreachSeverity::Existential,
            )
            .with_evidence(vec![
                "sha256:abc123".to_string(),
                "sha256:def456".to_string(),
            ]),
        );

        let result = InvocationCheck::check(&invocation);
        assert!(result.is_valid());
    }

    #[test]
    fn expression_invocation_passes_all_constraints() {
        // Expression right invocations are not checked by any constraint.
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Expression,
            "publish community manifesto",
            InvocationContext::empty(),
        );

        let result = InvocationCheck::check(&invocation);
        assert_eq!(result, InvocationResult::Valid(RightCategory::Expression));
    }

    #[test]
    fn weaponization_reason_explanations_are_nonempty() {
        let reasons = [
            WeaponizationReason::VetoWithoutRightsImpact,
            WeaponizationReason::InteropRefusal,
            WeaponizationReason::NoSpecificHarm,
            WeaponizationReason::PowerShielding,
        ];
        for reason in &reasons {
            assert!(!reason.explanation().is_empty());
        }
    }

    #[test]
    fn invocation_result_predicates() {
        let valid = InvocationResult::Valid(RightCategory::Dignity);
        assert!(valid.is_valid());
        assert!(!valid.is_rejected());
        assert!(!valid.is_needs_review());

        let rejected = InvocationResult::Rejected(WeaponizationReason::NoSpecificHarm);
        assert!(!rejected.is_valid());
        assert!(rejected.is_rejected());
        assert!(!rejected.is_needs_review());

        let review = InvocationResult::NeedsReview("ambiguous case".into());
        assert!(!review.is_valid());
        assert!(!review.is_rejected());
        assert!(review.is_needs_review());
    }

    #[test]
    fn right_invocation_serialization_roundtrip() {
        let invocation = RightInvocation::new(
            "pubkey_alice",
            RightCategory::Dignity,
            "remove harmful content",
            InvocationContext::community("community_alpha").with_decision(Uuid::new_v4()),
        )
        .with_harm(
            HarmClaim::new(
                "pubkey_bob",
                "Bob was publicly shamed using his medical records",
                BreachSeverity::Grave,
            )
            .with_evidence(vec!["sha256:abc123".to_string()]),
        );

        let json = serde_json::to_string(&invocation).unwrap();
        let restored: RightInvocation = serde_json::from_str(&json).unwrap();
        assert_eq!(invocation, restored);
    }

    #[test]
    fn invocation_result_serialization_roundtrip() {
        let results = vec![
            InvocationResult::Valid(RightCategory::Privacy),
            InvocationResult::Rejected(WeaponizationReason::InteropRefusal),
            InvocationResult::NeedsReview("complex case".into()),
        ];
        for result in &results {
            let json = serde_json::to_string(result).unwrap();
            let restored: InvocationResult = serde_json::from_str(&json).unwrap();
            assert_eq!(*result, restored);
        }
    }

    #[test]
    fn invocation_context_builder_pattern() {
        let ctx = InvocationContext::community("alpha")
            .with_decision(Uuid::nil())
            .with_role("moderator");

        assert_eq!(ctx.community_id.as_deref(), Some("alpha"));
        assert_eq!(ctx.decision_id, Some(Uuid::nil()));
        assert_eq!(ctx.invoker_role.as_deref(), Some("moderator"));
        assert!(ctx.is_authority_role());
    }

    #[test]
    fn empty_context_has_no_authority() {
        let ctx = InvocationContext::empty();
        assert!(!ctx.is_authority_role());
        assert!(ctx.community_id.is_none());
        assert!(ctx.decision_id.is_none());
        assert!(ctx.invoker_role.is_none());
    }

    #[test]
    fn harm_claim_vague_detection() {
        let vague = HarmClaim::new("everyone", "This is undignified", BreachSeverity::Minor);
        assert!(vague.is_vague_party());
        assert!(!vague.is_substantive_harm());

        let specific = HarmClaim::new(
            "pubkey_bob",
            "Bob received death threats referencing his home address",
            BreachSeverity::Existential,
        );
        assert!(!specific.is_vague_party());
        assert!(specific.is_substantive_harm());
    }
}
