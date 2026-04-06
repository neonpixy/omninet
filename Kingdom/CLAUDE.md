# Kingdom -- Governance Primitives

Toolkit for self-governance. Communities, charters, proposals, deliberation, pluggable voting, mandate-based delegation, federation, dispute resolution. The governed realm -- where the Covenant becomes lived practice.

## Architecture

```
Kingdom (governance primitives)
    |-- Community Layer
    |   |-- Community: 7 basis types, 6 roles, 5 statuses, lifecycle state machine
    |   |-- Charter: versioned constitution, CovenantAlignment, GovernanceStructure
    |   |-- Membership: 5 join processes, applications with review, formation requirements
    |   +-- Assembly: 7 types (council->ceremony), convocation triggers, records
    |-- Decision Layer
    |   |-- DecisionProcess trait: pluggable voting algorithms
    |   |-- 6 built-in: DirectVote, Consensus, Consent, SuperMajority, RankedChoice, LiquidDemocracy
    |   |-- Vote: 5 positions (Support/Oppose/Abstain/Block/StandAside), weighted
    |   |-- VoteTally: participation + approval rate, quorum check
    |   +-- QuorumRequirement: presets (majority/supermajority/unanimous/one_third)
    |-- Proposal Layer
    |   |-- Proposal: 8 types, lifecycle (Draft->Discussion->Voting->Resolved)
    |   |-- ProposalOutcome: from tally, participation/support rates
    |   +-- DiscussionPost: threaded discussion with reply_to
    |-- Delegation Layer
    |   |-- Mandate: authorized/requires-consultation/prohibited decisions, presets
    |   |-- Delegate: represents community, appointment source, activity log
    |   |-- DelegateRecall: signature threshold -> triggered -> vote -> recalled/retained
    |   +-- LiquidDemocracy: transitive delegation with cycle detection
    |-- Federation Layer
    |   |-- Consortium: member communities with delegates, 4 voting models
    |   |-- SubsidiarityCheck: validates decisions at proper level (community->planetary)
    |   +-- ConsortiumCharter: delegate selection, membership process, exit, dissolution
    |-- Challenge Layer
    |   |-- Challenge: 7 types (accountability, core violation, power concentration...)
    |   |-- ChallengeTarget: proposal, governance structure, delegate, charter
    |   +-- ChallengeResponse: acknowledge/deny/partial/proposes-reform
    |-- Adjudication Layer (restorative, not punitive)
    |   |-- Dispute: 7 types, lifecycle (Filed->Response->Hearing->Resolution)
    |   |-- Adjudicator: qualifications, jurisdiction, track record
    |   |-- Hearing: 5 formats, participants with roles
    |   |-- Resolution: findings with confidence, 8 restorative remedy actions
    |   |-- Appeal: 7 grounds, 4 decisions (affirmed/reversed/modified/remanded)
    |   +-- Compliance: verification lifecycle
    |-- Union Layer
    |   |-- Union: 7 types (marriage->trade union), consent-based formation
    |   |-- UnionFormation: unanimous consent, witnesses, ceremony record
    |   +-- Dissolution: "Where consent ends, the Union ends"
    |-- Star Court Layer (R1B)
    |   |-- CovenantCourt: file/accept/advance/decide cases
    |   |-- CourtCase: 6-status lifecycle (Filed -> PrecedentRecorded)
    |   |-- CourtJurisdiction: 4 types (InterCommunityDispute, CovenantInterpretation, AmendmentReview, ExclusionAppeal)
    |   |-- CourtDecision: reasoning, interpretation, dissents (preserved, never hidden)
    |   +-- AdjudicatorPool: eligibility, diversity minimums, conflict exclusions
    |-- Democratic Infrastructure Layer (R3A-D)
    |   |-- AdvisorDelegation: AI as liquid democracy delegate, override always available
    |   |-- AffectedPartyConsent: minority consent gate on proposals, mediation on block
    |   |-- ExitWithDignity: inalienable exit package, cost transparency, penalty rejection
    |   +-- GovernanceBudget + RoleRotation: anti-fatigue + anti-concentration
    |-- Diplomacy Layer (R4D)
    |   |-- DiplomaticChannel: bilateral/multilateral/treaty/mediation, 5-status lifecycle
    |   |-- Treaty: ratification-based activation, terms with obligation types, suspend/dissolve
    |   +-- Liaison: inter-community representative (Observer/Ambassador/Representative)
    +-- AI Pool Layer (R6B)
        |-- AIPool: community-scoped shared compute for AI inference
        |-- PooledProvider: contributed AI provider with capabilities + capacity
        |-- AIPoolPolicy: access, fair use, priority, reward rate
        +-- PoolRequest/PoolResponse: routed AI inference with priority ordering
```

### Source Layout

```
Kingdom/src/
  lib.rs                  -- module declarations + extensive re-exports
  error.rs                -- KingdomError enum (Clone + PartialEq)
  charter.rs              -- Charter, CovenantAlignment, GovernanceStructure, MembershipRules, DisputeResolutionConfig, DissolutionTerms, CharterSignature, LeadershipSelection, MediatorSelection, AdjudicationFormat, AssetDistribution
  community.rs            -- Community, CommunityBasis (7), CommunityRole (6), CommunityStatus (5), CommunityMember
  membership.rs           -- JoinProcess (5), MembershipApplication, ApplicationStatus, FormationRequirements
  vote.rs                 -- Vote, VotePosition (5), VoteTally, QuorumRequirement, DelegateVoteInfo, ConsultationResult
  decision.rs             -- DecisionProcess trait + 6 implementations (DirectVote, Consensus, Consent, SuperMajority, RankedChoice, LiquidDemocracy), VoteDelegation, DelegationScope, RankedBallot, ProposalResult
  proposal.rs             -- Proposal, ProposalType (8), ProposalStatus, ProposalOutcome, DecidingBody, DiscussionPost
  mandate.rs              -- Mandate, MandateDecision, Delegate, DelegateActivity, DelegateActivityType, DelegateRecall, RecallStatus, RecallSignature, AppointmentSource
  federation.rs           -- Consortium, ConsortiumCharter, ConsortiumGovernance, ConsortiumMember, ConsortiumStatus, GovernanceLevel (4), SubsidiarityCheck, VotingModel
  assembly.rs             -- Assembly, AssemblyType (7), ConvocationTrigger (7), AssemblyRecord, AssemblyStatus, RecordType
  challenge.rs            -- Challenge, ChallengeType (7), ChallengeTarget, ChallengeResponse, ChallengeStatus, ResponsePosition
  union.rs                -- Union, UnionType (7), UnionFormation, UnionCharter, UnionMember, UnionStatus, ConsentSignature, CeremonyRecord, DissolutionRecord
  adjudication/
    mod.rs                -- submodule re-exports
    dispute.rs            -- Dispute, DisputeType (7), DisputeStatus (8), DisputeResponse, DisputeContext, Counterclaim
    adjudicator.rs        -- Adjudicator, AdjudicatorRecord, AdjudicatorAssignment, AdjudicatorRole, AdjudicatorStatus, AdjudicatorAvailability, AdjudicatorJurisdiction, Qualification, QualificationType
    hearing.rs            -- HearingRecord, HearingFormat (5), HearingParticipant, ParticipantRole (6)
    resolution.rs         -- Resolution, Finding, FindingConfidence, OrderedRemedy, RemedyAction (8), DecisionOutcome
    appeal.rs             -- Appeal, AppealGround (7), AppealDecision (4), AppealOutcome, AppealStatus, EvidenceItem, EvidenceType
    compliance.rs         -- ComplianceRecord, ComplianceStatus
  covenant_court.rs       -- (R1B) Star Court: cross-community constitutional interpretation
  advisor_delegation.rs   -- (R3A) Advisor Delegation: AI as liquid democracy delegate
  affected_party.rs       -- (R3B) Affected-Party Consent: minority group consent gates on proposals
  exit.rs                 -- (R3C) Exit with Dignity: inalienable exit package, cost transparency, penalty rejection
  governance_health.rs    -- (R3D) Governance Budget + Role Rotation: anti-fatigue + anti-concentration
  diplomacy.rs            -- (R4D) Inter-Community Diplomacy: channels, treaties, liaisons
  ai_pool.rs              -- (R6B) Community AI Pools: shared compute for AI inference
```

### Key Types

- **Community** -- id, name, basis (7 types), charter, members, status lifecycle
- **CommunityBasis** -- Geographic, Cultural, Professional, Economic, Spiritual, Digital, Hybrid
- **CommunityRole** -- Founder, Elder, Member, Newcomer, Observer, Delegate
- **Charter** -- versioned constitution with CovenantAlignment, governance structure, membership rules, dispute resolution config, dissolution terms, amendment tracking
- **Proposal** -- 8 types, lifecycle, discussion, voting, outcome
- **Vote** -- 5 positions (Support, Oppose, Abstain, Block, StandAside), weighted, delegate info
- **DecisionProcess** -- trait with `decide(&self, tally: &VoteTally) -> ProposalResult`. 6 built-in implementations.
- **Mandate** -- specific scope (authorized/consult/prohibited), term limits, recall
- **Consortium** -- federation of communities, subsidiarity enforcement, 4 governance levels (Community, Regional, Continental, Planetary)
- **Challenge** -- public challenge to governance, 7 types from Constellation Art. 5
- **Dispute** -- full restorative adjudication pipeline with 8-status lifecycle
- **RemedyAction** -- Apology, Restitution, ServiceToAffected, StructuralReform, EducationalProcess, MediatedDialogue, PublicAccountability, CommunityReintegration
- **Union** -- consent-based bonds, 7 types from Conjunction Art. 4

### covenant_court.rs -- Star Court (R1B)

Cross-community constitutional interpretation body. Not a supreme court with enforcement power -- an interpretation body whose decisions become Layer 3 precedent. The Court has moral authority, not coercive power. Dissents are preserved, never hidden.

**Key Types:**

- **CovenantCourt** -- the court itself. Holds an `AdjudicatorPool`, seated `CourtAdjudicator`s, and active `CourtCase`s. Methods: `file_case()`, `accept_case()`, `advance_to_hearing()`, `advance_to_deliberation()`, `render_decision()`, `record_precedent()`.
- **CourtCase** -- lifecycle: Filed -> Accepted -> Hearing -> Deliberation -> Decided -> PrecedentRecorded. Carries petitioner, optional respondent, intervenors/amici curiae, jurisdiction, submissions.
- **CourtJurisdiction** -- 4 types: InterCommunityDispute, CovenantInterpretation, AmendmentReview, ExclusionAppeal.
- **AdjudicatorPool** -- eligible pubkeys with community diversity minimum and conflict-of-interest exclusions. Optional tenure limits.
- **AdjudicatorSelection** -- RandomFromPool, RotatingPanel, NominatedByParties.
- **CourtAdjudicator** -- seated adjudicator with community, cases heard, recusal record. `has_conflict()` checks if adjudicator belongs to a party's community.
- **CourtDecision** -- summary, reasoning, interpretation (the precedent text), unanimous flag, dissents, optional precedent_id. `add_dissent()` automatically marks non-unanimous.
- **CourtDissent** -- adjudicator pubkey, reasoning, alternative interpretation. Dissents are first-class -- they represent legitimate alternative readings.
- **CourtParty** -- pubkey, community, role (Petitioner/Respondent/Intervenor/AmicusCuriae).
- **CourtSubmission** -- written filing with evidence hashes.
- **PartyRole** -- Petitioner, Respondent, Intervenor, AmicusCuriae.

### advisor_delegation.rs -- Advisor Delegation (R3A)

Extends Kingdom's LiquidDemocracy to support Advisor (AI) as a delegate type. Members can delegate voting power to their AI Advisor, which votes according to configured governance values. Override is always one tap away.

**Key Types:**

- **DelegateType** -- Person(String) | Advisor. Distinguishes human-to-human delegation from AI delegation.
- **AdvisorDelegation** -- a member's active delegation: member pubkey, community, activation time, config, override history.
- **AdvisorDelegationConfig** -- opaque to Kingdom (Advisor interprets): values_profile_id, deliberation_window_secs (default 24h), always_direct_categories, reasoning_enabled, extensions HashMap.
- **DelegationOverride** -- record of a member overriding their Advisor's position on a specific proposal. Stores original and override positions.
- **DeliberationWindow** -- tracks the override window for a proposal. `is_open()` checks if members still have time to override. After window closes, Advisor votes are tallied for non-overriding members.
- **GovernanceAIPolicy** -- community charter setting: advisor_delegation_enabled, max_advisor_percentage cap (default 50%), deliberation_window_secs, excluded_proposal_types. Communities control how much AI delegation they allow.
- **DelegationStats** -- community-level stats: total members, advisor/person/direct counts, advisor percentage. `exceeds_cap()` checks policy compliance.
- **AdvisorDelegationRegistry** -- manages all active delegations in a community. `activate()`, `deactivate()`, `record_override()`, `compute_stats()`. Enforces GovernanceAIPolicy (rejects if policy disallows or cap exceeded).

### affected_party.rs -- Affected-Party Consent (R3B)

Proposals that specifically affect a minority group require that group's consent, not just majority approval. A block vote from the affected party prevents passage regardless of majority support.

**Key Types:**

- **AffectedPartyTag** -- identifies a group affected by a proposal: group identifier, affected member pubkeys, impact description.
- **ProposalConstraint** -- enum of constraints attachable to proposals: AffectedPartyConsent(tag), HumanVoteRequired, SuperMajorityRequired(f64), DeliberationMinimum(u64 secs).
- **AffectedPartyVote** -- separate consent vote by an affected group. `cast()` prevents duplicates. Any Block vote sets `consent_given = false`. Blockers are tracked with their reasoning.
- **MediationRecord** -- lifecycle: Pending -> InProgress -> Resolved/Failed. Triggered when an affected party blocks. Proposer revises to address concerns.
- **MediationStatus** -- Pending, InProgress, Resolved, Failed.

**Key Functions:**

- `evaluate_affected_party_constraints()` -- checks all AffectedPartyConsent constraints against group votes. Returns error if any group blocked or hasn't voted.
- `check_deliberation_minimum()` -- validates minimum discussion period before voting opens.

### exit.rs -- Exit with Dignity (R3C)

Any member can leave any community with everything they brought. The right to exit is absolute and inalienable -- it cannot be penalized, fined, or punished.

**Key Types:**

- **ExitPackage** -- everything a departing member takes and leaves behind. Member pubkey, community, timestamp, retained items, transferred items.
- **ExitRetained** -- inalienable possessions: Crown identity (always), Vault data (always), Fortune balance, Yoke history (always), reputation score, personal bonds.
- **ExitTransferred** -- stays with community: collective contributions, governance roles (vacated), delegations received (returned to delegators).
- **ExitCost** -- a natural cost (NOT a penalty) the member would face. `ExitCostType`: None, EconomicLoss, SocialLoss, DataLoss.
- **ExitCostCalculator** -- builder for computing exit costs from locked shares, community-only bonds, non-extractable data. Shows costs BEFORE the member decides. Clean exits produce a single `ExitCostType::None`.
- **VisibleBond** -- a personal bond visible in the exit package.

**Key Function:**

- `reject_exit_penalty_clause()` -- scans charter clause text for punitive exit costs (exit fee, departure fine, exit tax, reputation penalty, etc.). Any match is rejected as a Sovereignty violation. Retention incentives are fine -- penalties are not.

### governance_health.rs -- Governance Budget + Role Rotation (R3D)

Two mechanisms to prevent governance fatigue and power concentration.

**Governance Budget:**

- **GovernanceBudget** -- limits active proposals per community (default 5, configurable). `try_activate()` / `release()` manage slots. When full, new proposals queue.
- **ProposalQueue** -- FIFO queue for proposals waiting for budget slots. `enqueue()`, `dequeue()`, `peek()`, `remove()`.
- **QueuedProposal** -- entry with proposal ID, author, title, queued timestamp.

**Role Rotation:**

- **RoleRotationPolicy** -- term limits and cooling-off periods. Defaults: 3 consecutive terms, 180-day terms, applies to Elder and Steward (NOT Founder during formation), cooling-off = 1 term. From Constellation Art. 8 SS6.
- **RoleTermTracker** -- tracks consecutive terms per member per role. `start_term()` fails if at term limit or cooling off. `end_term()` begins cooling-off after max terms. `reset_after_cooloff()` resets eligibility.

### diplomacy.rs -- Inter-Community Diplomacy (R4D)

Peer-to-peer relationships between independent communities. Complements the existing Consortium (hierarchical federation) with bilateral and multilateral tools.

**Key Types:**

- **DiplomaticChannel** -- communication channel between 2+ communities. Lifecycle: Proposed -> Accepted -> Active -> Suspended -> Closed. Messages can only be sent on Active channels by member communities.
- **ChannelType** -- Bilateral, Multilateral, Treaty, Mediation.
- **DiplomaticMessage** -- author pubkey, community, content, timestamp.
- **Treaty** -- formal agreement between communities. Lifecycle: Drafted -> Ratifying -> Active -> Suspended -> Expired -> Dissolved. `ratify()` transitions on first ratification and activates when all parties have ratified.
- **TreatyTerm** -- a specific obligation within a treaty. `ObligationType`: MutualRecognition, SharedStandard, TradeAgreement, DefenseAlliance, InformationSharing, AdjudicationReciprocity, Custom(String).
- **TreatyRatification** -- links community to the Kingdom Proposal that authorized ratification.
- **Liaison** -- a representative from one community stationed in another. `LiaisonRole`: Observer (read-only), Ambassador (discuss, no vote), Representative (can vote on shared-concern proposals). Term-limited.

**Globe Integration:** Event kind range 11000-11099 for diplomacy (channel, message, treaty, ratification, liaison appointment). `kind::is_diplomacy_kind()` range check.

### ai_pool.rs -- Community AI Pools (R6B)

Communities can pool compute resources for shared AI inference. When a member's local Advisor lacks a capability, it falls back to the community pool.

**Key Types:**

- **AIPool** -- the pool itself. Community-scoped. Holds providers, policy, usage tracking. `route_request()` finds a capable, available provider (checks access, fair use, capability, availability). `calculate_rewards()` distributes Cool to contributors proportional to capacity.
- **PooledProvider** -- a contributed provider: ID, contributor pubkey, capabilities (bitflags), capacity (concurrent requests, tokens/day, available hours).
- **MinimumCapabilities** -- bitflags: TextEditing, DesignSuggestion, AccessibilityCheck, DataAnalysis, Translation, GovernanceReasoning, SearchAssistance. Mirrors Advisor's R6A layout for cross-crate compatibility (Kingdom doesn't depend on Advisor).
- **ProviderCapacity** -- concurrent request limit, daily token limit, UTC availability hours. `always_available()` convenience.
- **AIPoolPolicy** -- access (AllMembers, StewardAndAbove, ApprovedList), fair_use_limit (default 100/day), priority (FIFO, GovernanceFirst, RoundRobin), reward_rate (Cool per 1000 requests).
- **AIPoolUsage** -- period tracking: total requests, per-member counts, per-capability-type counts. `reset()` for new periods.
- **PoolRequest** -- routed to the pool: requester, capability needed, priority (Background < Search < Creation < Governance), context JSON.
- **PoolResponse** -- provider used, result JSON, optional compute cost.
- **AIPoolReward** -- Cool earned by a contributor for requests served.
- **RequestPriority** -- Background(0) < Search(1) < Creation(2) < Governance(3). Governance votes should never wait behind background tasks.

**Fortune Integration:** Contributors earn Cool for providing AI inference. `AIPoolReward::calculate()` computes compensation based on policy reward rate.

### Covenant Sources

| Module | Covenant Source |
|--------|---------------|
| community.rs | Constellation Art. 1 (governance from communities), Art. 2 (recognition) |
| charter.rs | Constellation Art. 2 SS5 (responsibilities), Art. 1 SS4 (diversity of form) |
| decision.rs | Constellation Art. 1 SS4, Art. 8 SS4 (synthesis over domination) |
| mandate.rs | Constellation Art. 8 SS3 (mandate-based delegation, immediate recall) |
| federation.rs | Constellation Art. 8 SS1 (subsidiarity), Consortium Art. 1 (lawful enterprise) |
| assembly.rs | Convocation Art. 1-5 (right to convoke, procedures, purposes) |
| challenge.rs | Constellation Art. 5 (public challenge, anti-weaponization) |
| adjudication/ | Constellation Art. 7 SS3 (graduated response), Art. 5 SS4 (restorative) |
| union.rs | Conjunction Art. 4 (personal unions, consent, dissolution in dignity) |
| covenant_court.rs | MPv6 R1B (Star Court), Constellation Art. 7 (restorative justice) |
| advisor_delegation.rs | MPv6 R3A (Advisor delegation), Constellation Art. 8 SS3 (delegation) |
| affected_party.rs | MPv6 R3B (affected-party consent), Constellation Art. 8 SS4 (dissent rights) |
| exit.rs | MPv6 R3C (exit with dignity), Core Art. 2 Sec. 6 (right to refuse/resist) |
| governance_health.rs | MPv6 R3D (governance budget + rotation), Constellation Art. 8 SS6 (term limits) |
| diplomacy.rs | MPv6 R4D (inter-community diplomacy), Constellation Art. 8 SS1 (subsidiarity) |
| ai_pool.rs | MPv6 R6B (community AI pools), Covenant axiom of Dignity (AI equity) |

## Dependencies

```toml
x = { path = "../X" }       # Value type
crown = { path = "../Crown" } # Identity/signatures
serde, serde_json, thiserror, uuid, chrono, log, bitflags
```

**Zero async.** Kingdom is pure data structures and logic -- no network, no storage, no platform deps. KingdomError is Clone + PartialEq.

## What Does NOT Live Here

- **Constitutional enforcement** (rights, duties, protections) -- Polity (P)
- **Economic systems** (currency, UBI, exchange) -- Fortune (F)
- **Safety mechanisms** (trust layers, health monitoring) -- Bulwark (B)
- **Accountability** (trust graph, flags, patterns) -- Jail (J)
- **Inter-module communication** -- Equipment (Pact)

Kingdom defines HOW communities govern. Other crates define WHAT they govern with.

## Covenant Alignment

**Dignity** -- every governance structure protects the irreducible worth of every person.
**Sovereignty** -- communities are self-governing; federation is voluntary; mandates are recallable.
**Consent** -- governance imposed without active consent has no standing.
