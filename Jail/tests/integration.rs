use jail::*;

/// Full trust graph BFS: alice → bob → carol → dave, query from alice about dave.
#[test]
fn trust_graph_bfs_three_degrees() {
    let mut graph = TrustGraph::new();
    graph
        .add_edge(VerificationEdge::new(
            "alice", "bob", "vouch", VerificationSentiment::Positive, 0.9,
        ))
        .unwrap();
    graph
        .add_edge(VerificationEdge::new(
            "bob", "carol", "vouch", VerificationSentiment::Positive, 0.85,
        ))
        .unwrap();
    graph
        .add_edge(VerificationEdge::new(
            "carol", "dave", "proximity", VerificationSentiment::Neutral, 0.95,
        ))
        .unwrap();

    let config = JailConfig::default();
    let flags: Vec<AccountabilityFlag> = vec![];

    let intel = trust_graph::query_intelligence(&graph, "alice", "dave", &flags, &config);
    assert_eq!(intel.about_pubkey, "dave");
    assert_eq!(intel.verification_count, 1); // carol verified dave
    assert_eq!(intel.closest_degree, Some(2)); // carol is 2 hops from alice
    assert_eq!(intel.pattern, VerificationPattern::Limited);
    assert_eq!(intel.recommendation, TrustRecommendation::Caution); // degree <= 2
    assert_eq!(intel.flag_count, 0);
}

/// Flag lifecycle: raise → review → uphold → community action.
#[test]
fn flag_lifecycle() {
    let flag = AccountabilityFlag::raise(
        "alice",
        "bob",
        FlagCategory::Harassment,
        FlagSeverity::Medium,
        "Repeated unwanted messages after being asked to stop",
    )
    .with_community("community_1")
    .with_context(
        FlagContext::new()
            .with_evidence("sha256_message_log")
            .with_witnesses(2),
    );

    assert_eq!(flag.status, FlagReviewStatus::Pending);

    // Review upholds the flag
    let review = FlagReview::new(flag.id, "reviewer_carol", ReviewOutcome::Upheld)
        .with_action(CommunityAction::Warning)
        .with_notes("Corroborated by 2 witnesses and message logs");

    assert_eq!(review.outcome, ReviewOutcome::Upheld);
    assert_eq!(review.action, Some(CommunityAction::Warning));
}

/// Cross-community pattern: flags from 2+ communities → pattern → duty to warn.
#[test]
fn cross_community_pattern_triggers_warning() {
    let config = JailConfig::default();

    let mut flag1 = AccountabilityFlag::raise(
        "alice", "target_bob", FlagCategory::PredatoryBehavior, FlagSeverity::High, "Concern 1",
    );
    flag1.community_id = Some("community_alpha".into());

    let mut flag2 = AccountabilityFlag::raise(
        "carol", "target_bob", FlagCategory::PredatoryBehavior, FlagSeverity::High, "Concern 2",
    );
    flag2.community_id = Some("community_beta".into());

    let flags = vec![flag1, flag2];
    let pattern = flag::detect_pattern(&flags, "target_bob", &config);

    assert!(pattern.pattern_established);
    assert_eq!(pattern.distinct_communities, 2);
    assert_eq!(pattern.total_flags, 2);

    // Issue duty to warn
    let warning = DutyToWarn::issue(
        "community_alpha",
        "target_bob",
        &pattern,
        &["community_gamma".to_string(), "community_delta".to_string()],
    );
    assert_eq!(warning.warning_records.len(), 2);
    assert!(!warning.fully_acknowledged());
}

/// Graduated response: education → censure → de-escalate → resolve.
#[test]
fn graduated_response_lifecycle() {
    let mut response = GraduatedResponse::begin("bob", "Harassment pattern", "community_council");
    assert_eq!(response.current_level, ResponseLevel::Education);

    // Escalate because education didn't help
    let next = response.escalate("No improvement after 30 days", "community_council").unwrap();
    assert_eq!(next, ResponseLevel::PublicCensure);

    // Person shows improvement — de-escalate
    let prev = response.de_escalate("Significant improvement shown", "community_council").unwrap();
    assert_eq!(prev, ResponseLevel::Education);

    // Resolve
    response.resolve("Matter fully addressed");
    assert!(!response.is_active());
    assert_eq!(response.history.len(), 3);
}

/// Admission: verified prospect admitted; flagged prospect denied.
#[test]
fn admission_check_verified_vs_flagged() {
    let mut graph = TrustGraph::new();
    graph
        .add_edge(VerificationEdge::new(
            "member_alice", "prospect_good", "vouch", VerificationSentiment::Positive, 0.9,
        ))
        .unwrap();
    graph
        .add_edge(VerificationEdge::new(
            "member_alice", "prospect_bad", "vouch", VerificationSentiment::Cautious, 0.5,
        ))
        .unwrap();

    let members = vec!["member_alice".to_string()];
    let config = JailConfig::default();

    // Good prospect: verified, no flags
    let good = check_admission(&graph, "prospect_good", "comm", &members, &[], &config);
    assert_eq!(good.action, AdmissionAction::Admit);

    // Bad prospect: verified but has critical flag from member
    let flags = vec![AccountabilityFlag::raise(
        "member_alice",
        "prospect_bad",
        FlagCategory::PredatoryBehavior,
        FlagSeverity::Critical,
        "Known predatory behavior",
    )];
    let bad = check_admission(&graph, "prospect_bad", "comm", &members, &flags, &config);
    assert_eq!(bad.action, AdmissionAction::Deny);
}

/// Re-verification flow: start → attestations → complete.
#[test]
fn reverification_full_flow() {
    let config = JailConfig {
        reverification_attestations_required: 2,
        ..JailConfig::default()
    };

    let mut session = ReVerificationSession::start(
        "bob",
        ReVerificationReason::VoluntaryUpdate,
        &config,
    );
    assert_eq!(session.state, ReVerificationState::Pending);

    // Collect attestations
    session
        .add_attestation(ReVerificationAttestation::new("alice"))
        .unwrap();
    assert_eq!(session.state, ReVerificationState::Collecting);

    session
        .add_attestation(ReVerificationAttestation::new("carol"))
        .unwrap();

    // Complete
    session.complete().unwrap();
    assert_eq!(session.state, ReVerificationState::Completed);
    assert!(session.completed_at.is_some());
}

/// Appeal lifecycle: flag upheld → appeal → new evidence → reversed.
#[test]
fn appeal_reverses_flag() {
    let flag = AccountabilityFlag::raise(
        "alice", "bob", FlagCategory::Harassment, FlagSeverity::Medium, "Alleged harassment",
    );

    // Flag upheld (simulate review)
    let _review = FlagReview::new(flag.id, "reviewer", ReviewOutcome::Upheld);

    // Bob appeals with new evidence
    let mut appeal = Appeal::file(
        flag.id,
        "bob",
        AppealGround::NewEvidence,
        "Messages show I was responding to their messages, not initiating contact",
    );
    assert_eq!(appeal.status, AppealStatus::Filed);

    appeal.begin_review().unwrap();
    assert_eq!(appeal.status, AppealStatus::UnderReview);

    appeal
        .decide(AppealOutcome {
            decision: AppealDecision::Reversed,
            reasoning: "New evidence shows contact was mutual and consensual".into(),
            decided_by: "appeal_board".into(),
            decided_at: chrono::Utc::now(),
        })
        .unwrap();

    assert_eq!(appeal.status, AppealStatus::Decided);
    assert_eq!(
        appeal.outcome.as_ref().unwrap().decision,
        AppealDecision::Reversed
    );
}

/// Accused rights always preserved — the floor.
#[test]
fn accused_rights_always_preserved() {
    let rights = AccusedRights::always();
    assert!(rights.validate());

    // All 6 rights enumerated
    let enumerated = rights.enumerate();
    assert_eq!(enumerated.len(), 6);

    // Default is also always
    let default_rights = AccusedRights::default();
    assert!(default_rights.validate());
}

/// Anti-weaponization: serial filer detected → consequence recommended.
#[test]
fn anti_weaponization_detects_serial_filing() {
    let config = JailConfig {
        weaponization_window_days: 30,
        weaponization_threshold: 3,
        ..JailConfig::default()
    };

    // Alice files 5 flags in quick succession
    let flags: Vec<AccountabilityFlag> = (0..5)
        .map(|i| {
            AccountabilityFlag::raise(
                "alice",
                format!("target_{i}"),
                FlagCategory::SuspiciousActivity,
                FlagSeverity::Low,
                "suspicious",
            )
        })
        .collect();

    let indicator = flag::detect_serial_filing(
        &flags,
        config.weaponization_window_days,
        config.weaponization_threshold,
    );
    assert!(indicator.is_some());
    assert_eq!(indicator.as_ref().unwrap().pattern, AbusePattern::SerialFiling);

    let consequence = flag::recommend_consequence(&[indicator.unwrap()]);
    assert!(consequence.is_some());
}

/// Protective exclusion with mandatory review and restoration path.
#[test]
fn protective_exclusion_with_restoration() {
    let path = RestorationPath::new(vec![
        "Complete mediation with affected parties".into(),
        "Demonstrate changed behavior for 60 days".into(),
        "Receive positive re-verification from 3 community members".into(),
    ])
    .with_mentor("mentor_carol");

    let mut exclusion = ProtectiveExclusion::new(
        "bob",
        "Persistent pattern of harm across communities",
        "intercommunity_assembly",
        vec!["comm_alpha".into(), "comm_beta".into()],
        90, // review every 90 days
    )
    .with_restoration_path(path);

    assert!(exclusion.is_active());
    assert!(!exclusion.is_review_overdue());
    assert!(!exclusion.restoration_conditions_met());

    // First review: maintain
    exclusion.record_review(ExclusionReview {
        reviewer_pubkey: "reviewer_dave".into(),
        reviewed_at: chrono::Utc::now(),
        decision: ExclusionDecision::Maintain,
        reasoning: "Conditions not yet met".into(),
        next_review: Some(chrono::Utc::now() + chrono::Duration::days(90)),
    });
    assert!(exclusion.is_active());
    assert_eq!(exclusion.reviews.len(), 1);

    // Record progress on restoration path
    if let Some(ref mut path) = exclusion.restoration_path {
        path.record_progress(0, "Mediation completed successfully", true);
        path.record_progress(1, "60 days of positive engagement", true);
        path.record_progress(2, "3 re-verifications received", true);
    }
    assert!(exclusion.restoration_conditions_met());

    // Second review: lift
    exclusion.record_review(ExclusionReview {
        reviewer_pubkey: "reviewer_eve".into(),
        reviewed_at: chrono::Utc::now(),
        decision: ExclusionDecision::Lift,
        reasoning: "All restoration conditions met. Welcome back.".into(),
        next_review: None,
    });
    assert!(!exclusion.is_active());
    assert!(exclusion.lifted_at.is_some());
}
