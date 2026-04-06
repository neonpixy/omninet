# Polity -- The Constitutional Guard

The Covenant made executable. Polity maintains the rights, duties, and protections of the Covenant as queryable, enforceable data structures. It performs constitutional review on actions, detects breaches, manages the amendment process, tracks enactments, and validates consent.

Polity doesn't govern -- it guards. It is the immune system that keeps everything else honest.

## Architecture

```
Polity (constitutional guard)
    |-- Three Registries
    |   |-- RightsRegistry: 12 Covenant rights (immutable), extensible with custom rights
    |   |-- DutiesRegistry: 8 Covenant duties (immutable), 3 binding levels
    |   +-- ProtectionsRegistry: 8 prohibitions (all absolute), violation checking
    |-- ImmutableFoundation
    |   |-- 10 immutable right categories (compile-time constants)
    |   |-- 7 absolute prohibition types (compile-time constants)
    |   |-- 3 axioms (Dignity, Sovereignty, Consent)
    |   +-- would_violate() -- heuristic violation detection
    |-- ConstitutionalReviewer
    |   |-- review() -- checks action against all registries
    |   |-- is_absolutely_prohibited() -- quick prohibition check
    |   +-- to_breach() -- converts review failure to formal breach
    |-- BreachRegistry
    |   |-- record/update lifecycle (Detected -> Investigating -> Confirmed -> Remediating -> Resolved)
    |   |-- is_foundational() -- detects violations of immutable foundations
    |   +-- queries by actor, severity, active status
    |-- Amendment
    |   |-- 4 triggers (Contradiction, PersistentBreach, MaterialTransformation, PublicInvocation)
    |   |-- lifecycle: Proposed -> Deliberating -> Ratifying -> Enacted (or Rejected/Null)
    |   |-- threshold enforcement (0.60 to 0.75 depending on trigger)
    |   +-- foundation guard -- amendments contradicting Core/Commons are blocked at creation
    |-- Enactment
    |   |-- voluntary, public, witnessed entry into the Covenant
    |   |-- lifecycle: Active -> Suspended -> Active (or Withdrawn)
    |   |-- re-enactment after withdrawal
    |   +-- default oath from Covenant Convergence Art. 4
    |-- Consent
    |   |-- ConsentRecord: grantor, recipient, scope, conditions, expiry
    |   |-- ConsentScope: CommunityMembership, GovernanceDecision, EconomicTransaction, DataSharing, Delegation, General
    |   |-- ConsentValidator: checks active consent exists for a given scope
    |   |-- ConsentValidation: Valid / Missing / Revoked / Expired
    |   +-- always revocable -- coerced consent is void
    |-- ConstitutionalLayers (R1A)
    |   |-- Layer 1: Axioms (compile-time constants, immutable)
    |   |-- Layer 1b: Reconstitution (90% + Star Court + 2yr + axiom alignment)
    |   |-- Layer 2: Constitutional Clauses (60-75% per-part thresholds)
    |   +-- Layer 3: Interpretive Precedent (voluntary, adoptable, supersedable)
    |-- CovenantCode (R1E)
    |   |-- 10 Parts encoded as type constraints (~70% mechanical, ~30% human judgment)
    |   |-- CovenantValidator: validate_action() -> Permitted / Breach / RequiresHumanJudgment
    |   +-- Types that omit unlawful variants (ResourceRelation::Stewardship only, etc.)
    +-- Weaponization (R1C)
        |-- 4 constraints: ConsentVetoLimit, SovereigntyInteropFloor, DignitySpecificHarm, RightsNotShields
        |-- InvocationCheck: validates rights invocations against anti-weaponization constraints
        +-- WeaponizationReason: structured rejection with specific constraint violated
```

### Source Layout

```
Polity/src/
  lib.rs                  -- module declarations + re-exports
  error.rs                -- PolityError enum (Clone + PartialEq)
  rights.rs               -- Right, RightCategory (11), RightScope (5), RightsRegistry
  duties.rs               -- Duty, DutyCategory (8), BindingLevel (3), DutyScope (3), DutiesRegistry
  protections.rs          -- Protection, ProhibitionType (8), ProtectionsRegistry, ActionDescription
  immutable.rs            -- ImmutableFoundation (hardcoded Core + Commons)
  breach.rs               -- Breach, BreachSeverity, BreachStatus, ViolationType, BreachRegistry
  review.rs               -- ConstitutionalReviewer, ConstitutionalReview, ReviewResult, ReviewViolation, ConsentRequirement
  amendment.rs            -- Amendment, AmendmentTrigger (4), AmendmentStatus, ProposedChange
  enactment.rs            -- Enactment, EnactorType, EnactmentStatus, Witness, EnactmentRegistry, DEFAULT_OATH
  consent.rs              -- ConsentRecord, ConsentScope (6), ConsentValidator, ConsentValidation (4), ConsentRegistry
  constitutional_layers.rs -- (R1A) Three-Layer Covenant: reconstitution, clauses, precedent
  covenant_code.rs        -- (R1E) Covenant Encoding: ~70% of 10 Parts as type constraints + CovenantValidator
  weaponization.rs        -- (R1C) Anti-weaponization: 4 constraints on bad-faith rights invocations
```

### Key Types

- **Right** -- id, category (11 types from Core Art. 2/3/6 + Conjunction Art. 7), scope, is_immutable, source
- **RightCategory** -- Dignity, Thought, Expression, LegalStanding, Safety, Privacy, Refusal, Earth, Community, Union, Labor
- **RightScope** -- AllPersons, AllCommunities, Earth, FutureGenerations, AllBeings
- **Duty** -- id, category (8 types from Core Art. 4/7), binding_level, applies_to (DutyScope), is_immutable, source
- **DutyCategory** -- UpholdDignity, Remember, Steward, RefuseAndReconstitute, CommunityFidelity, MutualAid, EcologicalStewardship, PublicChallenge
- **BindingLevel** -- Aspirational < Obligatory < Absolute (derives Ord)
- **Protection** -- id, prohibition_type, is_absolute, is_immutable, source. All Covenant protections default `is_immutable: true`.
- **ProhibitionType** -- Domination, Discrimination, Surveillance, Exploitation, Cruelty, Ecocide, IndustrialCruelty, SystemicBreach
- **ActionDescription** -- description, actor, violates (Vec\<ProhibitionType\>). Checked against ProtectionsRegistry.
- **ImmutableFoundation** -- Compile-time constants. Cannot be loaded, edited, or modified. `IMMUTABLE_RIGHTS` (10 categories), `ABSOLUTE_PROHIBITIONS` (7 types), `AXIOMS` (3). Note: Union is excluded from immutable rights (Conjunction, not Core). SystemicBreach is excluded from absolute prohibitions (describes conditions, not a prohibition).
- **ConstitutionalReview** -- action description -> ReviewResult (Permitted / Breach / NeedsConsent)
- **ReviewViolation** -- optional right_category, optional prohibition_type, description, severity
- **Breach** -- violation type, severity (Minor/Significant/Grave/Existential), affected rights/prohibitions, foundational flag
- **Amendment** -- threshold-based (not periodic), 4 triggers, foundation guard on creation
- **Enactment** -- person/community/consortium/cooperative entry, witnesses, lifecycle
- **ConsentRecord** -- grantor, recipient, scope, conditions, expiry, revocation. Builder pattern (with_expiry, with_condition).
- **ConsentScope** -- CommunityMembership, GovernanceDecision, EconomicTransaction, DataSharing, Delegation, General. Each variant carries context fields.
- **ConsentValidation** -- Valid (with consent_id), Missing (with reason), Revoked (with timestamp), Expired

### constitutional_layers.rs -- Three-Layer Covenant (R1A)

Implements the three-layer constitutional structure from MPv6. The Covenant is not flat -- it has a hierarchy of mutability.

**Layer 1: Axioms** -- Dignity, Sovereignty, Consent. Compile-time constants (reuses `ImmutableFoundation::AXIOMS`). Truly immutable.

**Layer 1b: Core & Commons (Reconstitution)** -- Parts 01-02. Reconstitutable through an extraordinary process requiring 90% community approval, Star Court unanimity, 2-year deliberation, and demonstrated axiom strengthening.

- **ReconstitutionProposal** -- lifecycle: Proposed -> Deliberation -> CovenantCourtReview -> CommunityVote -> Ratified/Rejected. Carries `AxiomAlignment` (must demonstrate how the change serves all three axioms).
- **ReconstitutionTrigger** -- 5 triggers: InternalContradiction, SustainedBreach, InterpretiveAmbiguity, FundamentalTransformation, PublicInvocation.
- **ReconstitutionThreshold** -- hardcoded constants: 90% communities, Star Court unanimous, 730 days minimum deliberation. `is_met()` validates a `RatificationRecord`.
- **ReconstitutionGuard** -- validates proposed text against axioms using both heuristic (`ImmutableFoundation::would_violate`) and full constitutional review (`ConstitutionalReviewer::review`). Rejects anything that weakens an axiom.

**Layer 2: Constitutional Clauses** -- Parts 03-09. Amendable through tiered thresholds (60-75% depending on part, 6-9 month deliberation).

- **CovenantPart** -- enum with all 10 parts (Preamble through Compact), each carrying a display name.
- **ConstitutionalClause** -- a clause within a part, with article reference, text, amendment history.
- **ClauseRegistry** -- `RwLock<HashMap>` of clauses, keyed by part. `amend()` validates proposed text against axioms before allowing modification. `reconstitute()` validates through the higher bar.
- **AmendmentThreshold** -- per-part thresholds: 60% for Parts 03/07, 65% for Parts 04/05/06, 70% for Parts 08/09.
- **ClauseAmendment** -- records the change: old text, new text, ratification record, axiom alignment.

**Layer 3: Interpretive Precedent** -- Living, community-generated interpretations. Voluntary, adoptable, supersedable.

- **CovenantPrecedent** -- an interpretation record with principles cited, community adoptions, and optional superseding precedent ID.
- **PrecedentRegistry** -- stores and queries precedents. Supports `adopt()` (community adopts), `supersede()` (new interpretation replaces old).
- **PrecedentSearch** -- query helper: by part, by principle reference, by community adoption.
- **PrincipleReference** -- links a precedent to specific Covenant articles.

### covenant_code.rs -- Covenant Encoding (R1E)

Encodes ~70% of the Covenant's mechanical rules from all 10 Parts as type constraints and validation functions. The remaining ~30% that requires human judgment returns `CovenantValidation::RequiresHumanJudgment` for Star Court evaluation.

**Per-Part Type Encodings:**

| Part | Types | What They Enforce |
|------|-------|-------------------|
| 00 Preamble | `COVENANT_AXIOMS`, `PREAMBLE_DECLARATION` | Symbolic constants |
| 01 Core | `NoDiscriminationBasis` (13 bases), `SurveillanceProhibition` (always false), `ExitRight`, `BreachCondition` | Anti-discrimination, anti-surveillance, exit rights, breach detection |
| 02 Commons | `ResourceRelation` (Stewardship only -- no Ownership variant), `RegenerationObligation`, `AccessEquity`, `KnowledgeCommons` (OpenAccess only) | Resource stewardship, regeneration, equitable access, open knowledge |
| 03 Coexistence | `ProtectedCharacteristic` (14), `AccommodationRequest`, `HistoricalHarmRecord` (immutable), `WhistleblowerProtection` | Anti-discrimination, accommodation, historical memory, reporter protection |
| 04 Conjunction | `BeingProtection`, `PersonhoodPresumption`, `UnionConsent`, `LaborProtection`, `WealthCap` | Being rights, consciousness precaution, union consent, UBI/labor, wealth limits |
| 05 Consortium | `CharterAlignment`, `CollectiveOwnership` (no Private variant), `TransparencyMandate`, `SunsetProvision` (10-year max), `WorkerStakeholder` | Enterprise ethics, collective ownership, transparency, sunset clauses, worker voice |
| 06 Constellation | `SubsidiarityPrinciple`, `MandateDelegation`, `EmergencySunset` (30/30/90 day hardcoded limits), `InviolableEmergencyRight` (4), `GraduatedEnforcement` (4 ordered steps) | Governance constraints, delegation, emergency limits, enforcement ladder |
| 07 Convocation | `ConvocationRight`, `LawfulPurpose`, `AccessibilityMandate`, `LivingRecord` | Assembly rights, accessibility, record-keeping |
| 08 Continuum | `DormancyDefault`, `CustodianAccountability`, `PublicOverride` | Meta-governance dormancy, custodian integrity, community override |
| 09 Compact | `CompactAlignment`, `CompactRevocability`, `CompactTransparency` | Binding agreement constraints |

**Unified Validation:**

- **CovenantAction** -- trait implemented by action types, providing `validate() -> Vec<CovenantCheck>` to declare which checks apply.
- **CovenantCheck** -- enum of all specific validatable constraints (e.g., `CheckDiscrimination`, `CheckSurveillance`, `CheckWealth`, etc.).
- **CovenantValidator** -- single entry point: `validate_action(action) -> CovenantValidation`. Runs all checks declared by the action.
- **CovenantValidation** -- Permitted | Breach(Vec\<BreachDetail\>) | RequiresHumanJudgment(String).
- **BreachDetail** -- part, article, violation description, severity. The structured error type for all covenant_code validations.

**Design pattern:** Types with no lawful alternative omit the unlawful variant entirely (e.g., `ResourceRelation` has only `Stewardship`, `CollectiveOwnership` has only `Collective`, `KnowledgeCommons` has only `OpenAccess`). The type system itself prevents the violation.

### weaponization.rs -- Anti-Weaponization Constraints (R1C)

Prevents bad-faith use of Covenant principles. The axioms are powerful tools that can be turned against the people they were meant to protect. This module defines when a principle CANNOT be invoked.

**The Four Constraints (`InvocationConstraint`):**

- **ConsentVetoLimit** -- Consent applies to actions taken UPON you, not actions taken BY your community. One person cannot veto a properly conducted collective decision that doesn't affect individual rights.
- **SovereigntyInteropFloor** -- You can choose not to participate, but you cannot break the protocol for others. Globe relay peering and Equipment message routing are non-negotiable protocol mechanics.
- **DignitySpecificHarm** -- "I find this undignified" without identifying who is harmed and how is not a valid Dignity claim. Prevents Dignity from becoming content censorship.
- **RightsNotShields** -- Rights protect individuals from power, not the powerful from accountability. A leader cannot invoke "rights" to dodge accountability.

**Key Types:**

- **RightInvocation** -- captures the claim: who invokes, which right category, target action, optional `HarmClaim`, `InvocationContext`.
- **HarmClaim** -- affected party (must be specific, not "everyone"), harm description (must be substantive), evidence hashes, severity. Vague parties and trivially vague harm descriptions are detected.
- **InvocationContext** -- community, decision, invoker role. `is_authority_role()` detects leadership roles for `RightsNotShields` checks.
- **InvocationCheck** -- the check function: `check(invocation, is_collective_decision, affects_individual_rights) -> InvocationResult`.
- **InvocationResult** -- Valid(RightCategory) | Rejected(WeaponizationReason) | NeedsHumanReview(String).
- **WeaponizationReason** -- the structured rejection: which constraint was violated, description, the invocation that triggered it.

### Pre-populated Covenant Data

- **12 rights** from Core Art. 2, 3, 6 and Conjunction Art. 7 (all immutable)
- **8 duties** from Core Art. 4 and 7 (all immutable, 6 absolute + 2 obligatory)
- **8 protections** from Core Art. 5 and 3 and Conjunction Art. 3 (all absolute and immutable)
- **3 axioms** (Dignity, Sovereignty, Consent)
- **Default oath** from Convergence Art. 4

## Dependencies

```toml
x = { path = "../X" }       # Value type
crown = { path = "../Crown" } # Identity/signatures
serde, serde_json, thiserror, uuid, chrono, log
```

**Zero async.** Polity is pure data structures and logic -- no network, no storage, no platform deps. PolityError is Clone + PartialEq (no opaque errors).

## What Does NOT Live Here

- **Governance structures** (communities, proposals, voting) -- Kingdom (K)
- **Economic systems** (currency, UBI, exchange) -- Fortune (F)
- **Safety mechanisms** (trust layers, health monitoring) -- Bulwark (B)
- **Accountability** (trust graph, flags, patterns) -- Jail (J)
- **Encryption/storage** -- Sentinal/Vault
- **Inter-module communication** -- Equipment (Pact)

Polity defines WHAT the rules are. Other crates implement HOW they operate.

## Covenant Alignment

Polity IS the Covenant in code. Every type traces to a specific Article and Section.

**Dignity** -- every right, duty, and protection flows from irreducible worth.
**Sovereignty** -- enactment is voluntary, withdrawal always available, amendments require broad consent.
**Consent** -- continuous, informed, revocable. Coerced consent is void. Silence is never consent.
