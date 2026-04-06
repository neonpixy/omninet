use bulwark::*;
use uuid::Uuid;

/// Full trust journey: join → verify → vouch → shield.
#[test]
fn trust_layer_progression() {
    let requirements = LayerTransitionRequirements::default();

    // L1: Connected (open join)
    let chain = TrustChain::connected("alice");
    assert!(!chain.is_in_chain());

    // L1 → L2: Verified (one verification)
    let evidence = LayerTransitionEvidence {
        verification_ids: vec![Uuid::new_v4()], // mutual vouch verification
        ..Default::default()
    };
    assert!(trust::layer_transition::check_transition(
        TrustLayer::Connected,
        TrustLayer::Verified,
        &requirements,
        &evidence,
    )
    .is_ok());

    // L2 → L3: Vouched (2 vouches from L3+ members)
    let evidence = LayerTransitionEvidence {
        vouch_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
        ..Default::default()
    };
    assert!(trust::layer_transition::check_transition(
        TrustLayer::Verified,
        TrustLayer::Vouched,
        &requirements,
        &evidence,
    )
    .is_ok());

    // L3 → L4: Shielded (parent vouch)
    let evidence = LayerTransitionEvidence {
        parent_vouch_pubkey: Some("parent_alice".into()),
        ..Default::default()
    };
    assert!(trust::layer_transition::check_transition(
        TrustLayer::Vouched,
        TrustLayer::Shielded,
        &requirements,
        &evidence,
    )
    .is_ok());
}

/// Cannot skip layers.
#[test]
fn cannot_skip_trust_layers() {
    let requirements = LayerTransitionRequirements::default();
    let evidence = LayerTransitionEvidence::default();

    let result = trust::layer_transition::check_transition(
        TrustLayer::Connected,
        TrustLayer::Vouched,
        &requirements,
        &evidence,
    );
    assert!(result.is_err());
}

/// Kids Sphere family bond flow: parents meet → bond → kids connect.
#[test]
fn kids_sphere_family_bond_flow() {
    // 1. Parents meet and create proximity proof
    let proof = ProximityProof::new("meeting_nonce").with_ble(-40);
    assert!(proof.is_valid());

    // 2. Create family bond (requires proximity)
    let bond = FamilyBond::new("parent_alice", "parent_bob", proof).unwrap();
    assert!(bond.involves_parent("parent_alice"));

    // 3. Kid connection request
    let mut request = KidConnectionRequest::new(
        "kid_a",
        "kid_b",
        "parent_alice",
        "parent_bob",
        bond.id,
    );

    // 4. Parent approves
    request.add_approval(kids_sphere::insulation::ParentApproval {
        parent_pubkey: "parent_alice".into(),
        approved_at: chrono::Utc::now(),
        note: Some("They're in the same class".into()),
    });

    let rules = KidConnectionRules::default();
    request.approve(&rules);
    assert_eq!(request.status, KidConnectionStatus::Approved);
}

/// Family bond REQUIRES proximity — no exceptions.
#[test]
fn family_bond_requires_proximity() {
    let no_evidence = ProximityProof::new("nonce"); // no BLE/NFC/ultrasonic
    let result = FamilyBond::new("parent_a", "parent_b", no_evidence);
    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(BulwarkError::FamilyBondRequiresProximity)
    ));
}

/// Minor registration: siloed → parent linked → authorized.
#[test]
fn minor_registration_flow() {
    let mut minor = SiloedMinor::new("kid", Some(10), MinorDetectionReason::SelfDeclared);
    assert_eq!(minor.state, MinorRegistrationState::Siloed);

    // Can't authorize without parent
    assert!(minor.authorize().is_err());

    // Link parent
    minor
        .link_parent(ParentLink {
            parent_pubkey: "parent".into(),
            relationship: kids_sphere::minor::ParentRelationship::Parent,
            linked_at: chrono::Utc::now(),
        })
        .unwrap();
    assert_eq!(minor.state, MinorRegistrationState::ParentLinked);

    // Authorize
    minor.authorize().unwrap();
    assert!(minor.is_authorized());
}

/// Child safety protocol — all 5 steps enforced.
#[test]
fn child_safety_protocol() {
    // 1. File flag — resources shown immediately
    let flag = ChildSafetyFlag::file(
        "reporter",
        ChildSafetyConcern::Grooming,
        "Adult sending inappropriate messages to child",
    )
    .with_affected_child("kid_alice")
    .with_accused("adult_bob");

    assert!(flag.real_world_resources_shown);
    assert_eq!(flag.status, ChildSafetyStatus::Filed);

    // 2. Silent restriction — accused not notified
    let restriction = SilentRestriction::apply("adult_bob", flag.id);
    assert!(!restriction.notified);

    // 3. Protocol validates all steps
    let protocol = ChildSafetyProtocol::default();
    assert!(protocol.is_valid());
    assert!(protocol.encrypted_flag);
    assert!(protocol.silent_restriction);
    assert!(protocol.reporter_protected);
    assert!(protocol.always_escalate);

    // 4. Real-world resources available
    let resources = RealWorldResources::us_defaults();
    assert!(!resources.emergency_number.is_empty());
    assert!(!resources.crisis_hotline.is_empty());
}

/// Asymmetric bond depth — effective is minimum of both.
#[test]
fn asymmetric_trust_uses_minimum() {
    let mut bond = VisibleBond::new("alice", "bob", BondDepth::Friend);

    // Alice upgrades to Best, Bob stays at Friend
    bond.update_depth("alice", BondDepth::Best);
    assert_eq!(bond.depth_from_a, BondDepth::Best);
    assert_eq!(bond.depth_from_b, BondDepth::Friend);
    assert_eq!(bond.effective_depth(), BondDepth::Friend); // min wins

    // Capabilities governed by effective depth
    let caps = bond.effective_depth().capabilities();
    assert!(caps.can_vouch_adult); // Friend allows adult vouching
    assert!(!caps.can_vouch_minor); // Friend doesn't allow minor vouching (need Best)
}

/// Community health: isolated + autocratic = toxic (cult detection).
#[test]
fn cult_detection() {
    let factors = CollectiveHealthFactors {
        engagement: EngagementDistribution::FewActive,
        communication: CollectiveCommunicationPattern::Hostile,
        cross_membership: CrossMembershipLevel::Isolated, // heaviest weight
        power_distribution: PowerDistribution::Autocratic,
        content_health: CollectiveContentHealth::Concerning,
    };

    let pulse = CollectiveHealthPulse::compute(Uuid::new_v4(), factors, 10);
    assert_eq!(pulse.status, CollectiveHealthStatus::Toxic);
}

/// Reputation: fraud confirmed tanks standing.
#[test]
fn fraud_tanks_reputation() {
    let mut rep = Reputation::new("alice");
    assert_eq!(rep.standing(), Standing::Neutral);

    rep.apply_event(ReputationEvent::new(ReputationEventType::FraudConfirmed));
    assert_eq!(rep.score, 300); // 500 - 200
    assert_eq!(rep.standing(), Standing::Cautioned);
}

/// Vouch rules are stricter for minors.
#[test]
fn minor_vouch_rules_stricter() {
    let rules = VouchRules::default();

    // Adults need 2 vouches at Friend depth
    assert_eq!(rules.for_adult.required_vouch_count, 2);
    assert_eq!(rules.for_adult.required_bond_depth, BondDepth::Friend);
    assert!(!rules.for_adult.requires_parent);

    // Minors need 3 vouches at Best depth, parent required, diversity required
    assert_eq!(rules.for_minor.required_vouch_count, 3);
    assert_eq!(rules.for_minor.required_bond_depth, BondDepth::Best);
    assert!(rules.for_minor.requires_parent);
    assert!(rules.for_minor.requires_diverse_vouchers);
}

/// Age tiers are gradual.
#[test]
fn age_tiers_gradual() {
    let config = AgeTierConfig::default();
    assert_eq!(AgeTier::from_age(8, &config), AgeTier::Kid);
    assert_eq!(AgeTier::from_age(14, &config), AgeTier::Teen);
    assert_eq!(AgeTier::from_age(20, &config), AgeTier::YoungAdult);
    assert_eq!(AgeTier::from_age(30, &config), AgeTier::Adult);

    // Kids and teens are in kids sphere
    assert!(AgeTier::Kid.is_in_kids_sphere());
    assert!(AgeTier::Teen.is_in_kids_sphere());
    assert!(!AgeTier::YoungAdult.is_in_kids_sphere());

    // Only adults can sponsor
    assert!(AgeTier::Adult.can_sponsor());
    assert!(!AgeTier::YoungAdult.can_sponsor());
}

/// Parent oversight scales with age.
#[test]
fn parent_oversight_scales() {
    let kid_oversight = ParentOversight::for_age(8);
    assert!(kid_oversight.view_messages); // full oversight
    assert!(kid_oversight.screen_time_limits);

    let teen_oversight = ParentOversight::for_age(15);
    assert!(!teen_oversight.view_messages); // message privacy
    assert!(!teen_oversight.screen_time_limits); // no screen time
    assert!(teen_oversight.health_alerts); // still get health alerts
}
