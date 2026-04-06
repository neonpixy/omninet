//! Phase 4 Vertical Slice — Civilization Gate Test
//!
//! Proves: "A community can form, govern itself, and trade using Cool."
//!
//! Full narrative: Alice and Bob found a cooperative. Charlie joins through
//! the admission system. Diana is a bad actor who gets flagged across
//! communities. The Covenant guards everything.
//!
//! Crates exercised: Crown, Globe, Lingo, Ideas, Sentinal, X,
//!                   Polity, Kingdom, Fortune, Bulwark, Jail.
//!
//! Phase 3 vertical slice proved: "A person has an identity and can connect
//! to Omnidea relays." Phase 4 proves: "A community can form, govern itself,
//! trade using Cool, and hold members accountable."

use chrono::Utc;
use crown::CrownKeypair;
use ideas::Digit;
use lingo::Babel;
use x::Value;

// ==========================================================================
// Test 1: The Full Civilization Lifecycle
// ==========================================================================
//
// The complete story: identity → enactment → trust → community → charter →
// economics → admission → proposal → vote → content → accountability.
//
// This single test proves the Phase 4 gate item.

#[test]
fn civilization_lifecycle() {
    // =====================================================================
    // ACT 1: Identity — Crown generates keypairs
    // =====================================================================

    let alice = CrownKeypair::generate();
    let bob = CrownKeypair::generate();
    let charlie = CrownKeypair::generate();
    let diana = CrownKeypair::generate(); // the bad actor

    let alice_pk = alice.public_key_hex();
    let bob_pk = bob.public_key_hex();
    let charlie_pk = charlie.public_key_hex();
    let diana_pk = diana.public_key_hex();

    // =====================================================================
    // ACT 2: Covenant Enactment — Polity (voluntary, witnessed)
    // =====================================================================

    let mut enactment_registry = polity::EnactmentRegistry::new();

    // Alice enacts the Covenant, witnessed by Bob
    let alice_enactment = polity::Enactment::new(
        &alice_pk,
        polity::EnactorType::Person,
        polity::DEFAULT_OATH,
    )
    .with_witness(polity::Witness::new(&bob_pk).with_name("Bob"));

    let alice_enactment_id = enactment_registry.record(alice_enactment).unwrap();

    // Bob enacts, witnessed by Alice
    let bob_enactment = polity::Enactment::new(
        &bob_pk,
        polity::EnactorType::Person,
        polity::DEFAULT_OATH,
    )
    .with_witness(polity::Witness::new(&alice_pk).with_name("Alice"));

    enactment_registry.record(bob_enactment).unwrap();

    // Verify both are active
    assert!(enactment_registry.get(&alice_enactment_id).unwrap().is_active());
    assert!(enactment_registry.is_enacted(&bob_pk));

    // =====================================================================
    // ACT 3: Trust Building — Jail trust graph + Bulwark reputation
    // =====================================================================

    // Build the web of trust
    let mut trust_graph = jail::TrustGraph::new();

    // Alice ↔ Bob: mutual vouch (high confidence)
    trust_graph
        .add_edge(jail::VerificationEdge::new(
            &alice_pk,
            &bob_pk,
            "mutual_vouch",
            jail::VerificationSentiment::Positive,
            0.95,
        ))
        .unwrap();
    trust_graph
        .add_edge(jail::VerificationEdge::new(
            &bob_pk,
            &alice_pk,
            "mutual_vouch",
            jail::VerificationSentiment::Positive,
            0.95,
        ))
        .unwrap();

    // Alice → Charlie: vouch (moderate confidence)
    trust_graph
        .add_edge(jail::VerificationEdge::new(
            &alice_pk,
            &charlie_pk,
            "vouch",
            jail::VerificationSentiment::Positive,
            0.7,
        ))
        .unwrap();

    // Bob → Charlie: vouch
    trust_graph
        .add_edge(jail::VerificationEdge::new(
            &bob_pk,
            &charlie_pk,
            "digital_verification",
            jail::VerificationSentiment::Positive,
            0.6,
        ))
        .unwrap();

    assert_eq!(trust_graph.node_count(), 3); // Diana not in graph yet
    assert_eq!(trust_graph.edge_count(), 4);

    // Verify BFS from Alice can reach Charlie
    let alice_network = trust_graph.verified_by(&alice_pk);
    assert!(alice_network.contains(&bob_pk));
    assert!(alice_network.contains(&charlie_pk));

    // Initialize reputations (Bulwark)
    let mut alice_rep = bulwark::Reputation::new(&alice_pk);
    let mut bob_rep = bulwark::Reputation::new(&bob_pk);
    let charlie_rep = bulwark::Reputation::new(&charlie_pk);

    // Alice and Bob get endorsement bumps for mutual trust
    alice_rep.apply_event(bulwark::ReputationEvent::new(
        bulwark::ReputationEventType::EndorsementReceived,
    ));
    bob_rep.apply_event(bulwark::ReputationEvent::new(
        bulwark::ReputationEventType::EndorsementReceived,
    ));

    assert!(alice_rep.score > 500); // above neutral
    assert!(bob_rep.score > 500);
    assert_eq!(charlie_rep.score, 500); // neutral start

    // =====================================================================
    // ACT 4: Community Formation — Kingdom
    // =====================================================================

    // Alice and Bob found the Garden Cooperative
    let mut community = kingdom::Community::new("Garden Cooperative", kingdom::CommunityBasis::Interest);
    let community_id = community.id;

    // Create a charter with Covenant alignment
    let mut charter = kingdom::Charter::new(
        community_id,
        "Garden Cooperative Charter",
        "A cooperative dedicated to community gardening and food sovereignty",
    )
    .with_values(vec![
        "Sustainability".into(),
        "Shared stewardship".into(),
        "Open participation".into(),
    ])
    .with_governance(kingdom::GovernanceStructure {
        decision_process: "direct_vote".into(),
        quorum_participation: 0.5,
        quorum_approval: 0.5,
        ..Default::default()
    });

    // Founders sign the charter
    charter.sign(&alice_pk, "alice_charter_sig");
    charter.sign(&bob_pk, "bob_charter_sig");
    assert_eq!(charter.signatories().len(), 2);
    assert!(charter.covenant_alignment.is_valid());

    // Attach charter, add founders, activate
    community = community.with_charter(charter);
    community.add_founder(&alice_pk);
    community.add_founder(&bob_pk);
    community.activate().unwrap();

    assert!(community.is_active());
    assert_eq!(community.member_count(), 2);
    assert!(community.is_founder(&alice_pk));

    // =====================================================================
    // ACT 5: Constitutional Review — Polity guards the formation
    // =====================================================================

    let rights = polity::RightsRegistry::default();
    let protections = polity::ProtectionsRegistry::default();
    let reviewer = polity::ConstitutionalReviewer::new(&rights, &protections);

    // The community formation is a lawful action
    let formation_action = polity::ActionDescription {
        description: "Form a cooperative community for gardening".into(),
        actor: alice_pk.clone(),
        violates: vec![], // clean action
    };
    let review = reviewer.review(&formation_action);
    assert!(review.result.is_permitted());

    // =====================================================================
    // ACT 6: Economics — Fortune (treasury + UBI + ledger)
    // =====================================================================

    let policy = fortune::FortunePolicy::testing();
    let mut treasury = fortune::Treasury::new(policy.clone());

    // Set up network metrics (community exists, 2 members, 0 ideas so far)
    treasury.update_metrics(fortune::NetworkMetrics {
        active_users: 2,
        total_ideas: 0,
        total_collectives: 1,
    });

    assert!(treasury.max_supply() > 0);
    let initial_supply = treasury.max_supply();

    // Mint initial Cool for founders
    let mut ledger = fortune::Ledger::new();
    let minted = treasury
        .mint(policy.initial_mint_amount, &alice_pk, fortune::MintReason::Initial)
        .unwrap();
    ledger.credit(&alice_pk, minted, fortune::TransactionReason::Initial, None);

    let minted = treasury
        .mint(policy.initial_mint_amount, &bob_pk, fortune::MintReason::Initial)
        .unwrap();
    ledger.credit(&bob_pk, minted, fortune::TransactionReason::Initial, None);

    assert_eq!(ledger.balance(&alice_pk).liquid, policy.initial_mint_amount);
    assert_eq!(ledger.balance(&bob_pk).liquid, policy.initial_mint_amount);

    // UBI distribution
    let mut ubi = fortune::UbiDistributor::new();
    ubi.verify_identity(&alice_pk);
    ubi.verify_identity(&bob_pk);

    let claim = ubi.claim(&alice_pk, &mut ledger, &mut treasury, &policy).unwrap();
    assert_eq!(claim.amount, policy.ubi_amount);
    assert_eq!(
        ledger.balance(&alice_pk).liquid,
        policy.initial_mint_amount + policy.ubi_amount
    );

    // =====================================================================
    // ACT 7: Admission — Jail checks Charlie for community entry
    // =====================================================================

    let jail_config = jail::JailConfig::testing();
    let all_flags: Vec<jail::AccountabilityFlag> = Vec::new(); // no flags yet

    let community_members = vec![alice_pk.clone(), bob_pk.clone()];
    let admission = jail::check_admission(
        &trust_graph,
        &charlie_pk,
        &community_id.to_string(),
        &community_members,
        &all_flags,
        &jail_config,
    );

    assert_eq!(
        admission.action,
        jail::AdmissionAction::Admit,
        "Charlie has 2 verifications from community members — should be admitted"
    );
    assert_eq!(admission.verification_count, 2);

    // Charlie joins the community
    community.add_member(&charlie_pk, Some(alice_pk.clone())).unwrap();
    assert_eq!(community.member_count(), 3);
    assert_eq!(
        community.member(&charlie_pk).unwrap().role,
        kingdom::CommunityRole::Newcomer
    );

    // Charlie also gets initial Cool
    ubi.verify_identity(&charlie_pk);
    let charlie_mint = treasury
        .mint(policy.initial_mint_amount, &charlie_pk, fortune::MintReason::Initial)
        .unwrap();
    ledger.credit(&charlie_pk, charlie_mint, fortune::TransactionReason::Initial, None);

    // =====================================================================
    // ACT 8: Proposal & Voting — Kingdom governance in action
    // =====================================================================

    // Alice proposes: "Fund the community garden"
    let mut proposal = kingdom::Proposal::new(
        &alice_pk,
        kingdom::DecidingBody::Community(community_id),
        "Fund the community garden",
        "Allocate 500 Cool from treasury for seeds, tools, and soil amendments",
    )
    .with_type(kingdom::ProposalType::Treasury)
    .with_quorum(kingdom::QuorumRequirement::majority());

    // Discussion phase
    proposal.open_discussion().unwrap();
    proposal.add_discussion(kingdom::DiscussionPost::new(
        &bob_pk,
        "Great idea! I can contribute extra labor for planting day.",
    ));
    let post_id = proposal.discussion[0].id;
    proposal.add_discussion(kingdom::DiscussionPost::reply(
        &charlie_pk,
        "I support this. Can we add composting equipment to the budget?",
        post_id,
    ));
    assert_eq!(proposal.discussion.len(), 2);

    // Voting phase
    let closes_at = Utc::now() + chrono::Duration::days(7);
    proposal.open_voting(closes_at).unwrap();
    assert!(proposal.is_voting_open());

    // All three members vote
    proposal
        .add_vote(kingdom::Vote::new(&alice_pk, proposal.id, kingdom::VotePosition::Support))
        .unwrap();
    proposal
        .add_vote(
            kingdom::Vote::new(&bob_pk, proposal.id, kingdom::VotePosition::Support)
                .with_reason("Fully aligned with our charter values"),
        )
        .unwrap();
    proposal
        .add_vote(kingdom::Vote::new(
            &charlie_pk,
            proposal.id,
            kingdom::VotePosition::Support,
        ))
        .unwrap();

    // Tally using DirectVoteProcess
    let eligible_voters = community.member_count() as u32;
    let tally = proposal.tally(eligible_voters);
    let process = kingdom::DirectVoteProcess;

    use kingdom::DecisionProcess;
    let result = process
        .is_resolved(&tally, &proposal.quorum)
        .expect("quorum met, should resolve");
    assert_eq!(result, kingdom::ProposalResult::Passed);

    // Resolve the proposal
    let outcome = kingdom::ProposalOutcome::from_tally(&tally, result, &proposal.quorum);
    assert!(outcome.quorum_met);
    assert_eq!(outcome.participation_rate(), 1.0); // 100% participation
    assert_eq!(outcome.support_rate(), 1.0); // unanimous

    proposal.resolve(outcome).unwrap();
    assert_eq!(proposal.status, kingdom::ProposalStatus::Resolved);

    // =====================================================================
    // ACT 9: Content — Ideas creates a record of the vote
    // =====================================================================

    let vote_record = Digit::new(
        "governance.vote-record".into(),
        Value::from("Community garden proposal passed unanimously"),
        alice.crown_id().to_string(),
    )
    .unwrap();

    assert_eq!(vote_record.digit_type(), "governance.vote-record");

    // Babel-encode the record for storage
    let master_key = vec![0x42u8; 32];
    let vocab_seed = sentinal::key_derivation::derive_vocabulary_seed(&master_key).unwrap();
    let babel = Babel::new(vocab_seed.expose());

    let original_content = "Community garden proposal passed unanimously";
    let encoded = babel.encode(original_content);
    assert_ne!(encoded, original_content);

    let decoded = babel.decode(&encoded);
    assert_eq!(decoded, original_content);

    // Full JSON round-trip through Babel
    let vote_json = serde_json::to_string(&vote_record).unwrap();
    let encoded_json = babel.encode(&vote_json);
    let decoded_json = babel.decode(&encoded_json);
    let recovered: Digit = serde_json::from_str(&decoded_json).unwrap();
    assert_eq!(recovered.id(), vote_record.id());

    // =====================================================================
    // ACT 10: Economic Activity — Fortune transfers
    // =====================================================================

    // Alice transfers Cool to Bob for garden supplies
    ledger
        .transfer(&alice_pk, &bob_pk, 50, Some("Garden supplies".into()))
        .unwrap();

    let alice_summary = ledger.summary(&alice_pk);
    assert!(alice_summary.transfers_sent > 0);

    let bob_summary = ledger.summary(&bob_pk);
    assert!(bob_summary.transfers_received > 0);

    // Verify treasury utilization
    let status = treasury.status();
    assert!(status.utilization > 0.0);
    assert!(status.in_circulation > 0);

    // Update metrics after Charlie joined and vote record was created
    treasury.update_metrics(fortune::NetworkMetrics {
        active_users: 3,
        total_ideas: 1,
        total_collectives: 1,
    });
    assert!(treasury.max_supply() > initial_supply, "supply grows with network");

    // =====================================================================
    // ACT 11: The Bad Actor — Diana gets flagged (Jail)
    // =====================================================================

    // Diana gets flagged by Alice in the Garden Cooperative
    let flag1 = jail::AccountabilityFlag::raise(
        &alice_pk,
        &diana_pk,
        jail::FlagCategory::Harassment,
        jail::FlagSeverity::Medium,
        "Repeated unwanted messages to multiple community members",
    )
    .with_community(community_id.to_string());

    // Diana also gets flagged in a DIFFERENT community (establishing a pattern)
    let other_community_id = uuid::Uuid::new_v4();
    let external_flagger = CrownKeypair::generate();
    let flag2 = jail::AccountabilityFlag::raise(
        external_flagger.public_key_hex(),
        &diana_pk,
        jail::FlagCategory::Harassment,
        jail::FlagSeverity::Medium,
        "Aggressive behavior in community forums",
    )
    .with_community(other_community_id.to_string());

    let all_flags = vec![flag1, flag2];

    // Detect cross-community pattern
    let pattern = jail::flag::detect_pattern(&all_flags, &diana_pk, &jail_config);
    assert!(
        pattern.pattern_established,
        "2 distinct communities flagged Diana — pattern established"
    );
    assert_eq!(pattern.distinct_communities, 2);
    assert_eq!(pattern.total_flags, 2);

    // =====================================================================
    // ACT 12: Constitutional Review of Bad Action — Polity
    // =====================================================================

    let bad_action = polity::ActionDescription {
        description: "Harass community members via repeated unwanted messages".into(),
        actor: diana_pk.clone(),
        violates: vec![polity::ProhibitionType::Domination],
    };

    let bad_review = reviewer.review(&bad_action);
    assert!(bad_review.result.is_breach());
    assert!(!bad_review.result.violations().is_empty());

    // Convert to formal breach
    let breach = reviewer.to_breach(&bad_review).unwrap();
    assert_eq!(breach.actor, diana_pk);

    // =====================================================================
    // ACT 13: Graduated Response — Jail (education → censure)
    // =====================================================================

    // Start at Education (always start at the bottom)
    let mut response = jail::GraduatedResponse::begin(
        &diana_pk,
        "Cross-community harassment pattern established",
        &alice_pk,
    );

    assert_eq!(response.current_level, jail::ResponseLevel::Education);
    assert!(response.current_level.is_reversible());

    // Diana doesn't respond — escalate to Public Censure
    let next = response
        .escalate(
            "No response to educational outreach after 30 days",
            &alice_pk,
        )
        .unwrap();
    assert_eq!(next, jail::ResponseLevel::PublicCensure);
    assert_eq!(response.history.len(), 2);

    // =====================================================================
    // ACT 14: Accused Rights — Jail (always on, no exceptions)
    // =====================================================================

    let diana_rights = jail::AccusedRights::always();
    assert!(diana_rights.validate()); // all 6 rights preserved
    assert!(diana_rights.right_to_know_charges);
    assert!(diana_rights.right_to_respond);
    assert!(diana_rights.right_to_challenge);
    assert!(diana_rights.right_to_present_evidence);
    assert!(diana_rights.right_to_appeal);
    assert!(diana_rights.right_to_proportional_response);

    // Reporter protection
    let reporter_protection = jail::ReporterProtection::for_flag(
        &all_flags[0].flagger_pubkey,
        all_flags[0].id,
    );
    assert!(reporter_protection.identity_protected);
    assert!(reporter_protection.retaliation_monitored);

    // =====================================================================
    // ACT 15: Admission Denied — Diana tries to join Garden Cooperative
    // =====================================================================

    let diana_admission = jail::check_admission(
        &trust_graph,
        &diana_pk,
        &community_id.to_string(),
        &community_members,
        &all_flags,
        &jail_config,
    );

    // Diana has flags from community members — should NOT be admitted
    assert_ne!(
        diana_admission.action,
        jail::AdmissionAction::Admit,
        "Diana has flags — should not be simply admitted"
    );

    // =====================================================================
    // ACT 16: Consent Lifecycle — Polity (grant → active → revoke)
    // =====================================================================

    let mut consent = polity::ConsentRecord::new(
        &charlie_pk,
        community_id.to_string(),
        polity::ConsentScope::CommunityMembership {
            community_id: community_id.to_string(),
        },
    )
    .with_condition("Charter alignment required");

    assert!(consent.is_active());
    assert!(!consent.is_revoked());

    // Revoke consent — always available, no questions asked
    consent.revoke("Personal decision to leave").unwrap();
    assert!(!consent.is_active());
    assert!(consent.is_revoked());

    // =====================================================================
    // FINAL: Cross-crate data serialization
    // =====================================================================

    // Serialize community state + economic state + trust graph
    let community_json = serde_json::to_string(&community).unwrap();
    let trust_json = serde_json::to_string(&trust_graph).unwrap();
    let ledger_json = serde_json::to_string(&ledger).unwrap();

    // Deserialize and verify
    let restored_community: kingdom::Community = serde_json::from_str(&community_json).unwrap();
    assert_eq!(restored_community.name, "Garden Cooperative");
    assert_eq!(restored_community.member_count(), 3);

    let restored_graph: jail::TrustGraph = serde_json::from_str(&trust_json).unwrap();
    assert_eq!(restored_graph.edge_count(), 4);

    let restored_ledger: fortune::Ledger = serde_json::from_str(&ledger_json).unwrap();
    assert!(restored_ledger.balance(&alice_pk).liquid > 0);
}

// ==========================================================================
// Test 2: Full Economics Pipeline
// ==========================================================================
//
// Treasury → Mint → UBI → Transfer → Demurrage → Cash → Redeem.
// Proves the complete Cool lifecycle.

#[test]
fn economics_pipeline() {
    let alice = CrownKeypair::generate();
    let bob = CrownKeypair::generate();
    let alice_pk = alice.public_key_hex();
    let bob_pk = bob.public_key_hex();

    let policy = fortune::FortunePolicy::testing();
    let mut treasury = fortune::Treasury::new(policy.clone());
    treasury.update_metrics(fortune::NetworkMetrics {
        active_users: 10,
        total_ideas: 5,
        total_collectives: 1,
    });

    let mut ledger = fortune::Ledger::new();

    // 1. Mint initial Cool
    let mint_amount = 1000;
    treasury
        .mint_exact(mint_amount, &alice_pk, fortune::MintReason::Initial)
        .unwrap();
    ledger.credit(&alice_pk, mint_amount, fortune::TransactionReason::Initial, None);
    assert_eq!(ledger.balance(&alice_pk).liquid, 1000);

    // 2. UBI claim
    let mut ubi = fortune::UbiDistributor::new();
    ubi.verify_identity(&alice_pk);
    let claim = ubi.claim(&alice_pk, &mut ledger, &mut treasury, &policy).unwrap();
    assert_eq!(claim.amount, policy.ubi_amount);

    // 3. Transfer
    ledger.transfer(&alice_pk, &bob_pk, 200, None).unwrap();
    assert_eq!(ledger.balance(&bob_pk).liquid, 200);

    // 4. Demurrage
    let decay = ledger.apply_demurrage(&alice_pk, 10);
    assert_eq!(decay, 10);
    let alice_bal = ledger.balance(&alice_pk);
    assert_eq!(alice_bal.liquid, 1000 + policy.ubi_amount - 200 - 10);

    // 5. Lock for Cash
    ledger.lock(&bob_pk, 100, "ABCD-EFGH-JKLM").unwrap();
    let bob_bal = ledger.balance(&bob_pk);
    assert_eq!(bob_bal.liquid, 100); // 200 - 100 locked
    assert_eq!(bob_bal.locked, 100);
    assert_eq!(bob_bal.total(), 200); // total unchanged

    treasury.lock_for_cash(100);

    // 6. Cash redemption (unlock without returning to issuer)
    ledger.unlock(&bob_pk, 100, "ABCD-EFGH-JKLM", false);
    treasury.unlock_from_cash(100);
    let bob_bal = ledger.balance(&bob_pk);
    assert_eq!(bob_bal.locked, 0);
    assert_eq!(bob_bal.liquid, 100); // 200 - 100 (redeemed by someone else)

    // Transaction summaries
    let alice_summary = ledger.summary(&alice_pk);
    assert!(alice_summary.transfers_sent > 0);
    assert!(alice_summary.demurrage_paid > 0);

    let bob_summary = ledger.summary(&bob_pk);
    assert!(bob_summary.transfers_received > 0);
}

// ==========================================================================
// Test 3: Multi-Community Trust & Accountability
// ==========================================================================
//
// Two communities. A bad actor flagged in both. Pattern detection triggers
// duty to warn. Graduated response from education to economic disengagement.
// Appeal filed. Accused rights preserved throughout.

#[test]
fn multi_community_accountability() {
    let alice = CrownKeypair::generate();
    let bob = CrownKeypair::generate();
    let charlie = CrownKeypair::generate();
    let diana = CrownKeypair::generate(); // flagged person
    let eve = CrownKeypair::generate(); // member of second community

    let alice_pk = alice.public_key_hex();
    let bob_pk = bob.public_key_hex();
    let charlie_pk = charlie.public_key_hex();
    let diana_pk = diana.public_key_hex();
    let eve_pk = eve.public_key_hex();

    let config = jail::JailConfig::testing();

    // Build trust graph — two overlapping communities
    let mut graph = jail::TrustGraph::new();
    // Community A members verify each other
    graph
        .add_edge(jail::VerificationEdge::new(
            &alice_pk, &bob_pk, "mutual_vouch", jail::VerificationSentiment::Positive, 0.9,
        ))
        .unwrap();
    graph
        .add_edge(jail::VerificationEdge::new(
            &bob_pk, &alice_pk, "mutual_vouch", jail::VerificationSentiment::Positive, 0.9,
        ))
        .unwrap();
    // Community B members
    graph
        .add_edge(jail::VerificationEdge::new(
            &charlie_pk, &eve_pk, "mutual_vouch", jail::VerificationSentiment::Positive, 0.85,
        ))
        .unwrap();

    // Diana verified by Alice (she's in the network)
    graph
        .add_edge(jail::VerificationEdge::new(
            &alice_pk, &diana_pk, "vouch", jail::VerificationSentiment::Neutral, 0.5,
        ))
        .unwrap();

    // Diana gets flagged in Community A
    let community_a = uuid::Uuid::new_v4();
    let community_b = uuid::Uuid::new_v4();

    let flag_a = jail::AccountabilityFlag::raise(
        &alice_pk,
        &diana_pk,
        jail::FlagCategory::Inappropriate,
        jail::FlagSeverity::Medium,
        "Consistently disruptive in community discussions",
    )
    .with_community(community_a.to_string());

    let flag_b = jail::AccountabilityFlag::raise(
        &bob_pk,
        &diana_pk,
        jail::FlagCategory::Inappropriate,
        jail::FlagSeverity::Low,
        "Disrespectful behavior during collective meetings",
    )
    .with_community(community_a.to_string());

    // Diana flagged in Community B by Eve
    let flag_c = jail::AccountabilityFlag::raise(
        &eve_pk,
        &diana_pk,
        jail::FlagCategory::Harassment,
        jail::FlagSeverity::Medium,
        "Targeted harassment of newer members",
    )
    .with_community(community_b.to_string());

    let all_flags = vec![flag_a, flag_b, flag_c];

    // Pattern detection — 2 distinct communities
    let pattern = jail::flag::detect_pattern(&all_flags, &diana_pk, &config);
    assert!(pattern.pattern_established);
    assert_eq!(pattern.distinct_communities, 2);
    assert_eq!(pattern.total_flags, 3);

    // Scan all patterns
    let all_patterns = jail::flag::detect_all_patterns(&all_flags, &config);
    assert_eq!(all_patterns.len(), 1); // only Diana has a cross-community pattern

    // Graduated response
    let mut response = jail::GraduatedResponse::begin(
        &diana_pk,
        "Cross-community pattern of disruptive behavior",
        &alice_pk,
    );
    assert_eq!(response.current_level, jail::ResponseLevel::Education);

    // Escalate through levels
    response
        .escalate("No improvement after educational outreach", &alice_pk)
        .unwrap();
    assert_eq!(response.current_level, jail::ResponseLevel::PublicCensure);

    response
        .escalate("Continued behavior despite public censure", &bob_pk)
        .unwrap();
    assert_eq!(
        response.current_level,
        jail::ResponseLevel::EconomicDisengagement
    );

    // Every level is reversible
    assert!(jail::ResponseLevel::Education.is_reversible());
    assert!(jail::ResponseLevel::PublicCensure.is_reversible());
    assert!(jail::ResponseLevel::EconomicDisengagement.is_reversible());
    assert!(jail::ResponseLevel::CoordinatedNonCooperation.is_reversible());
    assert!(jail::ResponseLevel::ProtectiveExclusion.is_reversible());

    // Accused rights always preserved
    let rights = jail::AccusedRights::always();
    assert!(rights.validate());

    // Diana files an appeal
    let appeal = jail::Appeal::file(
        all_flags[0].id,
        &diana_pk,
        jail::AppealGround::ProceduralError,
        "I was not given adequate time to respond to the educational outreach",
    );
    assert_eq!(appeal.status, jail::AppealStatus::Filed);
    assert_eq!(appeal.grounds, jail::AppealGround::ProceduralError);

    // Admission check for Diana trying to join a new community
    let community_a_members = vec![alice_pk.clone(), bob_pk.clone()];
    let diana_admission = jail::check_admission(
        &graph,
        &diana_pk,
        &community_a.to_string(),
        &community_a_members,
        &all_flags,
        &config,
    );
    // Diana has flags from community members — should be flagged for review
    assert!(
        matches!(
            diana_admission.action,
            jail::AdmissionAction::Deny | jail::AdmissionAction::FlagForReview
        ),
        "Diana should be denied or flagged for review, got: {:?}",
        diana_admission.action
    );
}

// ==========================================================================
// Test 4: Governance Pipeline with Decision Processes
// ==========================================================================
//
// Tests multiple voting algorithms on the same proposal structure.

#[test]
fn governance_decision_processes() {
    let alice_pk = "alice_pub";
    let bob_pk = "bob_pub";
    let charlie_pk = "charlie_pub";
    let dave_pk = "dave_pub";
    let eve_pk = "eve_pub";

    let community_id = uuid::Uuid::new_v4();

    // --- Direct Vote ---
    let mut proposal = kingdom::Proposal::new(
        alice_pk,
        kingdom::DecidingBody::Community(community_id),
        "Allocate garden budget",
        "500 Cool for seeds and tools",
    )
    .with_quorum(kingdom::QuorumRequirement::majority());

    proposal.open_voting(Utc::now() + chrono::Duration::days(7)).unwrap();
    proposal
        .add_vote(kingdom::Vote::new(alice_pk, proposal.id, kingdom::VotePosition::Support))
        .unwrap();
    proposal
        .add_vote(kingdom::Vote::new(bob_pk, proposal.id, kingdom::VotePosition::Support))
        .unwrap();
    proposal
        .add_vote(kingdom::Vote::new(charlie_pk, proposal.id, kingdom::VotePosition::Oppose))
        .unwrap();

    let tally = proposal.tally(5);
    let process = kingdom::DirectVoteProcess;
    use kingdom::DecisionProcess;
    let result = process.is_resolved(&tally, &proposal.quorum);
    // 3/5 participation = 0.6 > 0.5, 2/3 support = 0.667 > 0.5
    assert_eq!(result, Some(kingdom::ProposalResult::Passed));

    // --- Supermajority ---
    let mut proposal2 = kingdom::Proposal::new(
        alice_pk,
        kingdom::DecidingBody::Community(community_id),
        "Amend the charter",
        "Update dissolution terms",
    )
    .with_type(kingdom::ProposalType::Amendment)
    .with_quorum(kingdom::QuorumRequirement::supermajority());

    proposal2
        .open_voting(Utc::now() + chrono::Duration::days(14))
        .unwrap();
    proposal2
        .add_vote(kingdom::Vote::new(alice_pk, proposal2.id, kingdom::VotePosition::Support))
        .unwrap();
    proposal2
        .add_vote(kingdom::Vote::new(bob_pk, proposal2.id, kingdom::VotePosition::Support))
        .unwrap();
    proposal2
        .add_vote(kingdom::Vote::new(charlie_pk, proposal2.id, kingdom::VotePosition::Oppose))
        .unwrap();
    proposal2
        .add_vote(kingdom::Vote::new(dave_pk, proposal2.id, kingdom::VotePosition::Support))
        .unwrap();
    proposal2
        .add_vote(kingdom::Vote::new(eve_pk, proposal2.id, kingdom::VotePosition::Support))
        .unwrap();

    let tally2 = proposal2.tally(5);
    let supermajority = kingdom::SuperMajorityProcess::two_thirds();
    let result2 = supermajority.is_resolved(&tally2, &proposal2.quorum);
    // 5/5 participation, 4/5 support = 0.8 > 0.667
    assert_eq!(result2, Some(kingdom::ProposalResult::Passed));

    // --- Consensus with Block ---
    let mut proposal3 = kingdom::Proposal::new(
        alice_pk,
        kingdom::DecidingBody::Community(community_id),
        "Change meeting time",
        "Move weekly meeting to Wednesday",
    )
    .with_decision_process("consensus")
    .with_quorum(kingdom::QuorumRequirement::unanimous());

    proposal3
        .open_voting(Utc::now() + chrono::Duration::days(3))
        .unwrap();
    proposal3
        .add_vote(kingdom::Vote::new(alice_pk, proposal3.id, kingdom::VotePosition::Support))
        .unwrap();
    proposal3
        .add_vote(kingdom::Vote::new(bob_pk, proposal3.id, kingdom::VotePosition::Block))
        .unwrap();

    // Block should prevent consensus
    let tally3 = proposal3.tally(5);
    let consensus = kingdom::ConsensusProcess;
    let result3 = consensus.is_resolved(&tally3, &proposal3.quorum);
    // Block prevents consensus — should fail or not resolve
    assert_ne!(result3, Some(kingdom::ProposalResult::Passed));
}

// ==========================================================================
// Test 5: Constitutional Review Guards All Domains
// ==========================================================================
//
// Polity reviews actions from governance, economics, and safety domains.

#[test]
fn constitutional_review_across_domains() {
    let rights = polity::RightsRegistry::default();
    let protections = polity::ProtectionsRegistry::default();
    let reviewer = polity::ConstitutionalReviewer::new(&rights, &protections);

    // Clean governance action — passes
    let governance_action = polity::ActionDescription {
        description: "Community votes on shared resource allocation".into(),
        actor: "garden_cooperative".into(),
        violates: vec![],
    };
    assert!(reviewer.review(&governance_action).result.is_permitted());

    // Clean economic action — passes
    let economic_action = polity::ActionDescription {
        description: "UBI distribution to verified members".into(),
        actor: "treasury".into(),
        violates: vec![],
    };
    assert!(reviewer.review(&economic_action).result.is_permitted());

    // Surveillance — breach
    let surveillance = polity::ActionDescription {
        description: "Track user activity without consent".into(),
        actor: "platform_admin".into(),
        violates: vec![polity::ProhibitionType::Surveillance],
    };
    let review = reviewer.review(&surveillance);
    assert!(review.result.is_breach());
    assert!(reviewer.is_absolutely_prohibited(&surveillance));

    // Exploitation — breach
    let exploitation = polity::ActionDescription {
        description: "Extract user data for private profit".into(),
        actor: "data_broker".into(),
        violates: vec![polity::ProhibitionType::Exploitation],
    };
    assert!(reviewer.review(&exploitation).result.is_breach());

    // Discrimination — breach
    let discrimination = polity::ActionDescription {
        description: "Deny service based on identity".into(),
        actor: "service_provider".into(),
        violates: vec![polity::ProhibitionType::Discrimination],
    };
    assert!(reviewer.review(&discrimination).result.is_breach());

    // Multiple violations at once
    let authoritarian = polity::ActionDescription {
        description: "Forced labor with surveillance".into(),
        actor: "authoritarian_actor".into(),
        violates: vec![
            polity::ProhibitionType::Domination,
            polity::ProhibitionType::Surveillance,
            polity::ProhibitionType::Exploitation,
            polity::ProhibitionType::Cruelty,
        ],
    };
    let review = reviewer.review(&authoritarian);
    assert!(review.result.is_breach());
    assert_eq!(review.result.violations().len(), 4);

    // Breach converts to formal record
    let breach = reviewer.to_breach(&review).unwrap();
    assert!(!breach.violated_prohibitions.is_empty());
}

// ==========================================================================
// Test 6: Consent & Enactment Lifecycle
// ==========================================================================
//
// Full lifecycle of Covenant enactment (voluntary entry, suspension,
// reactivation, withdrawal) and consent (grant, active check, revoke).

#[test]
fn consent_and_enactment_lifecycle() {
    let alice = CrownKeypair::generate();
    let bob = CrownKeypair::generate();
    let alice_pk = alice.public_key_hex();
    let bob_pk = bob.public_key_hex();

    // --- Enactment Lifecycle ---
    let mut enactment = polity::Enactment::new(
        &alice_pk,
        polity::EnactorType::Person,
        polity::DEFAULT_OATH,
    )
    .with_witness(polity::Witness::new(&bob_pk));

    assert!(enactment.is_active());
    assert_eq!(enactment.enactor_type, polity::EnactorType::Person);

    // Suspend during investigation
    enactment.suspend().unwrap();
    assert!(!enactment.is_active());
    assert_eq!(enactment.status, polity::EnactmentStatus::Suspended);

    // Can't suspend twice
    assert!(enactment.suspend().is_err());

    // Reactivate after investigation cleared
    enactment.reactivate().unwrap();
    assert!(enactment.is_active());

    // Voluntary withdrawal — always available
    enactment.withdraw().unwrap();
    assert!(!enactment.is_active());
    assert_eq!(enactment.status, polity::EnactmentStatus::Withdrawn);

    // Can't withdraw twice
    assert!(enactment.withdraw().is_err());

    // --- Community Enactment ---
    let community_id = uuid::Uuid::new_v4();
    let community_enactment = polity::Enactment::new(
        community_id.to_string(),
        polity::EnactorType::Community,
        "We, the Garden Cooperative, enact this Covenant in collective affirmation.",
    )
    .with_witness(polity::Witness::new(&alice_pk).with_name("Alice"))
    .with_witness(polity::Witness::new(&bob_pk).with_name("Bob"));

    assert!(community_enactment.is_active());
    assert_eq!(community_enactment.witnesses.len(), 2);

    // --- Consent Lifecycle ---
    let mut consent = polity::ConsentRecord::new(
        &alice_pk,
        "garden_cooperative",
        polity::ConsentScope::DataSharing {
            data_type: "community_planning".into(),
            purpose: "Shared resource allocation".into(),
        },
    )
    .with_condition("Only for community planning purposes")
    .with_condition("Revocable at any time");

    assert!(consent.is_active());
    assert!(!consent.is_revoked());
    assert!(!consent.is_expired());
    assert_eq!(consent.conditions.len(), 2);

    // Revoke — always succeeds
    consent.revoke("I no longer wish to share this data").unwrap();
    assert!(!consent.is_active());
    assert!(consent.is_revoked());

    // --- Consent with Expiry ---
    let expiring_consent = polity::ConsentRecord::new(
        &bob_pk,
        "analytics_module",
        polity::ConsentScope::GovernanceDecision {
            proposal_id: uuid::Uuid::new_v4().to_string(),
        },
    )
    .with_expiry(Utc::now() + chrono::Duration::days(30));

    assert!(expiring_consent.is_active()); // not yet expired

    // Serialization round-trip
    let json = serde_json::to_string(&expiring_consent).unwrap();
    let restored: polity::ConsentRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.grantor, bob_pk);
    assert!(restored.expires_at.is_some());
}

// ==========================================================================
// Test 7: Full Stack — Phase 3 + Phase 4 Together
// ==========================================================================
//
// Identity → Community → Economics → Content → Relay broadcast.
// Proves all phases compose into one coherent stack.

#[tokio::test]
async fn full_stack_with_relay() {
    use futures_util::{SinkExt, StreamExt};
    use globe::*;
    use sha2::{Digest, Sha256};
    use tokio_tungstenite::tungstenite::Message;

    // --- Phase 1/2/3: Identity, Encryption, Content ---
    let alice = CrownKeypair::generate();
    let bob = CrownKeypair::generate();
    let alice_pk = alice.public_key_hex();
    let bob_pk = bob.public_key_hex();

    // --- Phase 4: Community + Economics ---
    let mut community = kingdom::Community::new("Relay Test Coop", kingdom::CommunityBasis::Digital);
    community.add_founder(&alice_pk);
    community.add_founder(&bob_pk);
    community.activate().unwrap();

    // Mint Cool for the community
    let policy = fortune::FortunePolicy::testing();
    let mut treasury = fortune::Treasury::new(policy.clone());
    treasury.update_metrics(fortune::NetworkMetrics {
        active_users: 2,
        total_ideas: 0,
        total_collectives: 1,
    });
    let mut ledger = fortune::Ledger::new();
    treasury
        .mint(100, &alice_pk, fortune::MintReason::Initial)
        .unwrap();
    ledger.credit(&alice_pk, 100, fortune::TransactionReason::Initial, None);

    // --- Phase 4: Proposal passes, creates an .idea ---
    let mut proposal = kingdom::Proposal::new(
        &alice_pk,
        kingdom::DecidingBody::Community(community.id),
        "Publish community manifesto",
        "Share our founding principles with the network",
    );
    proposal
        .open_voting(Utc::now() + chrono::Duration::days(7))
        .unwrap();
    proposal
        .add_vote(kingdom::Vote::new(&alice_pk, proposal.id, kingdom::VotePosition::Support))
        .unwrap();
    proposal
        .add_vote(kingdom::Vote::new(&bob_pk, proposal.id, kingdom::VotePosition::Support))
        .unwrap();

    let tally = proposal.tally(2);
    use kingdom::DecisionProcess;
    let process = kingdom::DirectVoteProcess;
    let result = process
        .is_resolved(&tally, &proposal.quorum)
        .unwrap();
    assert_eq!(result, kingdom::ProposalResult::Passed);

    // --- Phase 1: Create .idea for the manifesto ---
    let manifesto_text = "We are the Garden Cooperative. We grow together.";
    let digit = Digit::new(
        "document.manifesto".into(),
        Value::from(manifesto_text),
        alice.crown_id().to_string(),
    )
    .unwrap();

    // --- Phase 2/3: Babel encode with shared ECDH key ---
    let shared_secret = alice.shared_secret(bob.public_key_data()).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(shared_secret);
    hasher.update(b"omnidea-babel-shared-v1");
    let babel_seed = hasher.finalize().to_vec();

    let babel_alice = Babel::new(&babel_seed);
    let babel_bob = Babel::new(&{
        let bob_secret = bob.shared_secret(alice.public_key_data()).unwrap();
        let mut h = Sha256::new();
        h.update(bob_secret);
        h.update(b"omnidea-babel-shared-v1");
        h.finalize().to_vec()
    });

    let encoded = babel_alice.encode(manifesto_text);
    assert_ne!(encoded, manifesto_text);

    // --- Phase 3: Sign as ORP event ---
    let unsigned = UnsignedEvent::new(9001, &encoded)
        .with_tag("lang", &["en"])
        .with_tag("p", &[&bob_pk])
        .with_tag("digit-id", &[&digit.id().to_string()])
        .with_tag("digit-type", &["document.manifesto"])
        .with_tag("community", &[&community.id.to_string()])
        .with_application_tag("omnidea");

    let event = EventBuilder::sign(&unsigned, &alice).unwrap();
    assert!(event.validate().is_ok());

    // --- Phase 3: Relay broadcast ---
    let (_server, addr) = RelayServer::start_on_available_port().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let url = format!("ws://{addr}");

    // Alice publishes
    {
        let (ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        let (mut write, mut read) = ws.split();
        let msg = ClientMessage::Event(event.clone());
        write
            .send(Message::Text(msg.to_json().unwrap().into()))
            .await
            .unwrap();
        let response = read.next().await.unwrap().unwrap();
        match RelayMessage::from_json(response.to_text().unwrap()).unwrap() {
            RelayMessage::Ok { success, .. } => assert!(success),
            other => panic!("expected OK, got: {other:?}"),
        }
    }

    // Bob receives
    let received_event = {
        let (ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        let (mut write, mut read) = ws.split();
        let sub = ClientMessage::Req {
            subscription_id: "bob-sub".into(),
            filters: vec![OmniFilter {
                authors: Some(vec![alice_pk.clone()]),
                ..Default::default()
            }],
        };
        write
            .send(Message::Text(sub.to_json().unwrap().into()))
            .await
            .unwrap();
        let msg = read.next().await.unwrap().unwrap();
        match RelayMessage::from_json(msg.to_text().unwrap()).unwrap() {
            RelayMessage::Event { event, .. } => event,
            other => panic!("expected EVENT, got: {other:?}"),
        }
    };

    // Bob verifies and decodes
    assert!(EventBuilder::verify(&received_event).unwrap());
    assert_eq!(received_event.author, alice_pk);
    assert!(received_event.has_tag("community", &community.id.to_string()));

    let decoded = babel_bob.decode(&received_event.content);
    assert_eq!(decoded, manifesto_text);

    // --- Phase 4: Constitutional review of the publication ---
    let rights = polity::RightsRegistry::default();
    let protections = polity::ProtectionsRegistry::default();
    let reviewer = polity::ConstitutionalReviewer::new(&rights, &protections);
    let action = polity::ActionDescription {
        description: "Publish community manifesto to network".into(),
        actor: alice_pk,
        violates: vec![],
    };
    assert!(reviewer.review(&action).result.is_permitted());
}

// ==========================================================================
// Test 8: Bulwark Trust Layers + Jail Trust Graph Interplay
// ==========================================================================
//
// Bulwark manages trust layers and bond depths. Jail manages the
// verification graph. They're decoupled but complementary.

#[test]
fn trust_layers_and_graph_interplay() {
    let alice_pk = "alice";
    let bob_pk = "bob";
    let charlie_pk = "charlie";

    // Bulwark: trust layer management
    let alice_bob_bond = bulwark::VisibleBond::new(
        alice_pk,
        bob_pk,
        bulwark::BondDepth::Friend,
    );
    assert_eq!(alice_bob_bond.effective_depth(), bulwark::BondDepth::Friend);
    assert!(alice_bob_bond.is_mutual()); // both see Friend depth

    // Asymmetric bond — modify one side's view
    let mut alice_charlie_bond = bulwark::VisibleBond::new(
        alice_pk,
        charlie_pk,
        bulwark::BondDepth::Acquaintance,
    );
    // Alice sees Charlie as Acquaintance, but update Charlie's view to Casual
    alice_charlie_bond.depth_from_b = bulwark::BondDepth::Casual;
    // Effective = min(Acquaintance, Casual) = Casual
    assert_eq!(
        alice_charlie_bond.effective_depth(),
        bulwark::BondDepth::Casual
    );
    assert!(!alice_charlie_bond.is_mutual());

    // Jail: trust graph with verification edges
    let mut graph = jail::TrustGraph::new();
    graph
        .add_edge(jail::VerificationEdge::new(
            alice_pk,
            bob_pk,
            "mutual_vouch", // maps to Bulwark's vouch verification method
            jail::VerificationSentiment::Positive,
            0.9,
        ))
        .unwrap();
    graph
        .add_edge(jail::VerificationEdge::new(
            alice_pk,
            charlie_pk,
            "digital_verification",
            jail::VerificationSentiment::Neutral,
            0.5,
        ))
        .unwrap();

    // Alice has verified both — different confidence levels
    let alice_verified = graph.verified_by(alice_pk);
    assert_eq!(alice_verified.len(), 2);

    // Bob has higher-confidence verification than Charlie
    let bob_edges = graph.edges_to(bob_pk);
    let charlie_edges = graph.edges_to(charlie_pk);
    assert!(bob_edges[0].confidence > charlie_edges[0].confidence);

    // Reputation tracks trust through endorsements
    let mut alice_rep = bulwark::Reputation::new(alice_pk);
    alice_rep.factors.endorsements = 150; // strong endorsement factor
    alice_rep.factors.tenure = 120; // established member
    alice_rep.recompute_from_factors();
    assert!(alice_rep.score > 500); // above neutral

    let charlie_rep = bulwark::Reputation::new(charlie_pk);
    assert_eq!(charlie_rep.score, 500); // neutral start
    assert_eq!(charlie_rep.standing(), bulwark::Standing::Neutral);
}

// ==========================================================================
// Test 9: Re-Verification State Machine
// ==========================================================================
//
// When someone's identity needs re-verification: pending → collecting
// attestations → completed.

#[test]
fn reverification_lifecycle() {
    let alice_pk = "alice";
    let bob_pk = "bob";

    let config = jail::JailConfig::testing(); // 1 attestation required

    // Start re-verification session
    let mut session = jail::ReVerificationSession::start(
        alice_pk,
        jail::ReVerificationReason::VoluntaryUpdate,
        &config,
    );

    assert_eq!(session.state, jail::ReVerificationState::Pending);

    // Bob attests — first attestation transitions to Collecting
    let attestation = jail::ReVerificationAttestation::new(bob_pk);
    session.add_attestation(attestation).unwrap();
    assert_eq!(session.state, jail::ReVerificationState::Collecting);

    // Complete the session (testing config requires only 1 attestation)
    session.complete().unwrap();
    assert_eq!(session.state, jail::ReVerificationState::Completed);
    assert!(session.completed_at.is_some());

    // Serialization
    let json = serde_json::to_string(&session).unwrap();
    let restored: jail::ReVerificationSession = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.state, jail::ReVerificationState::Completed);
    assert_eq!(restored.pubkey, alice_pk);
}

// ==========================================================================
// Test 10: Immutable Foundations Cannot Be Violated
// ==========================================================================
//
// The Core and Commons principles are hardcoded in Polity. No amendment,
// no configuration, no API can touch them.

#[test]
fn immutable_foundations() {
    // Three axioms — compile-time constants
    assert!(polity::ImmutableFoundation::AXIOMS.iter().any(|a| a.contains("Dignity")));
    assert!(polity::ImmutableFoundation::AXIOMS.iter().any(|a| a.contains("Sovereignty")));
    assert!(polity::ImmutableFoundation::AXIOMS.iter().any(|a| a.contains("Consent")));

    // Immutable right categories
    assert!(!polity::ImmutableFoundation::IMMUTABLE_RIGHTS.is_empty());

    // Absolute prohibitions
    assert!(!polity::ImmutableFoundation::ABSOLUTE_PROHIBITIONS.is_empty());

    // Violation detection (heuristic)
    assert!(polity::ImmutableFoundation::would_violate("permit domination"));
    assert!(polity::ImmutableFoundation::would_violate("allow surveillance"));
    assert!(polity::ImmutableFoundation::would_violate("suspend dignity"));
    assert!(!polity::ImmutableFoundation::would_violate("community garden"));
    assert!(!polity::ImmutableFoundation::would_violate("cooperative economics"));

    // Check specific rights are immutable
    assert!(polity::ImmutableFoundation::is_right_immutable(&polity::RightCategory::Dignity));
    assert!(polity::ImmutableFoundation::is_right_immutable(&polity::RightCategory::Safety));

    // Check specific prohibitions are absolute
    assert!(polity::ImmutableFoundation::is_prohibition_absolute(&polity::ProhibitionType::Cruelty));
    assert!(polity::ImmutableFoundation::is_prohibition_absolute(&polity::ProhibitionType::Surveillance));
}
