# Oracle — Guidance & Onboarding

The source of truth. Oracle guides new participants from curiosity to sovereignty. The front door. 30 seconds from nothing to belonging. Then it steps back — until you need it again.

## Architecture

### Activation Flow (`activation.rs`)
State machine orchestrating onboarding. Pluggable steps via `ActivationStep` trait.
- **ActivationStep** trait — `id()`, `name()`, `description()`, `can_skip()`, `execute(context)`, `rollback(context)`. Each step receives and can modify a shared `HashMap<String, String>` context (e.g., pubkey from identity creation flows downstream).
- **ActivationFlow** — `add_step()`, `advance()`, `skip_current()`, `progress()`, `is_complete()`. Rollback on failure: completed steps rolled back in reverse order.
- **FlowConfig** — Configuration for the flow.
- **StepResult** / **StepStatus** / **StepId** — Step lifecycle types.
- Per-app flows: different step sequences via different registrations.

### Contextual Hints (`hints.rs`)
Guidance that appears when needed, disappears when not.
- **OracleHint** trait — `id()`, `should_show(context)`, `message()`, `action()`, `priority()`.
- **StaticHint** — Data-driven hints with `required_context` matching.
- **HintEngine** — Evaluates all registered hints against `HintContext`, filters dismissed, sorts by priority.
- **HintPriority** — Low / Medium / High / Critical.
- **HintAction** — Navigate / OpenUrl / Dismiss / Custom.
- Dismissed hints persist (caller's responsibility to save/load IDs).

### Recovery Flow (`recovery.rs`)
Restore identity on a new device.
- **RecoveryMethod** trait — `id()`, `name()`, `instructions()`, `recover(input)`.
- **RecoveryFlow** — `register()`, `select_method()`, `attempt(input)`, `reset()`.
- **RecoveryResult** — Restored { pubkey } / Failed / NeedsInput.
- **RecoveryStatus** — state tracking for the flow.
- BIP-39 is the launch method. Social recovery, hardware keys are future `RecoveryMethod` impls.

### Progressive Disclosure / Sovereignty Tiers (`disclosure.rs`)
Four sovereignty tiers, determined by behavior not selection. No tier is better — different engagement, equal dignity.

- **SovereigntyTier** — Sheltered (delegated) / Citizen (default) / Steward (governance) / Architect (protocol). Ordered enum. `UserLevel` is a backward-compatible type alias.
  - **Sheltered** — delegated sovereignty. A parent, caretaker, or trusted person manages participation. Signals are ignored. Exit only via explicit un-delegation or age threshold.
  - **Citizen** — sensible defaults. Identity is yours, Advisor handles governance delegation, everything works out of the box. Default for new participants.
  - **Steward** — active governance. Proposes, adjudicates, moderates. Unlocked by governance participation signals.
  - **Architect** — protocol-level. Operates Towers, interprets Covenant, builds tools. Unlocked by protocol participation signals.

- **DisclosureSignal** — 13 built-in + Custom. Steward signals: OpenedSettings, ChangedSetting, ViewedNetworkStats, ToggledFeature, ViewedRawData, ProposedInGovernance, VotedDirectly, ServedAsAdjudicator. Architect signals: UsedCli, RanTower, EditedConfig, SubmittedPrecedent, ContributedCode. Each signal `contributes_to()` either Steward or Architect tier. Architect signals cross-count toward Steward.

- **DisclosureTracker** — Counts signals, transitions tiers at configurable thresholds (default: 3 for Steward, 2 for Architect). Sheltered ignores all signals. Manual override allows both upward and downward transitions. `clear_override()` recomputes from signals (never auto-assigns Sheltered).

- **DisclosureConfig** — `steward_threshold`, `architect_threshold`. Serde aliases for backward compat (`enthusiast_threshold`, `operator_threshold`).

- **TierDefaults** — Per-tier sensible defaults: `tier`, `delegate_type`, `notification_level`, `feature_visibility`. `for_tier()` returns defaults, `all()` returns all four.

- **DelegateType** — Person(String) / Advisor / Direct. Default: Sheltered=Person, Citizen=Advisor, Steward/Architect=Direct.

- **NotificationLevel** — Essential / Standard / Detailed / Everything. Ordered enum.

- **FeatureVisibility** — CreationOnly (Sheltered) / FullApp (Citizen) / Governance (Steward) / Protocol (Architect). `for_tier()` maps tier to visibility.

- **Backward compatibility:** `UserLevel` type alias for `SovereigntyTier`. Serde aliases on enum variants (`Regular`→Citizen, `Enthusiast`→Steward, `Operator`→Architect) and config fields. `level()` and `set_level()` methods alongside `tier()` and `set_tier()`.

### Workflow Automation (`workflow.rs`)
Event-driven "if this, then that" automation. Declarative data, not arbitrary code.
- **Trigger** — Event pattern matching. Fields: `kind` (event kind match), `tags` (Vec of `TagMatch` with key/value), `author` (pubkey match). All specified fields must match (AND logic). Also supports `Schedule` for time-based triggers (interval or cron-style).
- **Condition** — Composable condition tree: Equals, NotEquals, Contains, Exists, NotExists, All(Vec), Any(Vec). Evaluated against a `HashMap<String, String>` context.
- **ActionSpec** — Declarative action descriptions: PhoneCall, EmailPost, PagerNotify, PublishEvent, Log. These map to Equipment's Phone/Email/Pager but Oracle never imports Equipment.
- **ActionExecutor** trait — `execute(action, context) -> Result<()>`. The host implements this, wiring Equipment behind it (keeps Oracle dependency-free).
- **ActionContext** — context HashMap passed to the executor.
- **Workflow** — trigger → conditions → actions chain. `requires_consent` flag, optional description. `WorkflowScope` (User or Community).
- **WorkflowRegistry** — register/unregister/evaluate workflows. `evaluate(event) -> Vec<WorkflowMatch>` finds matching workflows. `evaluate_and_execute(event, executor)` runs the full pipeline with audit trail. `check_scheduled(executor)` evaluates time-based triggers.
- **WorkflowEvent** — The event type that triggers evaluate: kind, tags HashMap, author, context HashMap.
- **AuditEntry** / **AuditOutcome** — Immutable execution records. Auto-pruning (configurable max entries, default 1000).
- Workflows respect consent — `requires_consent` flag, no action without actor's permission.

### Error (`error.rs`)
- **OracleError** — 8 variants: StepFailed, CannotSkip, RollbackFailed, RecoveryFailed, InvalidState, EmptyFlow, StepNotFound, WorkflowNotFound.

## Dependencies

**Zero internal Omninet dependencies.** Oracle defines traits that other crates implement:
- Crown implements `ActivationStep` for identity creation
- Sentinal implements `RecoveryMethod` for BIP-39
- Globe implements steps for network connection
- Any crate registers `OracleHint` implementations
- Equipment implements `ActionExecutor` for workflow actions

```toml
serde, serde_json, thiserror, log
```

This keeps Oracle dependency-free — any app can embed it.

## Covenant Alignment

**Dignity** — sovereignty requires understanding; no one should be sovereign in name only. No tier is better — Citizen and Architect have equal dignity. **Consent** — informed consent requires education; Oracle makes consent meaningful, not ceremonial. Workflows respect consent — no automation without the actor's permission. **Sovereignty** — the person chooses their pace; Oracle guides but never forces. Downward tier transitions always available by choice.
