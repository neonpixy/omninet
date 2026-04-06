# Bulwark — Safety & Protection

Care, not surveillance. Trust layers, health monitoring, reputation, Kids Sphere insulation, child safety protocol. The defensive wall that protects without watching.

## Architecture

```
Bulwark (safety & protection)
    ├── Trust (progressive access)
    │   ├── TrustLayer: Connected → Verified → Vouched → Shielded
    │   ├── BondDepth: Casual → Acquaintance → Friend → Best → Life
    │   ├── VisibleBond: asymmetric (effective = min(a,b)), BondChange history
    │   ├── TrustChain: provenance (EntryMethod, VouchRecord, SponsorRecord)
    │   └── LayerTransition: requirements per layer, check_transition(), blockers
    ├── Verification (pluggable identity methods)
    │   ├── VerificationMethod trait + 6 built-in impls
    │   ├── ProximityBond: BLE/NFC/QR, 60s nonce, -55dBm
    │   ├── VouchRules: stricter for minors (3 vouches, parent, diversity)
    │   └── Sponsorship: Life bond required, 2yr term, max 3 active, 90-day cooldown
    ├── Health (structural signals, never content)
    │   ├── UserHealthPulse: 4-factor, 0-12 → 5 statuses
    │   ├── CollectiveHealthPulse: 5-factor weighted, cult detection
    │   └── HealthSeverity: Normal/Concerning/Warning/Critical/Emergency
    ├── Reputation (earned, never purchased)
    │   ├── Reputation: 5-factor, 0-1000 score, Standing
    │   ├── FraudDetection trait: pluggable fraud algorithms
    │   ├── SuspiciousPattern: 11 pattern types, RiskScore
    │   └── Consequence: graduated, time-limited
    ├── Kids Sphere (child protection)
    │   ├── KidsSphereConfig: AllowedContactType, AllowedContentType
    │   ├── ParentOversight: scales with age (kid=full, teen=message privacy)
    │   ├── FamilyBond: REQUIRES proximity proof
    │   └── SiloedMinor: Siloed → ParentLinked → Authorized
    ├── Permissions (fine-grained access control)
    │   ├── Permission: Action + ResourceScope (hierarchical dot-separated + wildcard)
    │   ├── Role: named set of permissions with trust layer + collective role prerequisites
    │   ├── ConditionalPermission: conditions (field + ConditionOp)
    │   ├── Delegation: time-limited, revocable subset
    │   └── PermissionChecker: can(actor, action, resource) → PermissionDecision
    ├── Child Safety (bypasses governance)
    │   └── ChildSafetyProtocol: 5 immutable steps, SilentRestriction
    ├── Consent
    │   └── ConsentRecord, ConsentScope (6), ConsentValidator
    ├── Age Tiers
    │   └── AgeTier: Kid(<13) / Teen(13-17) / YoungAdult(18-24) / Adult(25+)
    ├── Network Origin
    │   └── NetworkOrigin, BootstrapPhase (100-member threshold)
    ├── Behavioral Drift Detection (R2D)
    │   ├── DriftComputer: stateless pure-function engine
    │   ├── BehavioralBaseline: per-person, per-community reference (180-day default)
    │   ├── BehavioralDrift: drift_score (0.0–1.0), 6 weighted factors
    │   ├── DriftAlert: Normal/Alert/Critical, top contributing factors
    │   └── DriftConfig: per-community thresholds (0.6 alert, 0.8 critical)
    ├── Power Concentration Index (R2E)
    │   ├── PowerConcentrationComputer: pure-function, 5-factor computation
    │   ├── PowerFactors: Gini, Herfindahl, decision influence, info centrality, exit barrier
    │   ├── PowerConcentrationIndex: 0.0–1.0 score + alerts
    │   ├── PowerConcentrationConfig: thresholds (cannot disable — 0.9 cap)
    │   └── Mathematical primitives: gini_coefficient(), herfindahl_index()
    └── KidsSphere Exclusion (R2B)
        ├── KidsSphereApproval: collective parental approval + proximity proof
        ├── KidsSphereExclusion: immutable exclusion from all KidsSphere communities
        ├── KidsSphereExclusionRegistry: Arc<RwLock<HashSet>>, thread-safe
        ├── KidsSphereApprovalPolicy: min 3 parents, annual renewal, proximity required
        └── KidsSphereAccessResult: mandatory 5-step check order
```

## Key Types

### Trust (`trust/`)
- **TrustLayer** — 4 levels: Connected → Verified → Vouched → Shielded. Each unlocks cumulative `LayerCapabilities` (messaging, checkmark, UBI eligibility, vouch capability, Kids Sphere access, sponsorship). Sequential progression via `next()`.
- **BondDepth** — 5 levels: Casual → Acquaintance → Friend → Best → Life. Each maps to `BondCapabilities` (messaging, collective invite, vouch adult/young adult/minor, sponsor family, emergency contact). Vouch requires Friend+, minor vouch Best+, sponsor Life only.
- **VisibleBond** — Asymmetric: effective depth = `min(a_thinks_of_b, b_thinks_of_a)`. `BondChange` tracks history.
- **TrustChain** — Provenance of how someone entered. `EntryMethod`, `VouchRecord`, `SponsorRecord`.
- **LayerTransition** — Per-layer requirements. `check_transition()` returns blockers. `LayerTransitionRequest` / `LayerTransitionStatus`.

### Verification (`verification/`)
- **VerificationMethod** trait — `method_id()`, `trust_weight()`, `description()`. Send + Sync.
- 6 built-in implementations with weights: ProximityVerification (1.0), MutualVouchVerification (0.7), CommunitySponsorVerification (0.8), DigitalAttestationVerification (0.5), ReputationBasedVerification (0.6), TimeBasedVerification (0.4).
- **VerificationEvidence** — method_id + subject_pubkey + evidence_data HashMap. Builder pattern.
- **VerificationResult** — verified bool + method_id + trust_weight + notes.
- **ProximityBond** / **ProximityProof** — BLE/NFC/QR proximity verification. `ProximityMethod` enum.
- **VouchRules** — Configurable vouch requirements. Stricter for minors (3 vouches, parent required, diversity check). `VouchEligibility`, `VouchDiversityCheck`, `MutualVouch`.
- **Sponsorship** — Life bond required, 2-year term, max 3 active, 90-day cooldown. `SponsorEligibility`, `SponsorshipStatus`.

### Health (`health/`)
- **UserHealthPulse** — 4 factors (activity, connection, content sentiment, communication balance). Score 0–12 mapped to 5 `UserHealthStatus` levels.
- **CollectiveHealthPulse** — 5 weighted factors including cross-membership (heaviest at +5 out of 19 max). Cult detection via `CrossMembershipLevel`, `PowerDistribution`, `EngagementDistribution`.
- **HealthSeverity** — Normal / Concerning / Warning / Critical / Emergency.

### Reputation (`reputation/`)
- **Reputation** — 5-factor, 0–1000 score. `Standing` enum. `ReputationEvent` / `ReputationEventType` for mutations.
- **FraudDetection** trait — `detect(pubkey) -> Vec<SuspiciousPattern>`, `detector_id()`. Pluggable.
- **SuspiciousPattern** — 11 types: CircularTrading, SelfTrading, PriceManipulation, DumpAndRun, MultipleIdentities, InactiveHarvesting, VouchingRing, NonFulfillment, SerialDisputer, IsolationPattern, ReputationGaming.
- **RiskScore** — Computed from `Vec<FraudIndicator>`. `RiskRecommendation`: Safe/Caution/AvoidLargeTransactions/DoNotTrade.
- **Consequence** — `ConsequenceType` (graduated), `ConsequenceDuration` (time-limited).

### Kids Sphere (`kids_sphere/`)
- **KidsSphereConfig** — `AllowedContactType`, `AllowedContentType`.
- **ParentOversight** — Scales with age: kid = full oversight, teen = message privacy.
- **FamilyBond** — REQUIRES `ProximityProof`. Non-negotiable.
- **SiloedMinor** — State machine: Siloed → ParentLinked → Authorized. `MinorDetectionReason`, `MinorRegistrationState`, `ParentLink`.
- **KidConnectionRequest** / **KidConnectionRules** / **KidConnectionStatus**.

### Permissions (`permissions/`)
- **Action** — App-defined string. 10 common constants: view, create, edit, delete, upload, download, approve, publish, invite, manage.
- **ResourceScope** — Hierarchical dot-separated paths. `"brand"` covers `"brand.logo"`. Wildcard `"*"` covers everything. `covers()` checks hierarchy.
- **Permission** — Action + ResourceScope. `covers(action, resource)` checks match including hierarchy.
- **Role** — Named set of permissions with trust layer and collective role prerequisites. `actor_qualifies()` checks both. `RoleRegistry` for register/lookup/unregister.
- **ConditionalPermission** — Permissions with `Condition` tree (Equals/NotEquals/Contains/Exists/NotExists/All/Any). Evaluated against `PermissionContext` (HashMap of field→value).
- **Delegation** — Grant subset of permissions to another actor. Time-limited, revocable. `DelegationStore` manages active delegations.
- **PermissionChecker** — Central `can(actor, action, resource)` and `check(actor, action, resource, context)`. Check order: (1) app-defined roles, (2) conditional permissions, (3) delegations. Returns `PermissionDecision` with source tracking (`PermissionSource::Role`/`Conditional`/`Delegation`). `effective_permissions()` lists all grants.
- **ActorContext** — pubkey + TrustLayer + CollectiveRole + assigned_roles. Builder pattern with `with_role()`.
- Local enforcement only — the app checks, not the protocol.

### Child Safety (`child_safety.rs`)
- **ChildSafetyProtocol** — 5 immutable steps (bypasses governance): encrypted flag, resources shown, silent restriction, reporter protected, always escalate.
- **ChildSafetyFlag**, **ChildSafetyConcern**, **ChildSafetyStatus**, **RealWorldResources**, **SilentRestriction**.

### Other
- **ConsentRecord** / **ConsentScope** (6 scopes) / **ConsentValidator** — Voluntary, informed, continuous, revocable consent.
- **AgeTier** — Kid (<13) / Teen (13–17) / YoungAdult (18–24) / Adult (25+). `AgeTierConfig` for configurable boundaries.
- **NetworkOrigin** / **BootstrapPhase** — 100-member threshold for bootstrap capabilities.
- **BulwarkError** — Categorized with `is_security_concern()` (ChildSafetyViolation, FraudDetected, MinorNotAuthorized, FamilyBondRequiresProximity) and `is_retryable()`. Implements `From<serde_json::Error>`.

### Behavioral Drift Detection (`behavioral_drift.rs`) — R2D
Detects long-con attacks by tracking structural changes in behavior over time. Shape-only detection -- never reads content, never profiles people, never restricts. Alerts inform; communities evaluate.

- **Activity** — Raw input: action_type string (e.g. "governance.vote", "content.create", "social.message") + timestamp. Never carries content.
- **BaselineMetrics** — Structural fingerprint of behavior: action_frequency (per week), action_type_distribution (HashMap, sums to ~1.0), governance_participation_rate (0.0--1.0), content_creation_rate (per week), social_engagement_rate (per week), role_changes (count). `zero()` constructor.
- **BehavioralBaseline** — Per-person, per-community baseline: pubkey, community_id, baseline_period_secs, metrics, established_at. Default baseline period is 180 days.
- **DriftFactor** — Single dimension of change: metric_name, baseline_value, current_value, deviation (0.0--1.0).
- **BehavioralDrift** — Comparison result: pubkey, community_id, baseline, current metrics, drift_score (0.0--1.0 weighted average), drift_factors, computed_at. `exceeds_alert(config)`, `exceeds_critical(config)`.
- **DriftAlert** — Surfaced for community awareness: drift, level (Normal/Alert/Critical), top_factors sorted by deviation.
- **DriftConfig** — Per-community, stored in Charter: alert_threshold (default 0.6), critical_threshold (default 0.8), computation_interval_days (default 30), baseline_period_days (default 180). `validate()` enforces sensible ranges.
- **DriftComputer** — Pure-function, stateless engine. `compute(baseline, activities, period_weeks, proposals_available, votes_cast, role_changes)` -> BehavioralDrift. `compute_metrics(activities, period_weeks, ...)` -> BaselineMetrics. Weighted factors: governance (0.25, heaviest -- primary long-con signal), action distribution (0.20), action frequency (0.15), content creation (0.15), social engagement (0.15), role changes (0.10). Uses `rate_deviation()` (relative change) and `distribution_deviation()` (Jensen-Shannon style) for normalization.

Integration: behavioral drift + power concentration = strong combined signal. High drift AND high power concentration together elevates to critical.

### Power Concentration Index (`power_index.rs`) — R2E
Structural metric that detects unhealthy power concentration in communities. Five-factor score, 0.0 (distributed) to 1.0 (concentrated). From Constellation Art. 8: "No person or office shall hold permanent power."

- **PowerConcentrationIndex** — community_id, score (0.0--1.0), factors (PowerFactors), alerts (Vec<PowerAlert>), computed_at.
- **PowerFactors** — Five 0.0--1.0 dimensions: role_concentration (Gini coefficient of role distribution), proposal_dominance (Herfindahl-Hirschman Index of proposal authorship), decision_influence (max share of deciding votes), information_centrality (HHI of content authorship), exit_barrier (average member exit cost). `combined_score(factors_enabled)` returns mean of selected factors. `as_named_vec()` returns all as (name, value) pairs.
- **PowerAlert** — factor name, value, threshold, description, recommendation. Generated when individual factors or overall score cross thresholds.
- **PowerConcentrationConfig** — Per-community, stored in Charter: alert_threshold (default 0.6), critical_threshold (default 0.8), factors_enabled (empty = all), computation_interval_hours (default 168 = weekly). `new()` enforces minimum alert threshold (0.9 cap -- communities can tune but never disable). `effective_alert_threshold()`, `validate()`.
- **PowerConcentrationComputer** — Pure-function computer. `compute(community_id, member_roles, proposal_authors, deciding_votes, content_authorship, exit_barriers, config)` -> PowerConcentrationIndex. No side effects.
- **Mathematical primitives** — `gini_coefficient(values)` (relative mean absolute difference), `herfindahl_index(counts)` (normalized HHI, 0.0--1.0), `decision_influence(votes)` (max deciding-vote share), `information_centrality(authorship)` (HHI of authorship), `average_exit_barrier(barriers)` (clamped mean).
- **Constants** — `MINIMUM_ALERT_THRESHOLD = 0.9` (absolute cap -- cannot disable detector), `DEFAULT_ALERT_THRESHOLD = 0.6`, `DEFAULT_CRITICAL_THRESHOLD = 0.8`, `DEFAULT_COMPUTATION_INTERVAL_HOURS = 168`.

Key design: minimum alert_threshold is 0.9. Communities can tune lower, but cannot set higher than 0.9 -- the detector cannot be turned off. Critical threshold triggers mandatory governance review. Alerts go to ALL members, not just leaders.

### KidsSphere Exclusion (`kids_exclusion.rs`) — R2B
Collective parental approval for KidsSphere access + immutable exclusion from all KidsSphere communities. Children's Dignity overrides adult community access.

**Collective Parental Approval (entry):**
- **ApprovalConfidence** — Comfortable, Cautious, Reluctant. `Reluctant` triggers community-wide notification.
- **ParentalApproval** — parent_pubkey, child_pubkeys (per-child granularity), confidence, signed_at. `is_reluctant()`.
- **KidsSphereApproval** — Collective approval: id, candidate_pubkey, community_id, approvals (Vec<ParentalApproval>), collective_meeting (ProximityProof), approved_at, expires_at (annual), revoked_at. `new()` validates proximity proof + minimum approvals. `is_valid()` checks both expired and revoked. `revoke()` sets revoked_at (one parent revoking = immediate suspension). `has_reluctant_approval()`. Community-specific -- approval in one community does NOT transfer.
- **KidsSphereApprovalPolicy** — min_approvals (default 3, minimum 2), renewal_interval_days (default 365, max 730), single_revocation_triggers_review (always true -- circuit breaker), proximity_required (always true -- remote approval defeats purpose). `validate()` enforces Covenant constraints.

**Immutable Exclusion (removal):**
- **KidsSphereExclusion** — id, excluded_pubkey, basis, adjudication_record, scope (always AllKidsSphere), established_at, review_schedule, is_permanent. `new()` validates: permanent only if unanimous adjudication AND AdjudicatedPredation basis. CrossCommunityMinorSafety requires 3+ independent communities.
- **KidsExclusionBasis** — AdjudicatedPredation (Jail Dispute finding), CrossCommunityMinorSafety (3+ independent communities), ChildSafetyProtocolTrigger (existing 5-step protocol confirmed).
- **KidsAdjudicationRecord** — dispute_ids, flag_ids, communities_involved, adjudicators, finding_summary, unanimous. Builder pattern.
- **KidsSphereExclusionScope** — Single variant: AllKidsSphere. Non-negotiable.
- **KidsReviewSchedule** — review_interval_days (default 730 -- biennial), next_review, reviews_completed, review_type (always StarCourt). For process integrity, not reinstatement.
- **KidsSphereAccessResult** — Allowed, ExclusionDenied, NoApproval, Expired, Revoked. Mandatory check order: exclusion registry -> approval check -> expiry -> revocation -> allowed.
- **KidsSphereExclusionRegistry** — Arc<RwLock<HashSet<String>>>. `is_excluded(pubkey)`, `exclude(pubkey)`, `check_access(pubkey, community_approvals)`. Thread-safe.

Identity rebirth defense: even with a new Crown identity, a predator needs both (1) entry into the Founding Verification Tree (new lineage detectable by graph analysis) and (2) multiple parents to physically meet and approve -- a face-to-face gauntlet.

## Key Design Decisions

1. **Physical proximity NOT required for adult trust.** VerificationMethod is a trait with 6 built-in implementations. Proximity is highest weight (1.0) but vouch (0.7), sponsor (0.8), digital (0.5), reputation (0.6), and time-based (0.4) all work.
2. **Physical proximity IS required for Kids Sphere family bonds.** FamilyBond requires ProximityProof. Non-negotiable.
3. **4 age tiers, not 3.** Kid (<13) / Teen (13–17) / YoungAdult (18–24) / Adult (25+). Configurable within ranges.
4. **Child safety protocol bypasses governance.** 5 immutable steps: encrypted flag, resources shown, silent restriction, reporter protected, always escalate.
5. **Cross-membership is the heaviest health weight.** Isolated communities score +5 (out of 19 max) — the single strongest cult detection signal.
6. **Permissions compose ON TOP of trust layers.** Roles require trust layer + collective role prerequisites. The checker evaluates roles → conditionals → delegations in order.

## Dependencies

```toml
x = { path = "../X" }       # Value type
crown = { path = "../Crown" } # Identity/signatures
serde, serde_json, thiserror, uuid, chrono, log
```

**Zero async.** Bulwark is pure data structures and logic.

## Covenant Sources

| Module | Covenant Source |
|--------|---------------|
| trust/ | Constellation Art. 8 (mandate-based, no permanent power) |
| verification/ | Proximity is ONE method, not THE method |
| health/ | Structural signals only, never content |
| reputation/ | Consortium Art. 6 (continuous accountability) |
| kids_sphere/ | Family bonds require proximity |
| permissions/ | Local enforcement, composes on trust layers |
| child_safety.rs | Bypasses governance, real-world escalation |
| consent.rs | Conjunction Art. 4 §1 (voluntary, informed, continuous, revocable) |
| age_tier.rs | Gradual tiers, configurable |
| behavioral_drift.rs | R2D — shape-only detection, never content; Dignity, Sovereignty, Consent |
| power_index.rs | Constellation Art. 8 (no permanent power); R2E — minimum threshold cannot be disabled |
| kids_exclusion.rs | R2B — children's Dignity overrides adult access; collective physical meeting; Star Court review |

## Covenant Alignment

**Dignity** — every person has inherent worth; trust layers protect, not exclude.
**Sovereignty** — you choose your verification path; no single method is mandated.
**Consent** — all monitoring is opt-in; parent oversight scales with age.
