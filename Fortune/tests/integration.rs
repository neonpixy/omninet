use fortune::*;

/// Full economic cycle: mint → UBI → transfer → demurrage → flow-back.
#[test]
fn full_economic_cycle() {
    let policy = FortunePolicy::testing();
    let mut treasury = Treasury::new(policy.clone());
    treasury.update_metrics(NetworkMetrics {
        active_users: 100,
        total_ideas: 50,
        total_collectives: 5,
    });
    let mut ledger = Ledger::new();
    let mut ubi = UbiDistributor::new();
    let demurrage = DemurrageEngine::new();
    let flow_back = FlowBack::new();

    // 1. Verify identity and claim UBI
    ubi.verify_identity("alice");
    ubi.verify_identity("bob");

    let claim = ubi
        .claim("alice", &mut ledger, &mut treasury, &policy)
        .unwrap();
    assert_eq!(claim.amount, policy.ubi_amount);
    assert_eq!(ledger.balance("alice").liquid, policy.ubi_amount);

    // 2. Give alice some initial Cool for testing
    treasury
        .mint(1000, "alice", MintReason::Initial)
        .unwrap();
    ledger.credit("alice", 1000, TransactionReason::Initial, None);

    // 3. Transfer Cool: alice → bob
    ledger.transfer("alice", "bob", 200, None).unwrap();
    assert_eq!(ledger.balance("bob").liquid, 200);

    // 4. Calculate demurrage (simulated — manually apply)
    let alice_balance = ledger.balance("alice").liquid;
    let decay = demurrage.calculate_decay(alice_balance, 30, &policy);
    assert!(decay > 0, "should have some decay on balance {alice_balance}");

    // 5. Flow-back calculation on large balances
    let whale_balance: i64 = 5_000_000;
    let fb = flow_back.calculate(whale_balance, &policy.flow_back_tiers);
    assert!(fb > 0, "whale should have flow-back");

    // 6. Verify treasury tracks everything
    let status = treasury.status();
    assert!(status.in_circulation > 0);
    assert!(status.available > 0);
}

/// Cash lifecycle: issue → lock → redeem → verify balance conservation.
#[test]
fn cash_lifecycle() {
    let policy = FortunePolicy::testing();
    let mut treasury = Treasury::new(policy.clone());
    treasury.update_metrics(NetworkMetrics {
        active_users: 100,
        total_ideas: 0,
        total_collectives: 0,
    });
    let mut ledger = Ledger::new();
    let mut registry = CashRegistry::new();
    let mut mint = CashMint::new();
    let mut redemption = CashRedemption::new();

    // Setup: give alice Cool
    treasury
        .mint(5000, "alice", MintReason::Initial)
        .unwrap();
    ledger.credit("alice", 5000, TransactionReason::Initial, None);

    // 1. Issue cash note
    let note = mint
        .issue(
            "alice",
            500,
            Some("For the market".into()),
            None,
            &mut ledger,
            &mut treasury,
            &mut registry,
            &policy,
        )
        .unwrap();

    // Alice: 4500 liquid, 500 locked
    assert_eq!(ledger.balance("alice").liquid, 4500);
    assert_eq!(ledger.balance("alice").locked, 500);
    assert_eq!(ledger.balance("alice").total(), 5000); // conserved!

    // 2. Bob redeems the cash note
    let result = redemption
        .redeem(
            &note.serial,
            "bob",
            &mut ledger,
            &mut treasury,
            &mut registry,
        )
        .unwrap();

    assert_eq!(result.amount, 500);
    assert_eq!(ledger.balance("bob").liquid, 500);
    assert_eq!(ledger.balance("alice").locked, 0);
    assert_eq!(ledger.balance("alice").total(), 4500); // alice lost 500

    // 3. Verify the note is redeemed
    let redeemed_note = registry.note(&note.serial).unwrap();
    assert_eq!(redeemed_note.status, CashStatus::Redeemed);
    assert_eq!(redeemed_note.redeemer.as_deref(), Some("bob"));
}

/// Cooperative surplus distribution.
#[test]
fn cooperative_surplus_distribution() {
    let mut coop = Cooperative::new("River Workshop", SurplusDistribution::ProportionalToContribution);
    coop.add_member(CooperativeMember {
        pubkey: "alice".into(),
        role: Some("coordinator".into()),
        contribution_score: 100,
        joined_at: chrono::Utc::now(),
    })
    .unwrap();
    coop.add_member(CooperativeMember {
        pubkey: "bob".into(),
        role: None,
        contribution_score: 50,
        joined_at: chrono::Utc::now(),
    })
    .unwrap();
    coop.add_member(CooperativeMember {
        pubkey: "charlie".into(),
        role: None,
        contribution_score: 50,
        joined_at: chrono::Utc::now(),
    })
    .unwrap();

    // Distribute 1000 Cool surplus proportionally
    let distribution = coop.distribute_surplus(1000);
    let alice_share = distribution
        .iter()
        .find(|(p, _)| p == "alice")
        .unwrap()
        .1;
    let bob_share = distribution.iter().find(|(p, _)| p == "bob").unwrap().1;

    // Alice: 100/200 = 50%, Bob: 50/200 = 25%
    assert_eq!(alice_share, 500);
    assert_eq!(bob_share, 250);

    // Total distributed ≈ 1000 (rounding may vary slightly)
    let total: i64 = distribution.iter().map(|(_, s)| s).sum();
    assert_eq!(total, 1000);
}

/// Commons trust stewardship.
#[test]
fn commons_trust_stewardship() {
    let mut trust = CommonsTrust::new("Watershed Commons", TrustType::Land);
    trust.add_steward("alice");
    trust.add_steward("bob");

    trust.add_asset(TrustAsset::new(
        "Oak Grove",
        "20 acres of old-growth oak forest",
        "forest",
        "community_council",
    ));
    trust.add_asset(TrustAsset::new(
        "Spring Creek",
        "Year-round freshwater spring",
        "water_source",
        "community_council",
    ));

    assert_eq!(trust.steward_count(), 2);
    assert_eq!(trust.asset_count(), 2);

    trust.record_stewardship(StewardshipRecord::new(
        "alice",
        "Cleared invasive species from Oak Grove section B",
    ));
    assert_eq!(trust.stewardship_records.len(), 1);
}

/// Trade proposal lifecycle.
#[test]
fn trade_proposal_lifecycle() {
    let mut trade = TradeProposal::new("alice", "bob", 100, 50)
        .unwrap()
        .with_message("Fair exchange for your pottery");

    assert_eq!(trade.status, TradeStatus::Proposed);
    assert!(trade.is_active());

    trade.accept().unwrap();
    assert_eq!(trade.status, TradeStatus::Accepted);

    trade.execute().unwrap();
    assert_eq!(trade.status, TradeStatus::Executed);
    assert!(!trade.is_active());
}

/// Escrow with milestone conditions.
#[test]
fn escrow_milestone_release() {
    let mut escrow = EscrowRecord::new("alice", "bob", 1000).with_conditions(vec![
        ReleaseCondition {
            description: "Design mockup delivered".into(),
            percentage: 30,
            met: false,
            met_at: None,
        },
        ReleaseCondition {
            description: "Final deliverable approved".into(),
            percentage: 70,
            met: false,
            met_at: None,
        },
    ]);

    assert!(!escrow.all_conditions_met());

    // Meet first condition
    escrow.conditions[0].met = true;
    escrow.conditions[0].met_at = Some(chrono::Utc::now());
    assert!(!escrow.all_conditions_met());

    // Meet second condition
    escrow.conditions[1].met = true;
    escrow.conditions[1].met_at = Some(chrono::Utc::now());
    assert!(escrow.all_conditions_met());

    escrow.release().unwrap();
    assert_eq!(escrow.status, EscrowStatus::Released);
}

/// UBI eligibility pipeline — all 6 checks.
#[test]
fn ubi_eligibility_pipeline() {
    let policy = FortunePolicy::default_policy();
    let mut treasury = Treasury::new(policy.clone());
    treasury.update_metrics(NetworkMetrics {
        active_users: 100,
        total_ideas: 0,
        total_collectives: 0,
    });
    let mut ledger = Ledger::new();
    let mut ubi = UbiDistributor::new();

    // 1. Not verified → ineligible
    let elig = ubi.check_eligibility("alice", &ledger, &treasury, &policy);
    assert_eq!(elig.reason, Some(IneligibilityReason::NotVerified));

    // Verify alice
    ubi.verify_identity("alice");

    // 2. Flagged → ineligible
    ubi.flag_account("alice");
    let elig = ubi.check_eligibility("alice", &ledger, &treasury, &policy);
    assert_eq!(elig.reason, Some(IneligibilityReason::Flagged));
    ubi.unflag_account("alice");

    // 3. Paused → ineligible
    ubi.is_paused = true;
    let elig = ubi.check_eligibility("alice", &ledger, &treasury, &policy);
    assert_eq!(elig.reason, Some(IneligibilityReason::Paused));
    ubi.is_paused = false;

    // 4. Balance capped → ineligible
    ledger.credit("alice", policy.ubi_balance_cap, TransactionReason::Initial, None);
    let elig = ubi.check_eligibility("alice", &ledger, &treasury, &policy);
    assert_eq!(elig.reason, Some(IneligibilityReason::BalanceCapped));

    // Spend below cap
    ledger
        .debit("alice", policy.ubi_balance_cap, TransactionReason::Purchase, None)
        .unwrap();

    // 5 & 6. Now eligible
    let elig = ubi.check_eligibility("alice", &ledger, &treasury, &policy);
    assert!(elig.eligible);

    // Claim
    ubi.claim("alice", &mut ledger, &mut treasury, &policy)
        .unwrap();

    // 5. Cooldown → ineligible
    let elig = ubi.check_eligibility("alice", &ledger, &treasury, &policy);
    assert_eq!(elig.reason, Some(IneligibilityReason::OnCooldown));
}

/// Flow-back is marginal and progressive.
#[test]
fn flow_back_progressive() {
    let fb = FlowBack::new();
    let tiers = FlowBackTier::default_tiers();

    // Below threshold: zero
    assert_eq!(fb.calculate(500_000, &tiers), 0);

    // First tier only
    let small = fb.calculate(2_000_000, &tiers);
    assert!(small > 0);

    // Multiple tiers
    let medium = fb.calculate(50_000_000, &tiers);
    assert!(medium > small);

    // Effective rate increases with wealth
    let rate_small = small as f64 / 2_000_000.0;
    let rate_medium = medium as f64 / 50_000_000.0;
    assert!(rate_medium > rate_small);
}

/// Balance conservation invariant.
#[test]
fn balance_conservation() {
    let mut ledger = Ledger::new();

    ledger.credit("alice", 1000, TransactionReason::Initial, None);
    ledger.transfer("alice", "bob", 300, None).unwrap();
    ledger.transfer("bob", "charlie", 100, None).unwrap();

    let total: i64 = ["alice", "bob", "charlie"]
        .iter()
        .map(|p| ledger.balance(p).total())
        .sum();

    // Total Cool is conserved through transfers
    assert_eq!(total, 1000);
}
