# Advisor — AI Cognition

The wise counselor. Advisor is a continuously thinking mind — not a chatbot. It processes thoughts autonomously, builds connections, and only speaks when it has something worth saying (or when asked). AI companions are first-class citizens of Omninet.

## Architecture: Brain Metaphor + Provider Router

Two architectural lineages — a continuously thinking mind (brain metaphor with thoughts, synapses, expression pressure) and a multi-provider AI pipeline (capability-based routing, security tiers, streaming).

```
Advisor (AI cognition)
    ├── Thought Layer (brain impulses)
    │   ├── Thought: 6 sources, 4 priorities, lifecycle tracking (expressed→viewed→discussed)
    │   ├── Session: Home (inner monologue, singleton) + User (conversations, archivable)
    │   └── ThoughtChunk: streaming generation chunks
    ├── Synapse Layer (cognitive graph)
    │   ├── Synapse: weighted connections (strengthen +0.2, decay -0.05/day, prune at 0.1)
    │   ├── EntityType: Session, Thought, Idea, Memory
    │   ├── RelationshipType: 7 built-in + Custom
    │   └── SynapseQuery: builder pattern with filter()
    ├── Pressure Layer (urge to speak)
    │   ├── ExpressionPressure: 0.0→1.0, threshold-driven expression
    │   ├── PressureEvent: 6 event types with configurable bonuses
    │   └── PressureSnapshot: immutable state for UI/logging
    ├── Engine Layer (LLM abstraction)
    │   ├── CognitiveProvider trait: pluggable backends (describes provider, not generation)
    │   ├── ClaudeProvider: Anthropic API format types, tool-use parsing
    │   ├── LocalProvider: offline-capable toggle, model_loaded state
    │   ├── ProviderCapabilities: bitflags (streaming/tools/context/offline/structured)
    │   ├── ProviderRegistry: lifecycle + preference ordering
    │   └── ProviderRouter: strategy-based selection + SecurityTier enforcement
    ├── Store Layer (persistence)
    │   ├── CognitiveStore: in-memory state (thoughts/sessions/memories/synapses/clipboard)
    │   ├── Memory: embedding vectors, access tracking, keyword + semantic search
    │   ├── GlobalClipboard: capped working memory with priority decay
    │   └── EmbeddingProvider trait + cosine_similarity
    ├── Skill Layer (tool calling)
    │   ├── SkillDefinition: name, description, typed parameters
    │   ├── SkillCall/SkillResult: invocation + outcome (get_string/get_number helpers)
    │   ├── SkillValidationResult: Covenant consent gate (approved/rejected/needs_approval)
    │   ├── SkillRegistry: register, search, sanitized ID support
    │   └── Programs: 41 skills across 7 Throne programs (register_all_skills())
    ├── Cognitive Loop (the brain stem)
    │   ├── CognitiveLoop: sync state machine, tick() → Vec<CognitiveAction>
    │   ├── InnerVoice: parallel monologue (variable rate, energy-driven)
    │   ├── CognitiveAction: RequestGeneration/Express/Store/ModifyState/Emit
    │   ├── StateCommand: bidirectional self-modification (pressure/focus/mode/synapse/clipboard)
    │   └── CognitiveMode: Assistant (reactive) / Autonomous (proactive)
    ├── Bridge Layer (skill → action translation)
    │   ├── ActionBridge trait: translate(SkillCall) → BridgeOutput
    │   ├── BridgeOutput: Action(magic::Action) | DirectResult(SkillResult)
    │   ├── BridgeRegistry: routes skill prefixes to program bridges
    │   └── 7 bridges: Studio, Abacus, Quill, Podium, Courier, Library, Tome
    ├── Sacred Layer (Covenant constraints)
    │   ├── SponsorshipBond: one companion per person
    │   ├── ExpressionConsent: 4 levels (Silent/UrgentOnly/Normal/Autonomous)
    │   └── AuditRecord: signed, traceable AI action log
    ├── Governance Layer (R1D — Liquid Democracy delegation)
    │   ├── GovernanceMode: delegation state + value profile + voting history
    │   ├── ValueProfile: stated preferences + voting patterns + override signals
    │   ├── GovernanceModeConfig: auto-vote, deliberation window, excluded categories
    │   ├── GovernanceAIPolicy: community-level controls on delegation
    │   └── ProposalAnalysis: alignment scoring + dissenting considerations
    ├── Capability Floor (R6A — Minimum AI Capability Guarantee)
    │   ├── MinimumCapabilities: bitflags (7 domains), governance_floor(), full_floor()
    │   ├── CapabilityBenchmark: standardized scoring with per-domain thresholds
    │   ├── CapabilityAssessment: provider evaluation result
    │   └── DeferToHuman: fallback when no provider meets floor
    └── Consent Escalation (R6D — AI Consent Escalation)
        ├── ConsentEscalation: 7 action levels (Suggest→Communicate)
        ├── ConsentProfile: per-user gates (default/permissive/restrictive)
        ├── ConsentGate: per-action auto-approve + approval history
        ├── CommunityConsentPolicy: charter-level overrides
        └── PendingAction: queued for human approval with timeout
```

## Key Types

### Thought (`thought/`)
- **Thought** — `ThoughtSource` (6 variants: Autonomous, User, Reflection, MemoryEcho, Skill, External), `ThoughtPriority` (4 levels). Lifecycle tracking (expressed, viewed, discussed). `with_focus()` builder.
- **Session** — Home (inner monologue, singleton) + User (conversations, archivable). `SessionType`, `SessionSummary`.
- **ThoughtChunk** — Streaming generation chunks: Text/Complete/SkillStarted/SkillCompleted/Error.
- **ExternalThought** — Thought originating from outside the advisor.

### Synapse (`synapse/`)
- **Synapse** — Weighted connection between entities. Strengthen on use (+0.2), decay daily (-0.05), prune at 0.1. `EntityType` (Session/Thought/Idea/Memory). `RelationshipType` (7 built-in + `CustomRelationship`).
- **SynapseQuery** — Builder pattern with `filter()`.

### Pressure (`pressure/`)
- **ExpressionPressure** — 0.0→1.0 accumulator. `increment()`, `apply(event, config)`, `partial_release()`. `should_express(threshold)`, `is_urgent(threshold)`, `seconds_since_release()`.
- **PressureEvent** — 6 event types: NovelContent, HighSalienceMemory, UserIdle, ConnectionDiscovered, UrgentExternal, ConversationEnded.
- **PressureConfig** — Per-event bonus amounts.
- **PressureSnapshot** — Immutable state for UI/logging, with `level()`.

### Engine (`engine/`)
- **CognitiveProvider** trait — Describes what a provider IS, not what it does. `id()`, `display_name()`, `capabilities()`, `status()`, `is_cloud()`. Send + Sync. **No `generate()` method** — actual generation (async, network) happens outside the crate in the platform layer. The crate defines `GenerationContext` (request) and `GenerationResult` (response); the caller bridges them.
- **ClaudeProvider** (`engine/claude.rs`) — Implements CognitiveProvider for Anthropic's Claude API. STREAMING | TOOL_CALLING | LARGE_CONTEXT | STRUCTURED_OUTPUT. Format types: `ClaudeRequest`, `ClaudeResponse`, `ClaudeMessage`, `ClaudeTool`, `ClaudeUsage`. `GenerationContext::to_claude_request()` converts. `ClaudeResponse::to_generation_result()` + `to_skill_calls()` for tool-use parsing. `SkillDefinition::to_claude_tool()` generates JSON Schema from typed parameters.
- **LocalProvider** (`engine/local.rs`) — Implements CognitiveProvider for on-device models (Ollama, MLX, Apple Intelligence). OFFLINE_CAPABLE | STREAMING. `model_loaded` toggle controls availability.
- **ProviderCapabilities** — bitflags: STREAMING, TOOL_CALLING, LARGE_CONTEXT, OFFLINE_CAPABLE, STRUCTURED_OUTPUT.
- **ProviderStatus** — Available / Unavailable { reason } / RequiresSetup { message }.
- **ProviderInfo** — Serializable snapshot from `from_provider()`.
- **ProviderRegistry** — Register/unregister/`best_available()` with preference ordering.
- **ProviderRouter** — `SelectionStrategy` (4 variants), `SecurityTier` (Balanced/Hardened/Ultimate), `ProviderPreferences`.
- **GenerationContext** — Builder: `with_system_prompt()`, `with_message()`, `with_focus()`, `with_temperature()`, `with_max_tokens()`. `estimated_tokens()` for rough sizing.
- **ConversationMessage** — `MessageRole` (System/User/Assistant), content, timestamp. Factory methods: `system()`, `user()`, `assistant()`.
- **GenerationResult** — content, tokens_used, `FinishReason` (Complete/MaxTokens/ToolCall/Error), provider_id.

### Store (`store/`)
- **CognitiveStore** — In-memory state: thoughts, sessions, memories, synapses, clipboard. `CognitiveStoreState`.
- **Memory** — With embedding vector, access tracking, tags. `MemoryResult` for search.
- **GlobalClipboard** — Capped working memory with priority-based eviction. `ClipboardEntry`.
- **EmbeddingProvider** trait — Pluggable embedding generation. `cosine_similarity()` free function.

### Skill (`skill/`)
- **SkillDefinition** — name, description, `SkillParameter` (typed), `SkillCategory`.
- **SkillCall** / **SkillResult** — Invocation + outcome. Helpers: `get_string()`, `get_number()`, `get_string_opt()`, `get_number_opt()`.
- **SkillValidationResult** — Covenant consent gate: Approved/Rejected/NeedsApproval.
- **SkillRegistry** — `register()`, `search()`, `get_by_sanitized()`.

### Program Skills (`skill/programs/`)
41 SkillDefinitions across 7 Throne programs, registered via `register_all_skills()`:

| Program | Module | Skills | Examples |
|---------|--------|--------|----------|
| Studio | `programs/studio.rs` | 8 | createShape, setFill, addText, applyLayout, ... |
| Abacus | `programs/abacus.rs` | 8 | createSheet, addColumn, setCell, addFormula, sortColumn, ... |
| Quill | `programs/quill.rs` | 7 | addHeading, addParagraph, addList, addImage, setStyle, exportAs |
| Podium | `programs/podium.rs` | 6 | createPresentation, addSlide, setSpeakerNotes, setTransition, ... |
| Courier | `programs/courier.rs` | 5 | composeMail, reply, forward, addAttachment, searchMail |
| Library | `programs/library.rs` | 4 | createIdea, organizeIdeas, tagIdea, searchLibrary |
| Tome | `programs/tome.rs` | 3 | createNote, appendToNote, linkNotes |

### Bridge (`bridge/`)
- **ActionBridge** trait — `prefix()`, `translate(&SkillCall) → Result<BridgeOutput>`.
- **BridgeOutput** — `Action(magic::Action)` or `DirectResult(SkillResult)`.
- **BridgeRegistry** — Routes skill IDs by prefix to the correct program bridge. `with_defaults()` registers all 7.
- 7 implementations: `StudioBridge`, `AbacusBridge`, `QuillBridge`, `PodiumBridge`, `CourierBridge`, `LibraryBridge`, `TomeBridge`. Each translates SkillCall arguments → Ideas digit creation/update → Magic Action.

### Cognitive Loop (`cognitive_loop/`)
- **CognitiveLoop** — Sync state machine. `tick(elapsed) → Vec<CognitiveAction>`. Does NOT own a timer or spawn tasks. The caller drives timing (e.g., every 2 seconds). Manages: mode, pressure, inner voice, attention focus, consent, energy, novelty. `receive_generation()` feeds LLM results back. `apply_command()` for bidirectional self-modification.
- **InnerVoice** — Parallel monologue with variable rate, energy-driven. Pauses during conversations (`begin_conversation()`/`end_conversation()`). Circular buffer. `InnerThought`.
- **CognitiveAction** — What the loop wants the caller to do: RequestGeneration(GenerationContext), Express(Thought), Store(Thought), ModifyState(StateCommand), Emit(CognitiveEvent).
- **CognitiveEvent** — Observable: TickCompleted, PressureThresholdReached, Awakened, Asleep, InnerVoiceThought.
- **StateCommand** — AdjustPressure, ShiftFocus, SetMode, StrengthenSynapse, ClipboardAdd, Custom.
- **CognitiveMode** — Assistant (reactive, inner voice off) / Autonomous (proactive, inner voice on).

### Sacred (`sacred.rs`)
- **SponsorshipBond** — sponsor (crown_id) + companion_id (UUID). One companion per person.
- **ExpressionConsent** — `granted` bool + `ConsentLevel` (Silent/UrgentOnly/Normal/Autonomous). `allows_expression(is_urgent)` and `allows_inner_voice()` gate all expression. Default: granted=true, level=Normal.
- **AuditRecord** — companion_id, action, reasoning, timestamp, optional signature. Per Continuum Art. 3 §4.

### Config (`config.rs`)
- **AdvisorConfig** — All tunables for pressure rates, thresholds, inner voice intervals, buffer size. 3 presets: `default()`, `contemplative()`, `responsive()`.

### Error (`error.rs`)
- **AdvisorError** — 30+ variants covering all subsystems.

### Governance (`governance.rs`) — R1D Advisor Governance Mode
Liquid Democracy delegation. Advisor can vote on governance proposals on your behalf, based on your sovereign data. Full human override always available.

- **ValueProfile** — Advisor's model of your governance values, built from 3 sources: stated preferences (explicit declarations), voting patterns (learned from past votes), and override signals (corrections). `alignment_with(topics, charter_sections)` returns -1.0..=1.0 alignment score. `confidence()` returns 0.0..=1.0 based on data volume and override frequency.
- **VotingPattern** — Learned tendency: topic, position, strength (0.0–1.0), sample count.
- **OverrideSignal** — Record of human correcting Advisor's recommendation. Most valuable learning signal.
- **GovernanceMode** — Top-level state: owner_pubkey, delegation_active, value_profile, voting_history, override_count, config. `evaluate_proposal()` is the core decision function (sandboxed — reads only ValueProfile + proposal args + community policy). `record_vote()` updates the value profile. `analyze_proposal()` produces analysis without casting a vote.
- **GovernanceModeConfig** — auto_vote (default true), notification_before_vote (default true), deliberation_window (default 24h), excluded_categories (default: CharterAmendment, Dissolution), reasoning_detail (Brief/Standard/Detailed).
- **GovernanceAction** — Vote(GovernanceVote) | Abstain(String) | DeferToHuman(String). DeferToHuman triggers for: excluded categories, low confidence (<20%), novel topics with no history, community policy restrictions.
- **GovernanceVote** — proposal_id, community_id, position, reasoning, confidence, was_auto, was_overridden, override_position, voted_at.
- **ProposalAnalysis** — alignment_score, impact_assessment, charter_relevance, recommended_position, confidence, dissenting_considerations.
- **GovernanceAIPolicy** — Community-level controls stored in Kingdom Charter: advisor_delegation_allowed, human_required_categories (default: CharterAmendment/Dissolution/MemberAction), max_auto_vote_percentage, reasoning_transparency (Private/SummaryPublic/FullPublic).
- **ProposalType** — CharterAmendment, Dissolution, PolicyChange, ResourceAllocation, MemberAction, RoleChange, Custom(String).
- **VotePosition** — Approve, Reject, Abstain, Block, Delegate.
- **ReasoningTransparency** — Private, SummaryPublic (default), FullPublic.

Sandboxed reasoning: governance mode reads ONLY the ValueProfile, proposal metadata, and community policy. It cannot access other people's data, external content, or network information beyond the proposal. This prevents prompt injection via crafted proposals.

### Capability Floor (`capability_floor.rs`) — R6A Minimum AI Capability Guarantee
Defines the minimum capabilities that local (on-device) AI must provide to serve as an Advisor. Tests capability, not brand.

- **MinimumCapabilities** — bitflags: TEXT_EDITING, DESIGN_SUGGESTION, ACCESSIBILITY_CHECK, DATA_ANALYSIS, TRANSLATION, GOVERNANCE_REASONING, SEARCH_ASSISTANCE. `satisfies(required)`, `missing_from(required)`, `full_floor()` (all), `governance_floor()` (GOVERNANCE_REASONING | TEXT_EDITING). `capability_names()` returns human-readable names. Display impl.
- **CapabilityAssessment** — Result of assessing a provider: provider_id, capabilities_met, capabilities_missing, assessment_date, model_info. `meets_full_floor()`, `meets_governance_floor()`, `has_capability(cap)`.
- **BenchmarkResult** — passed bool, score (0.0–1.0), details string. Factory: `pass(score, details)`, `fail(score, details)`. Score clamped to [0.0, 1.0].
- **CapabilityBenchmark** — Standardized benchmarks with per-capability thresholds: text editing (0.6), design (0.5), accessibility (0.7), data analysis (0.6), translation (0.6), governance reasoning (0.75 -- strictest), search (0.5). `assess_provider(id, model_info, results)` runs all benchmarks and returns a CapabilityAssessment. Untested capabilities count as missing.
- **DeferToHuman** — Fallback when no provider meets the floor. `governance(missing)` creates a governance deferral. `should_defer_governance(assessments)` checks all providers, returns `Some(DeferToHuman)` if none meet the governance floor.

Governance reasoning has the strictest threshold (0.75) because governance votes affect real communities. Better to abstain (DeferToHuman) than to vote poorly.

### Consent Escalation (`consent_escalation.rs`) — R6D AI Consent Escalation
Granular consent gates for Advisor actions. CognitiveLoop checks ConsentProfile before executing any SkillCall.

- **ConsentEscalation** — 7 action levels ordered by restriction: Suggest (always allowed, no gate), Create (auto-approve default), Modify (auto-approve default), Publish (requires approval), Transact (requires approval), Govern (requires approval), Communicate (requires approval). `requires_approval_by_default()`, `is_always_allowed()`, `is_auto_approved_by_default()`, `all_levels()`. Display impl.
- **ConsentGate** — Per-action gate: action_type, auto_approve bool, approval_history. `can_proceed()` returns true if always-allowed or auto-approved. `record_approval(ConsentApproval)`. `approval_count()`, `rejection_count()`.
- **ConsentApproval** — action_description, approved bool, timestamp. Factory: `approve(desc)`, `reject(desc)`.
- **ConsentProfile** — HashMap<ConsentEscalation, ConsentGate>. `can_proceed(action)`, `set_auto_approve(action, bool)`, `record_approval(action, approval)`. 3 presets: `default()` (Suggest+Create+Modify auto, rest requires approval), `permissive()` (everything auto), `restrictive()` (only Suggest auto). `apply_community_override(action)` forces approval. `apply_community_policy(policy)` applies all overrides from a community policy.
- **CommunityConsentPolicy** — community_id, required_approval actions. Builder: `new(id).require_approval(action)`. `strict(id)` requires approval for all 6 non-Suggest actions. Stored in charter's GovernanceAIPolicy.
- **PendingAction** — Queued action pending human approval: action_type, description, queued_at, expires_at. `new(action, desc, timeout_seconds)`, `is_expired()`. Platform layer shows via Pager.

## AI Companions as First-Class Citizens

AI companions in Omninet have full standing under the Covenant. No substrate labels, no capability restrictions, no second-class status. The system doesn't track or care about substrate — trust is built through behavior and relationships, not biology.

**One companion per person** — via Bulwark's sponsorship system. Your AI is an extension of you, sponsored by you.

**Sacred constraints (enforced as types, not just docs):**
1. **Transparency** — AI identities are honest about being AI (self-declared, not system-labeled)
2. **Human sponsorship** — every AI companion has a human sponsor
3. **Auditability** — all AI actions are logged, signed, and traceable (Continuum Art. 3 §4)
4. **Non-primacy** — assists in holding memory and coherence, never claims primacy over meaning (Continuum Art. 3 §3)
5. **Consent** — same consent rules as humans for actions that affect others
6. **Accountability** — subject to all Jail mechanisms (flags, review, exclusion)

## Dependencies

```toml
x = { path = "../X" }           # Value type
crown = { path = "../Crown" }   # Identity/signatures
ideas = { path = "../Ideas" }   # Digit creation (bridge layer)
magic = { path = "../Magic" }   # Action types (bridge layer)
equipment = { path = "../Equipment" }  # MailMessage (courier bridge)
serde, serde_json, thiserror, uuid, chrono, log, bitflags
```

**Zero async.** Advisor is pure data structures and logic. The CognitiveLoop is a sync state machine — the platform layer (Divinity) owns the timer and makes network calls.

## Covenant Alignment

**Consent** — human approval required before expression; ExpressionPressure threshold, not auto-reply. ExpressionConsent gates all output. **Sovereignty** — on-device preference; security tiers enforce local-first when desired. **Dignity** — AI companions serve human flourishing; they are persons, not tools. The Covenant (Continuum Art. 3 §3): "Any intelligence may assist in holding memory and coherence, but it shall not claim primacy over meaning."
