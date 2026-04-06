# Jail — Accountability Primitives

Web of trust and restorative accountability. Trust graph, pattern detection, graduated response, community admission, duty to warn. The court of justice — where accountability meets dignity.

## Architecture

```
Jail (accountability primitives)
    ├── Trust Graph (the social fabric)
    │   ├── TrustGraph: dual adjacency lists (outgoing + incoming), O(V+E) BFS
    │   ├── VerificationEdge: verifier→verified, method string ID, sentiment, confidence
    │   ├── BFS Queries: verifications and flags visible within N degrees
    │   ├── VerificationPattern: Healthy/Limited/Isolated/Suspicious/Flagged
    │   └── TrustRecommendation: Safe/Caution/PublicOnly/GroupOnly/Avoid
    ├── Flags (concerns surfaced)
    │   ├── AccountabilityFlag: 7 categories, 4 severities, builder pattern
    │   ├── FlagPattern: 2+ distinct communities = established pattern
    │   ├── FlagReview: community-driven review with 4 outcomes, 5 actions
    │   ├── DutyToWarn: inter-community notification when patterns emerge
    │   └── WeaponizationDetection: serial filing, coordinated campaigns, repetitive grounds
    ├── Response (graduated, restorative)
    │   ├── GraduatedResponse: Education → Censure → Disengagement → Non-Cooperation → Exclusion
    │   ├── Remedy: 4 types (repair/restore/prevent/reintegrate)
    │   └── ProtectiveExclusion: mandatory ReviewSchedule, RestorationPath, no permanent castes
    ├── Admission (community gates)
    │   └── check_admission(): count verifications + check flags → 5-action decision tree
    ├── Re-Verification (identity updates)
    │   └── State machine: Pending → Collecting → Completed/Failed/Expired
    ├── Appeal (challenging decisions)
    │   └── Appeal: 6 grounds, 4 decisions (upheld/reversed/modified/remanded)
    ├── Rights (the floor — always on)
    │   ├── AccusedRights: 6 rights, always true, validate() for safety
    │   └── ReporterProtection: identity protected, retaliation monitored, legal immunity
    └── Sustained Exclusion (R2A — above GraduatedResponse)
        ├── SustainedExclusion: long-duration, annual reviews, multi-community consensus
        ├── SustainedExclusionBasis: RepeatedProtectiveExclusion / CrossCommunityPattern / AdjudicatedHarm
        ├── SustainedExclusionRequest: builder with 5-point validation
        ├── ExclusionScope: SpecificCommunities / AllKidsSphere / AllAffirming (never Crown/Vault/Fortune)
        ├── SustainedReviewSchedule: annual, Standard or CovenantCourt
        └── Anti-weaponization: 3+ independent communities, 90-day evidence span, founder independence
```

## Key Types

### Trust Graph (`trust_graph/`)
- **TrustGraph** — Directed graph with dual adjacency lists (outgoing + incoming edge IDs). Edges stored by UUID. `add_edge()`, `remove_edge()`, `edges_from()`, `edges_to()`, `bfs_traverse()`. Self-verification rejected.
- **VerificationEdge** — verifier_pubkey → verified_pubkey with `method: String` (e.g., "proximity", "mutual_vouch"), `sentiment: VerificationSentiment` (Positive/Neutral/Cautious), and confidence (0.0–1.0).
- **NetworkIntelligence** — Full safety profile for a target as seen from a querier's BFS position. Combines verification query, flag query, pattern analysis, and recommendation into one struct.
- **VerificationPattern** — Derived from verification and flag counts: Healthy / Limited / Isolated / Suspicious / Flagged.
- **TrustRecommendation** — Safe / Caution / PublicOnly / GroupOnly / Avoid.

### Flags (`flag/`)
- **AccountabilityFlag** — 7 `FlagCategory` variants (PredatoryBehavior, IdentityFraud, Harassment, Inappropriate, SuspiciousActivity, MinorSafety, Other). 4 `FlagSeverity` levels (Low/Medium/High/Critical, ordered). Builder: `raise()` → `.with_community()` → `.with_context()` → `.with_signature()`. Status: Pending/UnderReview/Upheld/Dismissed/Appealed.
- **FlagContext** — Evidence hashes, witness count, related event/community IDs.
- **FlagPattern** — Cross-community pattern detection. `detect_pattern()` checks if flags come from 2+ distinct communities (configurable threshold).
- **FlagReview** — Community-driven review with ReviewOutcome (Upheld/Dismissed/Modified/Deferred) and CommunityAction (5 options).
- **DutyToWarn** / **WarningRecord** — Inter-community notification when patterns emerge.
- **Anti-weaponization** — `detect_serial_filing()`, `detect_coordinated_campaign()`, `detect_repetitive_grounds()`. 4 `AbusePattern` types. 4 `AbuseConsequence` levels (Warning → RequireCosigner → SuspendFiling → PublicIdentification). `recommend_consequence()` escalates by count and confidence. `check_rate_limit()` enforces per-day caps.

### Response (`response/`)
- **GraduatedResponse** — 5 `ResponseLevel` tiers: Education → PublicCensure → EconomicDisengagement → CoordinatedNonCooperation → ProtectiveExclusion. Always starts at Education via `begin()`. `escalate()` / `de_escalate()` / `resolve()`. Every level returns `true` from `is_reversible()`. History tracked via `ResponseRecord` entries.
- **Remedy** — 4 `RemedyType` variants: Repair/Restore/Prevent/Reintegrate.
- **ProtectiveExclusion** — Mandatory `ReviewSchedule`, `RestorationPath`, no permanent castes. `exclusion_review_days` validation rejects zero ("no permanent castes" in error message).

### Admission (`admission.rs`)
- `check_admission()` — Takes a trust graph, prospect pubkey, community members, and flags. Decision tree: (1) HIGH/CRITICAL flags from members → Deny, (2) Any flags → FlagForReview, (3) Insufficient verifications → RequireMoreVerifications, (4) No direct verifications → RequireInterview, (5) Otherwise → Admit. Returns `AdmissionRecommendation` with action, counts, reasons.

### Re-Verification (`reverification/`)
- **ReVerificationSession** — State machine: Pending → Collecting → Completed/Failed/Expired. Configurable attestation requirements and expiry.
- **ReVerificationAttestation** / **AttestationRequirements** — Who can attest, how many needed.

### Appeal (`appeal.rs`)
- **Appeal** — 6 `AppealGround` variants, 4 `AppealDecision` outcomes (Upheld/Reversed/Modified/Remanded). Status tracking via `AppealStatus`.

### Rights (`rights.rs`)
- **AccusedRights** — Single constructor: `always()`. All 6 rights always `true`. `validate()` checks all are on (returns `false` only if something went catastrophically wrong). Default impl delegates to `always()`.
- **ReporterProtection** — `for_flag()` creates with all protections on: `identity_protected`, `retaliation_monitored`, `legal_immunity_for_good_faith`.

### Config (`config.rs`)
- **JailConfig** — 10 tunables. `validate()` enforces Covenant constraints (e.g., `pattern_threshold_communities >= 2`, `exclusion_review_days > 0`). 3 presets: `default()`, `testing()` (permissive), `strict()` (shorter windows).

### Error (`error.rs`)
- **JailError** — Categorized with `is_security_concern()` (RightsViolation, ExclusionReviewOverdue) and `is_retryable()` (FlagRateLimited, InsufficientAttestations, InsufficientVerifications). Implements `From<serde_json::Error>`.

### Sustained Exclusion (`sustained_exclusion.rs`) — R2A
Above GraduatedResponse, for repeat offenders. Long-duration exclusion with annual (not quarterly) reviews, requiring multi-community consensus. Even sustained exclusion is NOT permanent. Crown identity, Vault data, Fortune balance, and the ability to create one's own community are NEVER affected.

- **SustainedExclusion** — id, excluded_pubkey, basis, evidence_chain, communities_affirming (minimum 3), established_at, scope, review_schedule, reviews, lifted_at, accused_rights (always on). `is_active()`, `is_review_overdue()`, `record_review(review)` (auto-lifts if finding is Lift, auto-modifies scope if ModifyScope). `validate_scope()`, `validate_rights()`, `inalienable_rights()` lists Crown/Vault/Fortune/community-creation.

- **SustainedExclusionBasis** — 3 grounds, each with strict evidence requirements:
  - `RepeatedProtectiveExclusion { cycle_count }` — at least 2 full cycles of exclusion->review->reinstatement->reoffense.
  - `CrossCommunityPattern { community_count }` — accountability flags from 3+ independent communities with no shared founding members.
  - `AdjudicatedHarm { severity, dispute_id }` — Jail Dispute finding of Grave or Existential severity.
- **AdjudicatedSeverity** — Grave (widespread harm) | Existential (threatens Core/Commons). Local equivalent of Polity's BreachSeverity (Jail does not depend on Polity).

- **ExclusionEvidence** — source_type (Flag/Dispute/ExclusionRecord/CrossCommunityReport), source_id, community_id, summary, evidence_hashes, submitted_at.
- **CommunityAffirmation** — community_id, decision_id (Kingdom Proposal), affirmed_at. Minimum 3 required.
- **ExclusionScope** — SpecificCommunities(Vec) | AllKidsSphere | AllAffirming. `respects_sovereign_rights()` always true -- the type system prevents representing scopes that touch Crown/Vault/Fortune/community-creation.

- **SustainedReviewSchedule** — review_interval_days (default 365), next_review, reviews_completed, review_type. `annual()` default. `is_overdue()`, `advance()`.
- **SustainedReviewType** — Standard (panel from affirming communities) | CovenantCourt (Star Court, for KidsSphere and cross-community patterns).
- **SustainedReview** — id, review_type, panel, finding, reasoning, reviewed_at.
- **SustainedReviewFinding** — Maintain | ModifyScope(ExclusionScope) | Lift.

- **SustainedExclusionRequest** — Builder for constructing and validating requests: excluded_pubkey, basis, evidence, affirmations, scope, community_founders (for independence checks). `validate()` checks: basis set + basis-specific requirements, minimum 3 affirmations, evidence spans 90+ days, community independence (no shared founders -- anti-weaponization), scope set + respects sovereign rights. `build()` validates then constructs.

Anti-weaponization safeguards: minimum 3 independent communities (no shared founding members), evidence must span 90+ days (prevents rapid coordinated action), community independence validated against founder sets. Review type escalates to CovenantCourt for KidsSphere scope or cross-community patterns.

## Key Design Decisions

1. **Trust graph uses string method IDs.** Edges reference verification methods by `method: String` (e.g., "proximity", "mutual_vouch"), keeping Jail completely decoupled from Bulwark's concrete VerificationMethod implementations.

2. **BFS from querier's perspective.** All queries are subjective — what you see depends on your position in the graph. This is by design. There is no god's-eye view.

3. **Patterns require 2+ distinct communities.** One community flagging someone is a local concern. Two or more is a pattern. This is the threshold for duty-to-warn notifications.

4. **Anti-weaponization is built in.** Rate limiting, serial filing detection, coordinated campaign detection, repetitive grounds detection — all with graduated consequences (warning → cosigner → suspend → public ID).

5. **All response levels are reversible.** Even ProtectiveExclusion. Mandatory ReviewSchedule ensures no exclusion lasts forever without re-evaluation. RestorationPath provides concrete conditions for return.

6. **AccusedRights has one constructor: `always()`.** There is no API to create rights with any right disabled. If `validate()` returns false, it's a bug.

7. **ReporterProtection is automatic.** Created with `for_flag()`, all protections on by default. Identity protected from accused (reviewers can see it).

## Dependencies

```toml
x = { path = "../X" }       # Value type
crown = { path = "../Crown" } # Identity/signatures
serde, serde_json, thiserror, uuid, chrono, log
```

**Zero async.** Jail is pure data structures and logic — no network, no storage, no platform deps.

## What Does NOT Live Here

- **Constitutional enforcement** (rights, duties, protections) → Polity (P)
- **Governance structures** (communities, proposals, voting) → Kingdom (K)
- **Economic systems** (currency, UBI, exchange) → Fortune (F)
- **Safety mechanisms** (trust layers, health monitoring, verification methods) → Bulwark (B)
- **Inter-module communication** → Equipment (Pact)

Jail defines HOW accountability works. Other crates define WHAT the rules are.

## Covenant Sources

| Module | Covenant Source |
|--------|---------------|
| trust_graph/ | BFS traversal, pattern analysis, recommendations |
| flag/types.rs | Flag categories, severity, review lifecycle |
| flag/pattern.rs | New — cross-community pattern detection (2+ communities) |
| flag/warning.rs | Constellation Art. 7 §4 (coordinated community response) |
| flag/weaponization.rs | Constellation Art. 5 §6-12 (anti-weaponization, graduated consequences) |
| response/graduated.rs | Constellation Art. 7 §3 (graduated response protocol) |
| response/remedy.rs | Constellation Art. 7 §10 (restoration, not punishment) |
| response/exclusion.rs | Constellation Art. 7 §12 (no permanent castes) |
| admission.rs | Community admission algorithm |
| reverification/ | Re-verification state machine |
| appeal.rs | Constellation Art. 7 §9 (appeal and review), Art. 5 §10 (fast-track dismissal) |
| rights.rs | Constellation Art. 5 §2-5 (accused rights), Coexistence Art. 6 §4 (whistleblower protection) |
| sustained_exclusion.rs | Constellation Art. 7 §12 (no permanent castes); R2A — anti-weaponization (3+ independent communities, 90-day evidence span), CovenantCourt review |

## Covenant Alignment

**Dignity** — the accused always keep their rights; response is proportional; no permanent castes.
**Sovereignty** — communities decide their own accountability parameters via JailConfig.
**Consent** — flags require evidence; weaponization is detected and blocked; reporters are protected.
