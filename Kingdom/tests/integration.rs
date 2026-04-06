use kingdom::*;
use uuid::Uuid;

/// Full governance cycle: create community → charter → propose → deliberate → vote → resolve.
#[test]
fn full_governance_cycle() {
    // 1. Create community with 3 founders
    let mut community = Community::new("River Valley", CommunityBasis::Place);
    community.add_founder("alice");
    community.add_founder("bob");
    community.add_founder("charlie");
    assert_eq!(community.member_count(), 3);

    // 2. Create and sign charter
    let mut charter = Charter::new(community.id, "River Valley Charter", "Steward the watershed");
    charter.sign("alice", "sig_alice");
    charter.sign("bob", "sig_bob");
    charter.sign("charlie", "sig_charlie");
    assert_eq!(charter.signatories().len(), 3);
    assert!(charter.covenant_alignment.is_valid());

    community.charter = Some(charter);
    community.activate().unwrap();

    // 3. Add new member
    community
        .add_member("diana", Some("alice".into()))
        .unwrap();
    assert_eq!(community.member_count(), 4);

    // 4. File a proposal
    let mut proposal = Proposal::new(
        "alice",
        DecidingBody::Community(community.id),
        "Community Garden",
        "Convert the vacant lot at Oak St into a community garden",
    )
    .with_type(ProposalType::Standard)
    .with_quorum(QuorumRequirement::majority());

    // 5. Discuss
    proposal.open_discussion().unwrap();
    proposal.add_discussion(DiscussionPost::new(
        "bob",
        "Great idea! I can help with soil prep.",
    ));
    proposal.add_discussion(DiscussionPost::new(
        "diana",
        "Can we include a section for native plants?",
    ));
    assert_eq!(proposal.discussion.len(), 2);

    // 6. Vote
    let closes = chrono::Utc::now() + chrono::Duration::days(7);
    proposal.open_voting(closes).unwrap();

    proposal
        .add_vote(Vote::new("alice", proposal.id, VotePosition::Support))
        .unwrap();
    proposal
        .add_vote(Vote::new("bob", proposal.id, VotePosition::Support))
        .unwrap();
    proposal
        .add_vote(Vote::new("charlie", proposal.id, VotePosition::Support))
        .unwrap();
    proposal
        .add_vote(Vote::new("diana", proposal.id, VotePosition::Abstain))
        .unwrap();

    // 7. Tally and resolve
    let tally = proposal.tally(4);
    assert!(tally.meets_quorum(&proposal.quorum));

    let process = DirectVoteProcess;
    let result = process.is_resolved(&tally, &proposal.quorum);
    assert_eq!(result, Some(ProposalResult::Passed));

    let outcome = ProposalOutcome::from_tally(&tally, ProposalResult::Passed, &proposal.quorum);
    proposal.resolve(outcome).unwrap();
    assert_eq!(proposal.status, ProposalStatus::Resolved);
    assert!(proposal.outcome.as_ref().unwrap().quorum_met);
}

/// Federation: create consortium → delegate → subsidiarity check → decide.
#[test]
fn federation_governance() {
    // Create two communities
    let mut community_a = Community::new("Upstream Village", CommunityBasis::Place);
    community_a.add_founder("alice");
    community_a.activate().unwrap();

    let mut community_b = Community::new("Downstream Village", CommunityBasis::Place);
    community_b.add_founder("bob");
    community_b.activate().unwrap();

    // Form a consortium
    let charter = ConsortiumCharter::new("Shared watershed stewardship");
    let mut consortium = Consortium::new("Watershed Council", "Protect the river together", charter);

    // Create delegates with mandates
    let mandate_a = Mandate::standard("alice", community_a.id, consortium.id);
    let delegate_a = Delegate::new(
        "alice",
        community_a.id,
        consortium.id,
        mandate_a,
        AppointmentSource::Election(Uuid::new_v4()),
    );

    let mandate_b = Mandate::standard("bob", community_b.id, consortium.id);
    let delegate_b = Delegate::new(
        "bob",
        community_b.id,
        consortium.id,
        mandate_b,
        AppointmentSource::Consensus,
    );

    // Add communities with delegates
    consortium
        .add_member(ConsortiumMember {
            community_id: community_a.id,
            joined_at: chrono::Utc::now(),
            delegates: vec![delegate_a],
        })
        .unwrap();

    consortium
        .add_member(ConsortiumMember {
            community_id: community_b.id,
            joined_at: chrono::Utc::now(),
            delegates: vec![delegate_b],
        })
        .unwrap();

    assert_eq!(consortium.member_count(), 2);
    assert_eq!(consortium.all_delegates().len(), 2);
    assert!(consortium.is_delegate("alice"));

    // Subsidiarity check: water policy affects multiple communities
    let check = SubsidiarityCheck {
        decision_type: ProposalType::Policy,
        proposed_level: GovernanceLevel::Bioregional,
        local_capable: false,
        justification: Some("Water policy affects both upstream and downstream".into()),
    };
    assert!(check.should_elevate());
    assert!(check.validate().is_ok());

    // Check mandate permissions
    let alice_delegate = &consortium.member(&community_a.id).unwrap().delegates[0];
    assert_eq!(
        alice_delegate.mandate.can_decide(&ProposalType::Policy),
        MandateDecision::Authorized
    );
    assert_eq!(
        alice_delegate.mandate.can_decide(&ProposalType::Emergency),
        MandateDecision::Prohibited
    );
}

/// Dispute resolution: file → respond → hear → resolve → appeal → comply.
#[test]
fn full_dispute_resolution() {
    // 1. File dispute
    let mut dispute = Dispute::new(
        "alice",
        "bob",
        DisputeType::ContractBreach,
        DisputeContext::Community(Uuid::new_v4()),
        "Bob agreed to share tools but hasn't returned them in 3 months",
    )
    .with_evidence(vec![EvidenceItem::new(
        EvidenceType::Communication,
        "Messages showing agreement to return tools by January",
    )]);

    // 2. Require and receive response
    dispute.require_response().unwrap();
    let response = DisputeResponse::new(
        "bob",
        adjudication::DisputeResponsePosition::Partial,
        "I had the tools but was using them for community repairs",
    );
    dispute.add_response(response).unwrap();

    // 3. Schedule hearing
    dispute.advance_to_hearing().unwrap();

    // 4. Complete hearing
    let mut hearing = HearingRecord::new(
        dispute.id,
        chrono::Utc::now() + chrono::Duration::days(7),
        HearingFormat::Video,
    );
    hearing.add_participant(HearingParticipant {
        pubkey: "alice".into(),
        role: ParticipantRole::Complainant,
        attended: true,
        notes: None,
    });
    hearing.add_participant(HearingParticipant {
        pubkey: "bob".into(),
        role: ParticipantRole::Respondent,
        attended: true,
        notes: None,
    });
    hearing.complete("Both parties presented their case", 60);
    assert!(hearing.is_completed());

    // 5. Resolve
    dispute.advance_to_resolution().unwrap();

    let adj_id = Uuid::new_v4();
    let resolution = Resolution::new(
        dispute.id,
        vec![adj_id],
        DecisionOutcome::Split,
        "Both parties bear some responsibility",
    )
    .with_findings(vec![
        Finding::new(
            "Agreement to return tools existed",
            FindingConfidence::Established,
        ),
        Finding::new(
            "Community repairs justified temporary retention",
            FindingConfidence::Preponderance,
        ),
    ])
    .with_remedies(vec![
        OrderedRemedy::new(
            RemedyAction::Restitution,
            "bob",
            "alice",
            "Return tools within 7 days",
        ),
        OrderedRemedy::new(
            RemedyAction::StructuralChange,
            "community",
            "all",
            "Create a tool-sharing registry",
        ),
    ]);

    dispute.resolve().unwrap();
    assert!(!dispute.is_active());
    assert!(!resolution.appeal_period_passed());

    // 6. Appeal
    let mut appeal = Appeal::new(
        resolution.id,
        dispute.id,
        "bob",
        vec![AppealGround::RemedyError],
        "7 days is too short given the community repair schedule",
    );
    appeal.begin_review().unwrap();

    let outcome = AppealOutcome::new(
        AppealDecision::Modified,
        "Extend return deadline to 14 days",
        vec![Uuid::new_v4()],
    );
    appeal.decide(outcome).unwrap();
    assert_eq!(appeal.status, AppealStatus::Decided);

    // 7. Compliance
    let mut compliance = ComplianceRecord::new(resolution.id, resolution.remedies[0].id, "bob");
    compliance.verify("steward_charlie", Some("Tools returned in good condition".into()));
    assert!(compliance.is_verified());
}

/// Challenge: file challenge → respond → uphold.
#[test]
fn public_challenge_lifecycle() {
    let mut challenge = Challenge::new(
        "diana",
        ChallengeType::PowerConcentration,
        ChallengeTarget::GovernanceStructure(Uuid::new_v4()),
        "Steward has served 3 consecutive terms without rotation",
    );

    // Gather co-signers (Constellation Art. 5 §8: collective challenge thresholds)
    challenge.add_co_signer("eve");
    challenge.add_co_signer("frank");
    assert_eq!(challenge.co_signers.len(), 2);

    // Governance body responds
    let response = ChallengeResponse::new(
        "steward_bob",
        ResponsePosition::Acknowledge,
        "We acknowledge the term limit violation",
    );
    challenge.respond(response).unwrap();

    // Challenge upheld
    challenge.uphold().unwrap();
    assert_eq!(challenge.status, ChallengeStatus::Upheld);
    assert!(!challenge.is_active());
}

/// Union lifecycle: form → live → dissolve.
#[test]
fn union_formation_and_dissolution() {
    // Form a chosen family union
    let mut union = Union::new("The Hearth", UnionType::ChosenFamily);

    // Formation with unanimous consent
    let mut formation = UnionFormation::new();
    formation.add_consent("alice", "sig_alice");
    formation.add_consent("bob", "sig_bob");
    formation.add_consent("charlie", "sig_charlie");

    let member_keys: Vec<String> = vec!["alice".into(), "bob".into(), "charlie".into()];
    assert!(formation.all_consented(&member_keys));

    union.formation = Some(formation);

    // Add members
    for name in &member_keys {
        union
            .add_member(UnionMember {
                pubkey: name.clone(),
                role: None,
                joined_at: chrono::Utc::now(),
            })
            .unwrap();
    }
    assert_eq!(union.member_count(), 3);

    // Live as a union...

    // Dissolve — "Where consent ends, the Union ends"
    let record = DissolutionRecord {
        requested_by: "alice".into(),
        reason: Some("Members moving to different regions".into()),
        member_consent: vec![],
        asset_distribution: AssetDistribution::EqualSplit,
        finalized_at: chrono::Utc::now(),
    };
    union.dissolve(record).unwrap();
    assert_eq!(union.status, UnionStatus::Dissolved);
}

/// Liquid democracy: delegate chain resolution with cycle detection.
#[test]
fn liquid_democracy_delegation_chain() {
    let pid = Uuid::new_v4();

    // alice → bob → charlie (transitive)
    let delegations = vec![
        VoteDelegation {
            delegator: "alice".into(),
            delegate: "bob".into(),
            scope: DelegationScope::All,
            active: true,
        },
        VoteDelegation {
            delegator: "bob".into(),
            delegate: "charlie".into(),
            scope: DelegationScope::All,
            active: true,
        },
    ];

    // Resolves through the chain
    let resolved =
        LiquidDemocracyProcess::resolve_delegation("alice", &delegations, pid).unwrap();
    assert_eq!(resolved, "charlie");

    // Charlie votes, carries weight for alice and bob
    let votes = vec![Vote::new("charlie", pid, VotePosition::Support)];
    let eligible = vec!["alice".into(), "bob".into(), "charlie".into()];

    let effective = LiquidDemocracyProcess::resolve_votes(&votes, &delegations, &eligible, pid).unwrap();

    // Charlie's vote should carry weight 3 (self + alice + bob)
    let charlie_vote = effective.iter().find(|v| v.voter == "charlie").unwrap();
    assert_eq!(charlie_vote.weight, 3.0);

    // Cycle detection
    let cyclic = vec![
        VoteDelegation {
            delegator: "x".into(),
            delegate: "y".into(),
            scope: DelegationScope::All,
            active: true,
        },
        VoteDelegation {
            delegator: "y".into(),
            delegate: "x".into(),
            scope: DelegationScope::All,
            active: true,
        },
    ];
    assert!(LiquidDemocracyProcess::resolve_delegation("x", &cyclic, pid).is_err());
}

/// Assembly convocation and record keeping.
#[test]
fn assembly_convocation() {
    let mut assembly = Assembly::new(
        "Spring Assembly",
        AssemblyType::CommunityAssembly,
        "steward_alice",
        "Review winter progress, plan spring planting",
        ConvocationTrigger::SeasonalReview,
    );

    assembly.add_participant("alice");
    assembly.add_participant("bob");
    assembly.add_participant("charlie");
    assembly.add_participant("diana");
    assert_eq!(assembly.participant_count(), 4);

    assembly.begin().unwrap();

    assembly
        .add_record(AssemblyRecord::new(
            RecordType::Agenda,
            "1. Winter review 2. Spring planning 3. Tool sharing",
            "alice",
        ))
        .unwrap();

    assembly
        .add_record(AssemblyRecord::new(
            RecordType::Decision,
            "Approved: community tool library with rotating stewardship",
            "alice",
        ))
        .unwrap();

    assembly.conclude().unwrap();
    assert!(assembly.is_concluded());
    assert_eq!(assembly.records.len(), 2);
}

/// Dissolved community cannot accept proposals.
#[test]
fn dissolved_community_invariant() {
    let mut community = Community::new("Temp", CommunityBasis::Digital);
    community.add_founder("alice");
    community.activate().unwrap();
    community.begin_dissolution().unwrap();
    community.dissolve().unwrap();

    // Can't add members
    assert!(community.add_member("bob", None).is_err());
}

/// Recalled delegate's mandate is inactive.
#[test]
fn recalled_delegate_invariant() {
    let mut mandate = Mandate::standard("alice", Uuid::new_v4(), Uuid::new_v4());
    assert!(mandate.is_active());

    mandate.recalled_at = Some(chrono::Utc::now());
    assert!(!mandate.is_active());
    assert!(mandate.is_recalled());
}
