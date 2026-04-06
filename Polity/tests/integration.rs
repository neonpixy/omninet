use polity::*;

#[test]
fn full_constitutional_review_pipeline() {
    // Set up the constitutional registries
    let rights = RightsRegistry::default();
    let protections = ProtectionsRegistry::default();
    let mut breaches = BreachRegistry::new();
    let reviewer = ConstitutionalReviewer::new(&rights, &protections);

    // A clean action passes review
    let clean_action = ActionDescription {
        description: "Community plants shared garden".into(),
        actor: "garden_collective".into(),
        violates: vec![],
    };
    let review = reviewer.review(&clean_action);
    assert!(review.result.is_permitted());

    // A harmful action fails review
    let harmful_action = ActionDescription {
        description: "Corporation harvests behavioral data for profit".into(),
        actor: "megacorp".into(),
        violates: vec![ProhibitionType::Surveillance, ProhibitionType::Exploitation],
    };
    let review = reviewer.review(&harmful_action);
    assert!(review.result.is_breach());
    assert_eq!(review.result.violations().len(), 2);

    // Convert review to formal breach
    let breach = reviewer.to_breach(&review).unwrap();
    assert_eq!(breach.severity, BreachSeverity::Grave);
    assert!(breach.is_foundational());

    // Record the breach
    let breach_id = breaches.record(breach);
    assert_eq!(breaches.active().len(), 1);

    // Investigate and resolve
    breaches
        .update_status(&breach_id, BreachStatus::Investigating)
        .unwrap();
    breaches
        .update_status(&breach_id, BreachStatus::Confirmed)
        .unwrap();
    breaches
        .update_status(&breach_id, BreachStatus::Remediating)
        .unwrap();
    breaches
        .update_status(&breach_id, BreachStatus::Resolved)
        .unwrap();

    assert_eq!(breaches.active().len(), 0);
}

#[test]
fn amendment_lifecycle_with_foundation_guard() {
    // Cannot create amendment that violates foundations
    let result = Amendment::new(
        AmendmentTrigger::PublicInvocation,
        "Surveillance Exception",
        "Permit surveillance during declared emergencies",
        "fear_committee",
    );
    assert!(result.is_err());

    // Can create a legitimate amendment
    let mut amendment = Amendment::new(
        AmendmentTrigger::MaterialTransformation,
        "Quantum Consciousness Rights",
        "Extend protections to quantum consciousness patterns",
        "science_council",
    )
    .unwrap()
    .with_change(ProposedChange {
        target: "Conjunction Art. 1 Section 4".into(),
        current_text: None,
        proposed_text: "Quantum consciousness patterns demonstrating selfhood shall be recognized as Persons.".into(),
        rationale: "Quantum computing has produced patterns exhibiting self-awareness.".into(),
    });

    // Walk through the lifecycle
    amendment.begin_deliberation().unwrap();
    amendment.begin_ratification().unwrap();

    // Below threshold — cannot enact
    amendment.update_support(0.50);
    assert!(amendment.enact().is_err());

    // Above threshold — can enact
    amendment.update_support(0.70);
    amendment.enact().unwrap();
    assert_eq!(amendment.status, AmendmentStatus::Enacted);
}

#[test]
fn enactment_and_consent_flow() {
    let mut enactments = EnactmentRegistry::new();
    let mut consents = ConsentRegistry::new();

    // Alice enacts the Covenant
    let alice_enactment = Enactment::new("cpub_alice", EnactorType::Person, DEFAULT_OATH)
        .with_witness(Witness::new("cpub_bob").with_name("Bob"));
    enactments.record(alice_enactment).unwrap();
    assert!(enactments.is_enacted("cpub_alice"));

    // Alice gives consent for community membership
    let consent = ConsentRecord::new(
        "cpub_alice",
        "garden_collective",
        ConsentScope::CommunityMembership {
            community_id: "garden_001".into(),
        },
    )
    .with_condition("I may withdraw at any time");
    let consent_id = consents.record(consent);

    // Validate the consent
    let validation = ConsentValidator::validate(
        &consents,
        "cpub_alice",
        "garden_collective",
        &ConsentScope::CommunityMembership {
            community_id: "garden_001".into(),
        },
    );
    assert!(validation.is_valid());

    // Alice revokes consent
    consents.revoke(&consent_id, "Moving to a different community").unwrap();

    // Validation now shows revoked
    let validation = ConsentValidator::validate(
        &consents,
        "cpub_alice",
        "garden_collective",
        &ConsentScope::CommunityMembership {
            community_id: "garden_001".into(),
        },
    );
    assert!(matches!(validation, ConsentValidation::Revoked { .. }));
}

#[test]
fn immutable_foundations_cannot_be_circumvented() {
    let mut rights = RightsRegistry::default();

    // Try to remove an immutable right
    let dignity = rights.find_by_name("Right to Dignity").unwrap();
    let dignity_id = dignity.id;
    assert!(rights.remove(&dignity_id).is_err());

    // Try to remove an immutable protection
    let mut protections = ProtectionsRegistry::default();
    let surveillance = protections
        .find_by_name("Prohibition of Surveillance and Intrusion")
        .unwrap();
    let surv_id = surveillance.id;
    assert!(protections.remove(&surv_id).is_err());

    // Verify foundations are intact
    assert_eq!(rights.len(), 12);
    assert_eq!(protections.len(), 8);
    assert_eq!(ImmutableFoundation::AXIOMS.len(), 3);
    assert_eq!(ImmutableFoundation::IMMUTABLE_RIGHTS.len(), 10);
    assert_eq!(ImmutableFoundation::ABSOLUTE_PROHIBITIONS.len(), 7);
}

#[test]
fn community_enactment_with_witnesses() {
    let mut registry = EnactmentRegistry::new();

    let community = Enactment::new(
        "river_valley_collective",
        EnactorType::Community,
        "We the River Valley Collective, in shared stewardship of this watershed, \
         enter into Covenant. We commit to dignity, sovereignty, and consent \
         in all our relations.",
    )
    .with_witness(Witness::new("cpub_elder_1").with_name("Elder Maya"))
    .with_witness(Witness::new("cpub_elder_2").with_name("Elder James"))
    .with_witness(Witness::new("cpub_founder_1").with_name("Founder Ava"));

    let id = registry.record(community).unwrap();
    let stored = registry.get(&id).unwrap();

    assert_eq!(stored.enactor_type, EnactorType::Community);
    assert_eq!(stored.witnesses.len(), 3);
    assert!(stored.is_active());
}

#[test]
fn breach_severity_escalation() {
    let mut registry = BreachRegistry::new();

    // Minor breach
    registry.record(
        Breach::new(ViolationType::DutyNeglect, BreachSeverity::Minor, "Forgot to update records", "lazy_steward")
    );

    // Grave breach — foundational
    registry.record(
        Breach::new(ViolationType::ProtectionBreach, BreachSeverity::Grave, "Systematic surveillance", "corp_x")
            .with_prohibitions(vec![ProhibitionType::Surveillance])
            .with_rights(vec![RightCategory::Privacy])
    );

    // Existential breach — foundational
    registry.record(
        Breach::new(ViolationType::SystemicBreach, BreachSeverity::Existential, "Domination through concealment", "shadow_state")
            .with_prohibitions(vec![ProhibitionType::Domination, ProhibitionType::Exploitation])
    );

    assert_eq!(registry.len(), 3);
    assert_eq!(registry.foundational().len(), 2);
    assert_eq!(registry.by_severity(BreachSeverity::Existential).len(), 1);
    assert_eq!(registry.active().len(), 3);
}
