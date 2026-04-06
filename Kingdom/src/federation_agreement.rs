//! Bilateral federation agreements between communities.
//!
//! From Constellation Art. 3 §3: "Communities may enter into federated
//! agreements to share governance, coordinate resource stewardship, or
//! pursue common purpose. Such federations shall remain voluntary,
//! revocable, and rooted in consent."
//!
//! FederationAgreement is the bilateral building block. For multilateral
//! federation, use Consortium (in federation.rs).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

use crate::error::KingdomError;

/// What a federation agreement covers.
///
/// Scopes are additive — an agreement can have multiple.
/// No Custom variant: custom obligations belong in Treaty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FederationScope {
    /// Recognize each other's identity, governance, and membership
    MutualRecognition,
    /// Propagate Gospel events between community Towers
    GospelPeering,
    /// Share data according to agreed standards
    DataSharing,
    /// Coordinate governance decisions on shared concerns
    GovernanceCoordination,
    /// Economic cooperation (Cool exchange, marketplace access)
    EconomicCooperation,
    /// Share trust/reputation data from Bulwark
    TrustSharing,
}

impl std::fmt::Display for FederationScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FederationScope::MutualRecognition => write!(f, "mutual-recognition"),
            FederationScope::GospelPeering => write!(f, "gospel-peering"),
            FederationScope::DataSharing => write!(f, "data-sharing"),
            FederationScope::GovernanceCoordination => write!(f, "governance-coordination"),
            FederationScope::EconomicCooperation => write!(f, "economic-cooperation"),
            FederationScope::TrustSharing => write!(f, "trust-sharing"),
        }
    }
}

/// Lifecycle of a federation agreement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FederationStatus {
    /// One community has proposed; awaiting the other's acceptance
    Proposed,
    /// Both communities have agreed; federation is live
    Active,
    /// Temporarily suspended (e.g., during dispute resolution)
    Suspended,
    /// Formally withdrawn by one or both parties
    Withdrawn,
}

impl std::fmt::Display for FederationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FederationStatus::Proposed => write!(f, "proposed"),
            FederationStatus::Active => write!(f, "active"),
            FederationStatus::Suspended => write!(f, "suspended"),
            FederationStatus::Withdrawn => write!(f, "withdrawn"),
        }
    }
}

/// A bilateral federation agreement between two communities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FederationAgreement {
    pub id: Uuid,
    pub community_a: String,
    pub community_b: String,
    pub scopes: Vec<FederationScope>,
    pub status: FederationStatus,
    pub proposed_by: String,
    pub proposed_at: DateTime<Utc>,
    pub accepted_by: Option<String>,
    pub activated_at: Option<DateTime<Utc>>,
    pub suspended_at: Option<DateTime<Utc>>,
    pub suspension_reason: Option<String>,
    pub withdrawn_at: Option<DateTime<Utc>>,
    pub withdrawal_reason: Option<String>,
    pub withdrawn_by: Option<String>,
    pub description: Option<String>,
    pub authorization_proposal_id: Option<Uuid>,
}

impl FederationAgreement {
    /// Create a new federation proposal.
    pub fn propose(
        community_a: impl Into<String>,
        community_b: impl Into<String>,
        proposed_by: impl Into<String>,
        scopes: Vec<FederationScope>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            community_a: community_a.into(),
            community_b: community_b.into(),
            scopes,
            status: FederationStatus::Proposed,
            proposed_by: proposed_by.into(),
            proposed_at: Utc::now(),
            accepted_by: None,
            activated_at: None,
            suspended_at: None,
            suspension_reason: None,
            withdrawn_at: None,
            withdrawal_reason: None,
            withdrawn_by: None,
            description: None,
            authorization_proposal_id: None,
        }
    }

    /// Set a human-readable description (builder pattern).
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Link the community proposal that authorized this federation (builder pattern).
    pub fn with_authorization(mut self, proposal_id: Uuid) -> Self {
        self.authorization_proposal_id = Some(proposal_id);
        self
    }

    /// Add a scope (only while Proposed).
    pub fn add_scope(&mut self, scope: FederationScope) -> Result<(), KingdomError> {
        if self.status != FederationStatus::Proposed {
            return Err(KingdomError::InvalidTransition {
                current: self.status.to_string(),
                target: "add scope".into(),
            });
        }
        if self.scopes.contains(&scope) {
            return Err(KingdomError::InvalidTransition {
                current: "scope already present".into(),
                target: scope.to_string(),
            });
        }
        self.scopes.push(scope);
        Ok(())
    }

    /// Accept the federation (Proposed -> Active).
    pub fn accept(&mut self, accepted_by: impl Into<String>) -> Result<(), KingdomError> {
        if self.status != FederationStatus::Proposed {
            return Err(KingdomError::InvalidTransition {
                current: self.status.to_string(),
                target: "active".into(),
            });
        }
        self.status = FederationStatus::Active;
        self.accepted_by = Some(accepted_by.into());
        self.activated_at = Some(Utc::now());
        Ok(())
    }

    /// Suspend the federation (Active -> Suspended).
    pub fn suspend(&mut self, reason: impl Into<String>) -> Result<(), KingdomError> {
        if self.status != FederationStatus::Active {
            return Err(KingdomError::InvalidTransition {
                current: self.status.to_string(),
                target: "suspended".into(),
            });
        }
        self.status = FederationStatus::Suspended;
        self.suspended_at = Some(Utc::now());
        self.suspension_reason = Some(reason.into());
        Ok(())
    }

    /// Reactivate a suspended federation (Suspended -> Active).
    pub fn reactivate(&mut self) -> Result<(), KingdomError> {
        if self.status != FederationStatus::Suspended {
            return Err(KingdomError::InvalidTransition {
                current: self.status.to_string(),
                target: "active".into(),
            });
        }
        self.status = FederationStatus::Active;
        self.suspended_at = None;
        self.suspension_reason = None;
        Ok(())
    }

    /// Withdraw from the federation (Active|Suspended -> Withdrawn).
    pub fn withdraw(
        &mut self,
        withdrawn_by: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<(), KingdomError> {
        if self.status != FederationStatus::Active && self.status != FederationStatus::Suspended {
            return Err(KingdomError::InvalidTransition {
                current: self.status.to_string(),
                target: "withdrawn".into(),
            });
        }
        self.status = FederationStatus::Withdrawn;
        self.withdrawn_at = Some(Utc::now());
        self.withdrawal_reason = Some(reason.into());
        self.withdrawn_by = Some(withdrawn_by.into());
        Ok(())
    }

    /// Whether this agreement involves a specific community.
    pub fn involves(&self, community_id: &str) -> bool {
        self.community_a == community_id || self.community_b == community_id
    }

    /// The partner community (given one side's ID).
    pub fn partner_of(&self, community_id: &str) -> Option<&str> {
        if self.community_a == community_id {
            Some(&self.community_b)
        } else if self.community_b == community_id {
            Some(&self.community_a)
        } else {
            None
        }
    }

    /// Whether this agreement is currently active.
    pub fn is_active(&self) -> bool {
        self.status == FederationStatus::Active
    }

    /// Whether this agreement has been withdrawn.
    pub fn is_withdrawn(&self) -> bool {
        self.status == FederationStatus::Withdrawn
    }

    /// Whether this agreement includes a specific scope.
    pub fn has_scope(&self, scope: FederationScope) -> bool {
        self.scopes.contains(&scope)
    }

    /// Number of scopes covered by this agreement.
    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }
}

/// Registry of all federation agreements with indexed lookups.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FederationRegistry {
    agreements: HashMap<Uuid, FederationAgreement>,
    by_community: HashMap<String, Vec<Uuid>>,
}

impl FederationRegistry {
    /// Create an empty federation registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new agreement. Rejects duplicates (same pair, Proposed or Active).
    pub fn register(&mut self, agreement: FederationAgreement) -> Result<Uuid, KingdomError> {
        // Check for existing active or proposed federation between same communities
        let dominated = |a: &FederationAgreement| {
            a.status == FederationStatus::Proposed || a.status == FederationStatus::Active
        };
        let same_pair = |a: &FederationAgreement| {
            (a.community_a == agreement.community_a && a.community_b == agreement.community_b)
                || (a.community_a == agreement.community_b
                    && a.community_b == agreement.community_a)
        };
        if self.agreements.values().any(|a| same_pair(a) && dominated(a)) {
            return Err(KingdomError::FederationAlreadyExists {
                community_a: agreement.community_a.clone(),
                community_b: agreement.community_b.clone(),
            });
        }

        let id = agreement.id;
        self.by_community
            .entry(agreement.community_a.clone())
            .or_default()
            .push(id);
        self.by_community
            .entry(agreement.community_b.clone())
            .or_default()
            .push(id);
        self.agreements.insert(id, agreement);
        Ok(id)
    }

    /// Look up an agreement by its ID.
    pub fn get(&self, id: &Uuid) -> Option<&FederationAgreement> {
        self.agreements.get(id)
    }

    /// Get a mutable reference to an agreement by its ID.
    pub fn get_mut(&mut self, id: &Uuid) -> Option<&mut FederationAgreement> {
        self.agreements.get_mut(id)
    }

    /// Whether two communities are actively federated.
    pub fn is_federated(&self, community_a: &str, community_b: &str) -> bool {
        self.agreements.values().any(|a| {
            a.is_active()
                && ((a.community_a == community_a && a.community_b == community_b)
                    || (a.community_a == community_b && a.community_b == community_a))
        })
    }

    /// All communities actively federated with a given community.
    pub fn federated_with(&self, community_id: &str) -> Vec<&str> {
        self.by_community
            .get(community_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.agreements.get(id))
                    .filter(|a| a.is_active())
                    .filter_map(|a| a.partner_of(community_id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find the agreement between two specific communities (any status).
    pub fn federation_between(
        &self,
        community_a: &str,
        community_b: &str,
    ) -> Option<&FederationAgreement> {
        self.agreements.values().find(|a| {
            (a.community_a == community_a && a.community_b == community_b)
                || (a.community_a == community_b && a.community_b == community_a)
        })
    }

    /// All agreements involving a community (any status).
    pub fn agreements_for(&self, community_id: &str) -> Vec<&FederationAgreement> {
        self.by_community
            .get(community_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.agreements.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Active agreements involving a community.
    pub fn active_agreements_for(&self, community_id: &str) -> Vec<&FederationAgreement> {
        self.agreements_for(community_id)
            .into_iter()
            .filter(|a| a.is_active())
            .collect()
    }

    /// All active agreements.
    pub fn all_active(&self) -> Vec<&FederationAgreement> {
        self.agreements.values().filter(|a| a.is_active()).collect()
    }

    /// BFS path between two communities through active federation links.
    pub fn path_between(&self, from: &str, to: &str) -> Option<Vec<String>> {
        if from == to {
            return Some(vec![from.to_string()]);
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        visited.insert(from.to_string());
        queue.push_back(vec![from.to_string()]);

        while let Some(path) = queue.pop_front() {
            let current = path.last().expect("BFS path always has at least one element");
            for partner in self.federated_with(current) {
                if partner == to {
                    let mut result = path;
                    result.push(to.to_string());
                    return Some(result);
                }
                if visited.insert(partner.to_string()) {
                    let mut new_path = path.clone();
                    new_path.push(partner.to_string());
                    queue.push_back(new_path);
                }
            }
        }
        None
    }

    /// Number of active federation partners for a community.
    pub fn federation_count(&self, community_id: &str) -> usize {
        self.federated_with(community_id).len()
    }

    /// Total number of agreements in the registry (all statuses).
    pub fn total_agreements(&self) -> usize {
        self.agreements.len()
    }

    /// Number of currently active agreements.
    pub fn active_agreements(&self) -> usize {
        self.agreements.values().filter(|a| a.is_active()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn propose_federation() {
        let agreement = FederationAgreement::propose(
            "community-a",
            "community-b",
            "cpub1delegate_a",
            vec![
                FederationScope::MutualRecognition,
                FederationScope::GospelPeering,
            ],
        );
        assert_eq!(agreement.status, FederationStatus::Proposed);
        assert_eq!(agreement.community_a, "community-a");
        assert_eq!(agreement.community_b, "community-b");
        assert_eq!(agreement.scope_count(), 2);
        assert!(agreement.has_scope(FederationScope::MutualRecognition));
        assert!(agreement.has_scope(FederationScope::GospelPeering));
        assert!(!agreement.has_scope(FederationScope::TrustSharing));
    }

    #[test]
    fn accept_federation() {
        let mut agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        assert!(agreement.accept("cpub1b").is_ok());
        assert_eq!(agreement.status, FederationStatus::Active);
        assert_eq!(agreement.accepted_by.as_deref(), Some("cpub1b"));
        assert!(agreement.activated_at.is_some());
        assert!(agreement.is_active());
    }

    #[test]
    fn accept_only_from_proposed() {
        let mut agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        agreement.accept("cpub1b").unwrap();
        // Can't accept from Active
        assert!(agreement.accept("cpub1c").is_err());
    }

    #[test]
    fn suspend_federation() {
        let mut agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        agreement.accept("cpub1b").unwrap();
        assert!(agreement.suspend("dispute in progress").is_ok());
        assert_eq!(agreement.status, FederationStatus::Suspended);
        assert_eq!(
            agreement.suspension_reason.as_deref(),
            Some("dispute in progress")
        );
    }

    #[test]
    fn suspend_only_from_active() {
        let mut agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        // Can't suspend from Proposed
        assert!(agreement.suspend("reason").is_err());
    }

    #[test]
    fn reactivate_federation() {
        let mut agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        agreement.accept("cpub1b").unwrap();
        agreement.suspend("pause").unwrap();
        assert!(agreement.reactivate().is_ok());
        assert_eq!(agreement.status, FederationStatus::Active);
    }

    #[test]
    fn reactivate_only_from_suspended() {
        let mut agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        agreement.accept("cpub1b").unwrap();
        // Can't reactivate from Active
        assert!(agreement.reactivate().is_err());
    }

    #[test]
    fn withdraw_from_active() {
        let mut agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        agreement.accept("cpub1b").unwrap();
        assert!(agreement.withdraw("cpub1a", "policy divergence").is_ok());
        assert_eq!(agreement.status, FederationStatus::Withdrawn);
        assert!(agreement.is_withdrawn());
        assert_eq!(
            agreement.withdrawal_reason.as_deref(),
            Some("policy divergence")
        );
    }

    #[test]
    fn withdraw_from_suspended() {
        let mut agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        agreement.accept("cpub1b").unwrap();
        agreement.suspend("dispute").unwrap();
        assert!(agreement.withdraw("cpub1a", "irreconcilable").is_ok());
        assert!(agreement.is_withdrawn());
    }

    #[test]
    fn cannot_withdraw_from_proposed() {
        let mut agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        assert!(agreement.withdraw("cpub1a", "changed mind").is_err());
    }

    #[test]
    fn double_withdraw_fails() {
        let mut agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        agreement.accept("cpub1b").unwrap();
        agreement.withdraw("cpub1a", "done").unwrap();
        assert!(agreement.withdraw("cpub1b", "also done").is_err());
    }

    #[test]
    fn involves_and_partner_of() {
        let agreement = FederationAgreement::propose(
            "alpha",
            "beta",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        assert!(agreement.involves("alpha"));
        assert!(agreement.involves("beta"));
        assert!(!agreement.involves("gamma"));
        assert_eq!(agreement.partner_of("alpha"), Some("beta"));
        assert_eq!(agreement.partner_of("beta"), Some("alpha"));
        assert_eq!(agreement.partner_of("gamma"), None);
    }

    #[test]
    fn add_scope_while_proposed() {
        let mut agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        assert!(agreement.add_scope(FederationScope::GospelPeering).is_ok());
        assert_eq!(agreement.scope_count(), 2);
    }

    #[test]
    fn add_scope_after_acceptance_fails() {
        let mut agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        agreement.accept("cpub1b").unwrap();
        assert!(agreement.add_scope(FederationScope::GospelPeering).is_err());
    }

    #[test]
    fn add_duplicate_scope_fails() {
        let mut agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        assert!(agreement
            .add_scope(FederationScope::MutualRecognition)
            .is_err());
    }

    #[test]
    fn builders() {
        let prop_id = Uuid::new_v4();
        let agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        )
        .with_description("Friendship treaty")
        .with_authorization(prop_id);
        assert_eq!(agreement.description.as_deref(), Some("Friendship treaty"));
        assert_eq!(agreement.authorization_proposal_id, Some(prop_id));
    }

    #[test]
    fn agreement_serde_round_trip() {
        let agreement = {
            let mut a = FederationAgreement::propose(
                "community-a",
                "community-b",
                "cpub1delegate",
                vec![
                    FederationScope::MutualRecognition,
                    FederationScope::EconomicCooperation,
                ],
            )
            .with_description("test");
            a.accept("cpub1other").unwrap();
            a
        };

        let json = serde_json::to_string(&agreement).unwrap();
        let restored: FederationAgreement = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.community_a, "community-a");
        assert_eq!(restored.status, FederationStatus::Active);
        assert_eq!(restored.scope_count(), 2);
    }

    #[test]
    fn scope_display() {
        assert_eq!(
            FederationScope::MutualRecognition.to_string(),
            "mutual-recognition"
        );
        assert_eq!(
            FederationScope::GospelPeering.to_string(),
            "gospel-peering"
        );
        assert_eq!(FederationScope::DataSharing.to_string(), "data-sharing");
        assert_eq!(
            FederationScope::GovernanceCoordination.to_string(),
            "governance-coordination"
        );
        assert_eq!(
            FederationScope::EconomicCooperation.to_string(),
            "economic-cooperation"
        );
        assert_eq!(FederationScope::TrustSharing.to_string(), "trust-sharing");
    }

    #[test]
    fn status_display() {
        assert_eq!(FederationStatus::Proposed.to_string(), "proposed");
        assert_eq!(FederationStatus::Active.to_string(), "active");
        assert_eq!(FederationStatus::Suspended.to_string(), "suspended");
        assert_eq!(FederationStatus::Withdrawn.to_string(), "withdrawn");
    }

    // Registry tests

    #[test]
    fn register_and_get() {
        let mut registry = FederationRegistry::new();
        let agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        let id = agreement.id;
        registry.register(agreement).unwrap();
        assert!(registry.get(&id).is_some());
        assert_eq!(registry.total_agreements(), 1);
    }

    #[test]
    fn duplicate_registration_rejected() {
        let mut registry = FederationRegistry::new();
        let a1 = FederationAgreement::propose(
            "alpha",
            "beta",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        registry.register(a1).unwrap();
        // Same pair reversed
        let a2 = FederationAgreement::propose(
            "beta",
            "alpha",
            "cpub1b",
            vec![FederationScope::GospelPeering],
        );
        assert!(registry.register(a2).is_err());
    }

    #[test]
    fn is_federated_active_only() {
        let mut registry = FederationRegistry::new();
        let agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        let id = agreement.id;
        // Proposed -- not federated yet
        registry.register(agreement).unwrap();
        assert!(!registry.is_federated("a", "b"));

        // Accept -- now federated
        registry.get_mut(&id).unwrap().accept("cpub1b").unwrap();
        assert!(registry.is_federated("a", "b"));
        assert!(registry.is_federated("b", "a")); // symmetric

        // Withdraw -- no longer federated
        registry
            .get_mut(&id)
            .unwrap()
            .withdraw("cpub1a", "done")
            .unwrap();
        assert!(!registry.is_federated("a", "b"));
    }

    #[test]
    fn federated_with_returns_partners() {
        let mut registry = FederationRegistry::new();
        for partner in &["b", "c", "d"] {
            let mut a = FederationAgreement::propose(
                "a",
                *partner,
                "cpub1a",
                vec![FederationScope::MutualRecognition],
            );
            a.accept("cpub1other").unwrap();
            let id = a.id;
            registry.agreements.insert(id, a);
            registry
                .by_community
                .entry("a".into())
                .or_default()
                .push(id);
            registry
                .by_community
                .entry(partner.to_string())
                .or_default()
                .push(id);
        }
        let mut partners = registry.federated_with("a");
        partners.sort();
        assert_eq!(partners, vec!["b", "c", "d"]);
    }

    #[test]
    fn federated_with_excludes_withdrawn() {
        let mut registry = FederationRegistry::new();
        let mut a1 = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        a1.accept("cpub1b").unwrap();
        let id1 = a1.id;
        registry.agreements.insert(id1, a1);
        registry
            .by_community
            .entry("a".into())
            .or_default()
            .push(id1);
        registry
            .by_community
            .entry("b".into())
            .or_default()
            .push(id1);

        let mut a2 = FederationAgreement::propose(
            "a",
            "c",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        a2.accept("cpub1c").unwrap();
        a2.withdraw("cpub1a", "done").unwrap();
        let id2 = a2.id;
        registry.agreements.insert(id2, a2);
        registry
            .by_community
            .entry("a".into())
            .or_default()
            .push(id2);
        registry
            .by_community
            .entry("c".into())
            .or_default()
            .push(id2);

        assert_eq!(registry.federated_with("a"), vec!["b"]);
    }

    #[test]
    fn path_between_connected() {
        let mut registry = FederationRegistry::new();
        // A <-> B, B <-> C
        for (a, b) in &[("a", "b"), ("b", "c")] {
            let mut agreement = FederationAgreement::propose(
                *a,
                *b,
                "cpub1x",
                vec![FederationScope::MutualRecognition],
            );
            agreement.accept("cpub1y").unwrap();
            let id = agreement.id;
            registry.agreements.insert(id, agreement);
            registry
                .by_community
                .entry(a.to_string())
                .or_default()
                .push(id);
            registry
                .by_community
                .entry(b.to_string())
                .or_default()
                .push(id);
        }
        let path = registry.path_between("a", "c");
        assert_eq!(
            path,
            Some(vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string()
            ])
        );
    }

    #[test]
    fn path_between_disconnected() {
        let mut registry = FederationRegistry::new();
        // A <-> B, C <-> D (no connection)
        for (a, b) in &[("a", "b"), ("c", "d")] {
            let mut agreement = FederationAgreement::propose(
                *a,
                *b,
                "cpub1x",
                vec![FederationScope::MutualRecognition],
            );
            agreement.accept("cpub1y").unwrap();
            let id = agreement.id;
            registry.agreements.insert(id, agreement);
            registry
                .by_community
                .entry(a.to_string())
                .or_default()
                .push(id);
            registry
                .by_community
                .entry(b.to_string())
                .or_default()
                .push(id);
        }
        assert_eq!(registry.path_between("a", "d"), None);
    }

    #[test]
    fn path_between_same() {
        let registry = FederationRegistry::new();
        assert_eq!(
            registry.path_between("a", "a"),
            Some(vec!["a".to_string()])
        );
    }

    #[test]
    fn federation_count_and_totals() {
        let mut registry = FederationRegistry::new();
        let mut a1 = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        a1.accept("cpub1b").unwrap();
        let id1 = a1.id;
        registry.agreements.insert(id1, a1);
        registry
            .by_community
            .entry("a".into())
            .or_default()
            .push(id1);
        registry
            .by_community
            .entry("b".into())
            .or_default()
            .push(id1);

        let a2 = FederationAgreement::propose(
            "a",
            "c",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        let id2 = a2.id;
        registry.agreements.insert(id2, a2);
        registry
            .by_community
            .entry("a".into())
            .or_default()
            .push(id2);
        registry
            .by_community
            .entry("c".into())
            .or_default()
            .push(id2);

        assert_eq!(registry.federation_count("a"), 1); // only b is active
        assert_eq!(registry.total_agreements(), 2);
        assert_eq!(registry.active_agreements(), 1);
    }

    #[test]
    fn registry_serde_round_trip() {
        let mut registry = FederationRegistry::new();
        let agreement = FederationAgreement::propose(
            "a",
            "b",
            "cpub1a",
            vec![FederationScope::MutualRecognition],
        );
        registry.register(agreement).unwrap();
        let json = serde_json::to_string(&registry).unwrap();
        let restored: FederationRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.total_agreements(), 1);
    }

    #[test]
    fn full_lifecycle() {
        let mut agreement = FederationAgreement::propose(
            "alpha",
            "beta",
            "cpub1alpha_rep",
            vec![
                FederationScope::MutualRecognition,
                FederationScope::GospelPeering,
            ],
        )
        .with_description("Alpha-Beta federation");

        // Propose
        assert_eq!(agreement.status, FederationStatus::Proposed);
        assert!(agreement.accept("cpub1beta_rep").is_ok());

        // Active
        assert_eq!(agreement.status, FederationStatus::Active);
        assert!(agreement.suspend("reviewing terms").is_ok());

        // Suspended
        assert_eq!(agreement.status, FederationStatus::Suspended);
        assert!(agreement.reactivate().is_ok());

        // Active again
        assert_eq!(agreement.status, FederationStatus::Active);
        assert!(agreement
            .withdraw("cpub1alpha_rep", "completed purpose")
            .is_ok());

        // Withdrawn
        assert_eq!(agreement.status, FederationStatus::Withdrawn);
        assert!(agreement.is_withdrawn());
    }
}
