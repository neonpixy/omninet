//! # Exit with Dignity (R3C)
//!
//! Any member can leave any community with everything they brought. The right to
//! exit is absolute and inalienable — it cannot be penalized, fined, or punished.
//!
//! From the Covenant's axiom of Sovereignty: "The birthright of every person to
//! choose, refuse, and reshape the terms of their participation."
//!
//! ## What stays with the member
//!
//! - Crown identity (always)
//! - Vault data — personal .idea files (always)
//! - Fortune balance — personal funds (always)
//! - Yoke history — full activity history (always)
//! - Reputation score — Bulwark reputation travels with you (always)
//! - Personal bonds
//!
//! ## What stays with the community
//!
//! - Collective contributions (copies, not originals if sole author)
//! - Governance roles (vacated)
//! - Delegations received (returned to delegators)
//!
//! ## Charter constraint
//!
//! Communities CANNOT add exit penalties (economic fines, reputation damage,
//! bond severance) beyond the natural cost of leaving. Any charter clause that
//! adds exit penalties is rejected by `ConstitutionalReviewer::review()` as
//! violating Sovereignty. You're free to make it attractive to stay — you're
//! not free to make it punitive to leave.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::KingdomError;

// ---------------------------------------------------------------------------
// Exit package
// ---------------------------------------------------------------------------

/// Everything a departing member takes and leaves behind.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExitPackage {
    pub id: Uuid,
    /// The member who is leaving.
    pub member_pubkey: String,
    /// The community being left.
    pub community_id: String,
    /// When the exit was finalized.
    pub exited_at: DateTime<Utc>,
    /// What the member keeps.
    pub retained: ExitRetained,
    /// What stays with the community.
    pub transferred: ExitTransferred,
}

impl ExitPackage {
    pub fn new(
        member_pubkey: impl Into<String>,
        community_id: impl Into<String>,
        retained: ExitRetained,
        transferred: ExitTransferred,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            member_pubkey: member_pubkey.into(),
            community_id: community_id.into(),
            exited_at: Utc::now(),
            retained,
            transferred,
        }
    }
}

// ---------------------------------------------------------------------------
// Retained items (go with the member)
// ---------------------------------------------------------------------------

/// Everything that goes with the departing member. These are inalienable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExitRetained {
    /// Crown identity always travels with you — it IS you.
    pub crown_identity: bool,
    /// Personal .idea files in Vault — always retained.
    pub vault_data: bool,
    /// Personal Fortune balance — always retained.
    pub fortune_balance: i64,
    /// Full activity history — always retained.
    pub yoke_history: bool,
    /// Bulwark reputation score — portable across communities.
    pub reputation_score: i32,
    /// Personal bonds are not community property.
    pub bonds: Vec<VisibleBond>,
}

impl ExitRetained {
    /// Create a retained package with default inalienable rights.
    pub fn new(fortune_balance: i64, reputation_score: i32) -> Self {
        Self {
            crown_identity: true,
            vault_data: true,
            fortune_balance,
            yoke_history: true,
            reputation_score,
            bonds: Vec::new(),
        }
    }

    /// Add personal bonds to the retained package.
    pub fn with_bonds(mut self, bonds: Vec<VisibleBond>) -> Self {
        self.bonds = bonds;
        self
    }
}

impl Default for ExitRetained {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

/// A bond visible in the exit package (personal bonds, not community bonds).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisibleBond {
    pub bond_id: String,
    pub description: String,
}

impl VisibleBond {
    pub fn new(bond_id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            bond_id: bond_id.into(),
            description: description.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Transferred items (stay with the community)
// ---------------------------------------------------------------------------

/// Things that stay with the community when a member leaves.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExitTransferred {
    /// .idea files created in community Collectives. Copies remain — if the
    /// member was sole author, the original goes with them and the community
    /// keeps a copy.
    pub collective_contributions: Vec<String>,
    /// Governance roles vacated upon departure.
    pub governance_roles: Vec<String>,
    /// Delegations from other members are returned to those members.
    pub delegations_received: Vec<String>,
}

impl ExitTransferred {
    pub fn new() -> Self {
        Self {
            collective_contributions: Vec::new(),
            governance_roles: Vec::new(),
            delegations_received: Vec::new(),
        }
    }

    pub fn with_contributions(mut self, contributions: Vec<String>) -> Self {
        self.collective_contributions = contributions;
        self
    }

    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.governance_roles = roles;
        self
    }

    pub fn with_delegations(mut self, delegations: Vec<String>) -> Self {
        self.delegations_received = delegations;
        self
    }
}

impl Default for ExitTransferred {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Exit cost transparency
// ---------------------------------------------------------------------------

/// A cost the member would incur by leaving — shown BEFORE they decide.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExitCost {
    pub cost_type: ExitCostType,
    /// Numeric amount, if applicable (e.g. locked cooperative shares).
    pub amount: Option<i64>,
    /// Human-readable explanation.
    pub description: String,
}

impl ExitCost {
    pub fn new(cost_type: ExitCostType, description: impl Into<String>) -> Self {
        Self {
            cost_type,
            amount: None,
            description: description.into(),
        }
    }

    pub fn with_amount(mut self, amount: i64) -> Self {
        self.amount = Some(amount);
        self
    }
}

/// What kind of cost this represents.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ExitCostType {
    /// No cost — ideal exit.
    None,
    /// Locked cooperative shares with a configurable unlock period.
    EconomicLoss,
    /// Community-only bonds that can't transfer.
    SocialLoss,
    /// Collective data that can't be individually copied.
    DataLoss,
}

// ---------------------------------------------------------------------------
// Exit cost calculator
// ---------------------------------------------------------------------------

/// Calculates the costs a member would face by leaving a community.
///
/// Shows the member exactly what they'd lose BEFORE they leave. Transparency
/// is the only acceptable approach — no hidden penalties.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExitCostCalculator {
    /// Locked cooperative shares that haven't vested.
    pub locked_shares: i64,
    /// Community-only bonds that wouldn't transfer.
    pub community_only_bonds: Vec<String>,
    /// Collective data items that can't be individually extracted.
    pub non_extractable_data: Vec<String>,
}

impl ExitCostCalculator {
    pub fn new() -> Self {
        Self {
            locked_shares: 0,
            community_only_bonds: Vec::new(),
            non_extractable_data: Vec::new(),
        }
    }

    pub fn with_locked_shares(mut self, shares: i64) -> Self {
        self.locked_shares = shares;
        self
    }

    pub fn with_community_bonds(mut self, bonds: Vec<String>) -> Self {
        self.community_only_bonds = bonds;
        self
    }

    pub fn with_non_extractable_data(mut self, data: Vec<String>) -> Self {
        self.non_extractable_data = data;
        self
    }

    /// Calculate all exit costs for a member leaving a community.
    ///
    /// These are NATURAL costs — not penalties. The difference is fundamental:
    /// natural costs arise from the structure of participation. Penalties are
    /// imposed to punish departure. Penalties are unconstitutional.
    pub fn calculate(&self) -> Vec<ExitCost> {
        let mut costs = Vec::new();

        if self.locked_shares > 0 {
            costs.push(
                ExitCost::new(
                    ExitCostType::EconomicLoss,
                    format!(
                        "Locked cooperative shares ({}) may have a vesting period",
                        self.locked_shares
                    ),
                )
                .with_amount(self.locked_shares),
            );
        }

        for bond in &self.community_only_bonds {
            costs.push(ExitCost::new(
                ExitCostType::SocialLoss,
                format!("Community-only bond cannot transfer: {bond}"),
            ));
        }

        for data in &self.non_extractable_data {
            costs.push(ExitCost::new(
                ExitCostType::DataLoss,
                format!("Collective data cannot be individually copied: {data}"),
            ));
        }

        if costs.is_empty() {
            costs.push(ExitCost::new(
                ExitCostType::None,
                "Clean exit — no costs identified",
            ));
        }

        costs
    }
}

impl Default for ExitCostCalculator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Charter penalty rejection
// ---------------------------------------------------------------------------

/// Checks a charter clause description for exit penalties that violate Sovereignty.
///
/// Returns `Err` if the clause appears to impose punitive exit costs. Communities
/// CANNOT add economic fines, reputation damage, or bond severance as exit penalties.
pub fn reject_exit_penalty_clause(clause: &str) -> Result<(), KingdomError> {
    let lower = clause.to_lowercase();
    let penalty_indicators = [
        "exit fee",
        "exit penalty",
        "departure fine",
        "leaving penalty",
        "exit tax",
        "departure tax",
        "reputation penalty on exit",
        "bond forfeiture on exit",
        "mandatory bond severance",
        "exit surcharge",
    ];

    for indicator in &penalty_indicators {
        if lower.contains(indicator) {
            return Err(KingdomError::ExitPenaltyViolation(clause.to_string()));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- ExitRetained ---

    #[test]
    fn retained_defaults_are_inalienable() {
        let retained = ExitRetained::new(1000, 85);
        assert!(retained.crown_identity);
        assert!(retained.vault_data);
        assert!(retained.yoke_history);
        assert_eq!(retained.fortune_balance, 1000);
        assert_eq!(retained.reputation_score, 85);
    }

    #[test]
    fn retained_with_bonds() {
        let bonds = vec![
            VisibleBond::new("bond-1", "Friendship with Alice"),
            VisibleBond::new("bond-2", "Mentorship from Bob"),
        ];
        let retained = ExitRetained::new(500, 70).with_bonds(bonds);
        assert_eq!(retained.bonds.len(), 2);
    }

    #[test]
    fn retained_default_zero_values() {
        let retained = ExitRetained::default();
        assert!(retained.crown_identity);
        assert_eq!(retained.fortune_balance, 0);
        assert_eq!(retained.reputation_score, 0);
    }

    // --- ExitTransferred ---

    #[test]
    fn transferred_builder() {
        let transferred = ExitTransferred::new()
            .with_contributions(vec!["doc-1".into(), "design-2".into()])
            .with_roles(vec!["Elder".into()])
            .with_delegations(vec!["alice_delegation".into()]);

        assert_eq!(transferred.collective_contributions.len(), 2);
        assert_eq!(transferred.governance_roles, vec!["Elder"]);
        assert_eq!(transferred.delegations_received.len(), 1);
    }

    #[test]
    fn transferred_empty_default() {
        let transferred = ExitTransferred::default();
        assert!(transferred.collective_contributions.is_empty());
        assert!(transferred.governance_roles.is_empty());
        assert!(transferred.delegations_received.is_empty());
    }

    // --- ExitPackage ---

    #[test]
    fn exit_package_creation() {
        let retained = ExitRetained::new(5000, 90);
        let transferred = ExitTransferred::new()
            .with_roles(vec!["Steward".into()]);

        let pkg = ExitPackage::new("alice_pubkey", "community-123", retained, transferred);
        assert_eq!(pkg.member_pubkey, "alice_pubkey");
        assert_eq!(pkg.community_id, "community-123");
        assert!(pkg.retained.crown_identity);
        assert_eq!(pkg.transferred.governance_roles, vec!["Steward"]);
    }

    #[test]
    fn exit_package_serialization_roundtrip() {
        let pkg = ExitPackage::new(
            "bob",
            "comm-1",
            ExitRetained::new(100, 50),
            ExitTransferred::new(),
        );
        let json = serde_json::to_string(&pkg).unwrap();
        let restored: ExitPackage = serde_json::from_str(&json).unwrap();
        assert_eq!(pkg.member_pubkey, restored.member_pubkey);
        assert_eq!(pkg.retained.fortune_balance, restored.retained.fortune_balance);
    }

    // --- ExitCost ---

    #[test]
    fn exit_cost_with_amount() {
        let cost = ExitCost::new(ExitCostType::EconomicLoss, "Locked shares")
            .with_amount(500);
        assert_eq!(cost.amount, Some(500));
        assert_eq!(cost.cost_type, ExitCostType::EconomicLoss);
    }

    #[test]
    fn exit_cost_without_amount() {
        let cost = ExitCost::new(ExitCostType::SocialLoss, "Community-only bond");
        assert_eq!(cost.amount, None);
    }

    #[test]
    fn exit_cost_type_serialization() {
        let types = vec![
            ExitCostType::None,
            ExitCostType::EconomicLoss,
            ExitCostType::SocialLoss,
            ExitCostType::DataLoss,
        ];
        let json = serde_json::to_string(&types).unwrap();
        let restored: Vec<ExitCostType> = serde_json::from_str(&json).unwrap();
        assert_eq!(types, restored);
    }

    // --- ExitCostCalculator ---

    #[test]
    fn calculator_clean_exit() {
        let calc = ExitCostCalculator::new();
        let costs = calc.calculate();
        assert_eq!(costs.len(), 1);
        assert_eq!(costs[0].cost_type, ExitCostType::None);
    }

    #[test]
    fn calculator_with_locked_shares() {
        let calc = ExitCostCalculator::new().with_locked_shares(1000);
        let costs = calc.calculate();
        assert_eq!(costs.len(), 1);
        assert_eq!(costs[0].cost_type, ExitCostType::EconomicLoss);
        assert_eq!(costs[0].amount, Some(1000));
    }

    #[test]
    fn calculator_with_community_bonds() {
        let calc = ExitCostCalculator::new()
            .with_community_bonds(vec!["guild-bond".into(), "council-bond".into()]);
        let costs = calc.calculate();
        assert_eq!(costs.len(), 2);
        assert!(costs.iter().all(|c| c.cost_type == ExitCostType::SocialLoss));
    }

    #[test]
    fn calculator_with_non_extractable_data() {
        let calc = ExitCostCalculator::new()
            .with_non_extractable_data(vec!["collaborative-mural".into()]);
        let costs = calc.calculate();
        assert_eq!(costs.len(), 1);
        assert_eq!(costs[0].cost_type, ExitCostType::DataLoss);
    }

    #[test]
    fn calculator_mixed_costs() {
        let calc = ExitCostCalculator::new()
            .with_locked_shares(500)
            .with_community_bonds(vec!["bond-1".into()])
            .with_non_extractable_data(vec!["data-1".into()]);
        let costs = calc.calculate();
        assert_eq!(costs.len(), 3);

        let types: Vec<ExitCostType> = costs.iter().map(|c| c.cost_type).collect();
        assert!(types.contains(&ExitCostType::EconomicLoss));
        assert!(types.contains(&ExitCostType::SocialLoss));
        assert!(types.contains(&ExitCostType::DataLoss));
    }

    #[test]
    fn calculator_serialization() {
        let calc = ExitCostCalculator::new().with_locked_shares(200);
        let json = serde_json::to_string(&calc).unwrap();
        let restored: ExitCostCalculator = serde_json::from_str(&json).unwrap();
        assert_eq!(calc.locked_shares, restored.locked_shares);
    }

    // --- Charter penalty rejection ---

    #[test]
    fn reject_exit_fee_clause() {
        let result = reject_exit_penalty_clause("Members must pay an exit fee of 100 Cool");
        assert!(result.is_err());
    }

    #[test]
    fn reject_departure_fine() {
        let result = reject_exit_penalty_clause("A departure fine applies to early leavers");
        assert!(result.is_err());
    }

    #[test]
    fn reject_exit_tax() {
        let result = reject_exit_penalty_clause("An exit tax of 10% is levied");
        assert!(result.is_err());
    }

    #[test]
    fn reject_reputation_penalty_on_exit() {
        let result = reject_exit_penalty_clause(
            "Members face a reputation penalty on exit from the community",
        );
        assert!(result.is_err());
    }

    #[test]
    fn accept_legitimate_clause() {
        let result = reject_exit_penalty_clause(
            "Members may leave at any time. We encourage discussion before departure.",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn accept_retention_incentive() {
        // Making it attractive to stay is fine — it's not punitive to leave.
        let result = reject_exit_penalty_clause(
            "Long-term members receive additional governance weight",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn reject_case_insensitive() {
        let result = reject_exit_penalty_clause("An EXIT PENALTY of 50 applies");
        assert!(result.is_err());
    }

    #[test]
    fn reject_bond_forfeiture_on_exit() {
        let result = reject_exit_penalty_clause(
            "Bond forfeiture on exit applies to all members",
        );
        assert!(result.is_err());
    }

    // --- VisibleBond ---

    #[test]
    fn visible_bond_creation() {
        let bond = VisibleBond::new("b-1", "Alice's friendship");
        assert_eq!(bond.bond_id, "b-1");
        assert_eq!(bond.description, "Alice's friendship");
    }

    #[test]
    fn visible_bond_serialization() {
        let bond = VisibleBond::new("b-2", "Mentorship");
        let json = serde_json::to_string(&bond).unwrap();
        let restored: VisibleBond = serde_json::from_str(&json).unwrap();
        assert_eq!(bond, restored);
    }
}
