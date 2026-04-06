//! Workflow automation — declarative event-driven pipelines.
//!
//! Workflows are "when X happens, if Y is true, do Z" — expressed as data,
//! not code. This keeps them safe (no arbitrary execution), serializable
//! (persist to Vault, sync across devices), and user-configurable (Oracle
//! guides creation).
//!
//! # Architecture
//!
//! - [`Trigger`] — event pattern that starts a workflow (kind/tag/author match)
//! - [`Condition`] — additional check before actions fire (field comparisons)
//! - [`ActionSpec`] — declarative description of what to do (Phone call, Email post, Pager notification)
//! - [`Workflow`] — trigger → conditions → action chain, with metadata
//! - [`WorkflowRegistry`] — register/unregister workflows, evaluate events, produce action specs
//! - [`AuditEntry`] — immutable record of every workflow execution
//!
//! # Dependency-free design
//!
//! Oracle defines the [`ActionExecutor`] trait but does NOT import Equipment.
//! The executor is provided by the host application, which wires up Phone/Email/Pager
//! calls behind the trait. Oracle produces [`ActionSpec`] values; the executor
//! consumes them.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::OracleError;

// ---------------------------------------------------------------------------
// MARK: - Trigger
// ---------------------------------------------------------------------------

/// An event pattern that can start a workflow.
///
/// Triggers match against incoming events. All specified fields must match
/// (AND logic). Unspecified fields are wildcards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trigger {
    /// Match events of this kind (Gospel event kind number).
    /// `None` means "any kind."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<u32>,

    /// Match events that contain ALL of these tags.
    /// Empty means "any tags." Tag format: `["key", "value"]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagMatch>,

    /// Match events from this author (public key hex).
    /// `None` means "any author."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// A scheduled trigger (cron-like). When set, the workflow fires on
    /// schedule rather than in response to events. The event fields above
    /// are ignored for scheduled triggers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<Schedule>,
}

impl Trigger {
    /// Create a trigger that matches a specific event kind.
    pub fn on_kind(kind: u32) -> Self {
        Self {
            kind: Some(kind),
            tags: Vec::new(),
            author: None,
            schedule: None,
        }
    }

    /// Create a trigger that matches events from a specific author.
    pub fn on_author(pubkey: impl Into<String>) -> Self {
        Self {
            kind: None,
            tags: Vec::new(),
            author: Some(pubkey.into()),
            schedule: None,
        }
    }

    /// Create a scheduled trigger.
    pub fn on_schedule(schedule: Schedule) -> Self {
        Self {
            kind: None,
            tags: Vec::new(),
            author: None,
            schedule: Some(schedule),
        }
    }

    /// Add a required tag match.
    pub fn with_tag(mut self, tag: TagMatch) -> Self {
        self.tags.push(tag);
        self
    }

    /// Also require a specific kind.
    pub fn with_kind(mut self, kind: u32) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Also require a specific author.
    pub fn with_author(mut self, pubkey: impl Into<String>) -> Self {
        self.author = Some(pubkey.into());
        self
    }

    /// Check whether this trigger matches an incoming event.
    ///
    /// Scheduled triggers never match events (they fire on schedule).
    pub fn matches(&self, event: &WorkflowEvent) -> bool {
        // Scheduled triggers don't match events.
        if self.schedule.is_some() {
            return false;
        }

        // Kind check.
        if let Some(required_kind) = self.kind {
            if event.kind != required_kind {
                return false;
            }
        }

        // Author check.
        if let Some(ref required_author) = self.author {
            if event.author != *required_author {
                return false;
            }
        }

        // Tag check — all required tags must be present.
        for required_tag in &self.tags {
            if !event.tags.iter().any(|(k, v)| {
                k == &required_tag.key && v == &required_tag.value
            }) {
                return false;
            }
        }

        true
    }
}

/// A tag key-value pair to match against.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagMatch {
    /// Tag key (e.g., "t", "p", "e").
    pub key: String,
    /// Tag value to match.
    pub value: String,
}

impl TagMatch {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// A schedule for time-based triggers.
///
/// Simplified cron-like schedule. The host evaluates this against the
/// current time. Oracle does not run a scheduler — it defines the data
/// and the host calls `check_scheduled()` at appropriate intervals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    /// Day of week (0 = Sunday, 6 = Saturday). `None` = every day.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub day_of_week: Option<u8>,

    /// Hour of day (0-23). `None` = every hour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hour: Option<u8>,

    /// Minute of hour (0-59). `None` = top of hour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minute: Option<u8>,

    /// Human-readable label (e.g., "Every Monday at 9am").
    pub label: String,
}

impl Schedule {
    /// Weekly schedule: specific day, hour, minute.
    pub fn weekly(day_of_week: u8, hour: u8, minute: u8, label: impl Into<String>) -> Self {
        Self {
            day_of_week: Some(day_of_week),
            hour: Some(hour),
            minute: Some(minute),
            label: label.into(),
        }
    }

    /// Daily schedule: specific hour and minute.
    pub fn daily(hour: u8, minute: u8, label: impl Into<String>) -> Self {
        Self {
            day_of_week: None,
            hour: Some(hour),
            minute: Some(minute),
            label: label.into(),
        }
    }

    /// Hourly schedule: specific minute.
    pub fn hourly(minute: u8, label: impl Into<String>) -> Self {
        Self {
            day_of_week: None,
            hour: None,
            minute: Some(minute),
            label: label.into(),
        }
    }

    /// Check if a time point matches this schedule.
    ///
    /// `day_of_week`: 0=Sunday..6=Saturday.
    /// `hour`: 0-23. `minute`: 0-59.
    pub fn matches_time(&self, day_of_week: u8, hour: u8, minute: u8) -> bool {
        if let Some(required_day) = self.day_of_week {
            if day_of_week != required_day {
                return false;
            }
        }
        if let Some(required_hour) = self.hour {
            if hour != required_hour {
                return false;
            }
        }
        if let Some(required_minute) = self.minute {
            if minute != required_minute {
                return false;
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// MARK: - Condition
// ---------------------------------------------------------------------------

/// A condition that must be true for the workflow to proceed.
///
/// Conditions inspect the event's fields (key-value pairs in `fields`).
/// Multiple conditions are AND-ed together.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Condition {
    /// Field equals a specific value.
    Equals { field: String, value: String },
    /// Field does NOT equal a specific value.
    NotEquals { field: String, value: String },
    /// Field contains a substring.
    Contains { field: String, substring: String },
    /// Field is present (any value).
    Exists { field: String },
    /// Field is absent.
    NotExists { field: String },
    /// All sub-conditions must be true.
    All(Vec<Condition>),
    /// At least one sub-condition must be true.
    Any(Vec<Condition>),
}

impl Condition {
    /// Evaluate this condition against an event's fields.
    pub fn evaluate(&self, fields: &HashMap<String, String>) -> bool {
        match self {
            Self::Equals { field, value } => {
                fields.get(field).is_some_and(|v| v == value)
            }
            Self::NotEquals { field, value } => {
                // True if field is missing OR has a different value.
                fields.get(field) != Some(value)
            }
            Self::Contains { field, substring } => {
                fields.get(field).is_some_and(|v| v.contains(substring.as_str()))
            }
            Self::Exists { field } => fields.contains_key(field),
            Self::NotExists { field } => !fields.contains_key(field),
            Self::All(conditions) => conditions.iter().all(|c| c.evaluate(fields)),
            Self::Any(conditions) => conditions.iter().any(|c| c.evaluate(fields)),
        }
    }
}

// ---------------------------------------------------------------------------
// MARK: - Action
// ---------------------------------------------------------------------------

/// A declarative description of an action to perform.
///
/// These are pure data — they describe WHAT to do, not HOW.
/// The [`ActionExecutor`] trait handles the HOW. This separation
/// keeps Oracle dependency-free.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionSpec {
    /// Make an RPC call (Equipment's Phone).
    /// The `call_id` routes to a registered handler.
    PhoneCall {
        /// Phone call ID (e.g., "vault.getEntries").
        call_id: String,
        /// JSON payload for the call.
        payload: String,
    },

    /// Broadcast an event (Equipment's Email).
    /// Fire-and-forget to all subscribers.
    EmailPost {
        /// Email event ID (e.g., "crdt.documentChanged").
        email_id: String,
        /// JSON payload for the event.
        payload: String,
    },

    /// Queue a notification (Equipment's Pager).
    PagerNotify {
        /// Notification title.
        title: String,
        /// Optional notification body.
        #[serde(skip_serializing_if = "Option::is_none")]
        body: Option<String>,
        /// Source module identifier.
        source_module: String,
    },

    /// Publish a Gospel event (Equipment's Globe, via Phone RPC).
    PublishEvent {
        /// Event kind number.
        kind: u32,
        /// Event content.
        content: String,
        /// Event tags.
        tags: Vec<(String, String)>,
    },

    /// Log a message (for audit/debugging workflows).
    Log {
        /// Log level ("info", "warn", "error").
        level: String,
        /// Log message.
        message: String,
    },
}

/// Trait for executing workflow actions.
///
/// The host application implements this trait, wiring up Equipment's
/// Phone, Email, and Pager behind it. Oracle never imports Equipment.
pub trait ActionExecutor: Send + Sync {
    /// Execute an action. Returns Ok on success, Err with reason on failure.
    fn execute(&self, action: &ActionSpec, context: &ActionContext) -> Result<(), String>;
}

/// Context available to an action executor.
///
/// Contains information about the triggering event and the workflow
/// so the executor can make informed decisions (e.g., consent checks).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionContext {
    /// The workflow that produced this action.
    pub workflow_id: String,
    /// The actor (user pubkey) who owns this workflow.
    pub actor: String,
    /// Fields from the triggering event.
    pub event_fields: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// MARK: - WorkflowEvent
// ---------------------------------------------------------------------------

/// A simplified representation of an incoming event for trigger matching.
///
/// This is NOT a Gospel event — it's an Oracle-local DTO that the host
/// constructs from whatever event source it uses. Keeps Oracle free
/// from Globe dependencies.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowEvent {
    /// Event kind number.
    pub kind: u32,
    /// Author's public key (hex).
    pub author: String,
    /// Event tags as key-value pairs.
    pub tags: Vec<(String, String)>,
    /// Arbitrary fields extracted from the event (for condition evaluation).
    pub fields: HashMap<String, String>,
}

impl WorkflowEvent {
    /// Create a new event.
    pub fn new(kind: u32, author: impl Into<String>) -> Self {
        Self {
            kind,
            author: author.into(),
            tags: Vec::new(),
            fields: HashMap::new(),
        }
    }

    /// Add a tag.
    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.push((key.into(), value.into()));
        self
    }

    /// Add a field.
    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }
}

// ---------------------------------------------------------------------------
// MARK: - Workflow
// ---------------------------------------------------------------------------

/// A complete workflow definition: trigger + conditions + actions.
///
/// Workflows are declarative data. They describe WHAT should happen
/// in response to events, not HOW. The host executes them.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workflow {
    /// Unique identifier for this workflow.
    pub id: String,
    /// Human-readable name (e.g., "Notify marketing on logo approval").
    pub name: String,
    /// Optional description for the user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The event pattern that starts this workflow.
    pub trigger: Trigger,
    /// Conditions that must ALL be true (AND logic).
    /// Empty means "always proceed."
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
    /// Actions to perform in order.
    pub actions: Vec<ActionSpec>,
    /// Whether this workflow is currently active.
    pub enabled: bool,
    /// The actor (user pubkey) who owns this workflow.
    pub actor: String,
    /// Optional scope: community ID, user pubkey, or "global".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<WorkflowScope>,
    /// Whether the actor has explicitly consented to this workflow's actions.
    /// Workflows without consent are never executed.
    pub consented: bool,
}

impl Workflow {
    /// Create a new workflow with required fields.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        actor: impl Into<String>,
        trigger: Trigger,
        actions: Vec<ActionSpec>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            trigger,
            conditions: Vec::new(),
            actions,
            enabled: true,
            actor: actor.into(),
            scope: None,
            consented: false,
        }
    }

    /// Set the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add a condition.
    pub fn with_condition(mut self, condition: Condition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Set the scope.
    pub fn with_scope(mut self, scope: WorkflowScope) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Mark as consented (the actor explicitly approved these actions).
    pub fn with_consent(mut self) -> Self {
        self.consented = true;
        self
    }

    /// Check if this workflow should fire for the given event.
    ///
    /// Returns true when:
    /// 1. The workflow is enabled and consented
    /// 2. The trigger matches the event
    /// 3. All conditions evaluate to true
    pub fn should_fire(&self, event: &WorkflowEvent) -> bool {
        if !self.enabled || !self.consented {
            return false;
        }

        if !self.trigger.matches(event) {
            return false;
        }

        self.conditions.iter().all(|c| c.evaluate(&event.fields))
    }

    /// Check if this is a scheduled workflow (fires on time, not events).
    pub fn is_scheduled(&self) -> bool {
        self.trigger.schedule.is_some()
    }

    /// Check if this scheduled workflow should fire at the given time.
    ///
    /// Returns false for non-scheduled workflows.
    pub fn should_fire_at(&self, day_of_week: u8, hour: u8, minute: u8) -> bool {
        if !self.enabled || !self.consented {
            return false;
        }

        self.trigger
            .schedule
            .as_ref()
            .is_some_and(|s| s.matches_time(day_of_week, hour, minute))
    }
}

/// The scope a workflow applies to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowScope {
    /// Applies globally (all events the user sees).
    Global,
    /// Applies to a specific community.
    Community(String),
    /// Applies to a specific user (their events only).
    User(String),
}

// ---------------------------------------------------------------------------
// MARK: - Audit trail
// ---------------------------------------------------------------------------

/// An immutable record of a workflow execution.
///
/// Every time a workflow fires (or is skipped), an entry is logged.
/// This provides accountability and debugging.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    /// The workflow that was evaluated.
    pub workflow_id: String,
    /// The actor who owns the workflow.
    pub actor: String,
    /// What happened.
    pub outcome: AuditOutcome,
    /// Unix timestamp (seconds) when this occurred.
    pub timestamp: u64,
    /// Snapshot of the event that triggered this (if event-driven).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_event: Option<WorkflowEvent>,
}

/// The outcome of a workflow evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditOutcome {
    /// All actions executed successfully.
    Success,
    /// Workflow was skipped because conditions were not met.
    ConditionsNotMet,
    /// Workflow was skipped because it is disabled.
    Disabled,
    /// Workflow was skipped because the actor has not consented.
    NoConsent,
    /// One or more actions failed.
    ActionFailed {
        /// Index of the first action that failed.
        action_index: usize,
        /// The error message.
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// MARK: - WorkflowRegistry
// ---------------------------------------------------------------------------

/// Central registry for workflows. Evaluates events and produces actions.
///
/// The registry is the runtime engine. It holds workflows, evaluates
/// incoming events against triggers, checks conditions, and either
/// produces action specs (for the host to execute) or directly executes
/// them through an [`ActionExecutor`] if one is provided.
pub struct WorkflowRegistry {
    /// All registered workflows, keyed by workflow ID.
    workflows: HashMap<String, Workflow>,
    /// Audit trail of all evaluations.
    audit_log: Vec<AuditEntry>,
    /// Maximum audit log size before oldest entries are pruned.
    max_audit_entries: usize,
}

impl WorkflowRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            workflows: HashMap::new(),
            audit_log: Vec::new(),
            max_audit_entries: 10_000,
        }
    }

    /// Create a registry with a custom audit log limit.
    pub fn with_max_audit(max_entries: usize) -> Self {
        Self {
            workflows: HashMap::new(),
            audit_log: Vec::new(),
            max_audit_entries: max_entries,
        }
    }

    /// Register a workflow. Replaces any existing workflow with the same ID.
    pub fn register(&mut self, workflow: Workflow) {
        self.workflows.insert(workflow.id.clone(), workflow);
    }

    /// Unregister a workflow by ID. Returns the removed workflow if it existed.
    pub fn unregister(&mut self, workflow_id: &str) -> Option<Workflow> {
        self.workflows.remove(workflow_id)
    }

    /// Get a workflow by ID.
    pub fn get(&self, workflow_id: &str) -> Option<&Workflow> {
        self.workflows.get(workflow_id)
    }

    /// Get a mutable reference to a workflow by ID.
    pub fn get_mut(&mut self, workflow_id: &str) -> Option<&mut Workflow> {
        self.workflows.get_mut(workflow_id)
    }

    /// Enable a workflow. Returns an error if the workflow does not exist.
    pub fn enable(&mut self, workflow_id: &str) -> Result<(), OracleError> {
        let wf = self.workflows.get_mut(workflow_id)
            .ok_or_else(|| OracleError::WorkflowNotFound(workflow_id.to_string()))?;
        wf.enabled = true;
        Ok(())
    }

    /// Disable a workflow. Returns an error if the workflow does not exist.
    pub fn disable(&mut self, workflow_id: &str) -> Result<(), OracleError> {
        let wf = self.workflows.get_mut(workflow_id)
            .ok_or_else(|| OracleError::WorkflowNotFound(workflow_id.to_string()))?;
        wf.enabled = false;
        Ok(())
    }

    /// Grant consent for a workflow. Returns an error if the workflow does not exist.
    pub fn grant_consent(&mut self, workflow_id: &str) -> Result<(), OracleError> {
        let wf = self.workflows.get_mut(workflow_id)
            .ok_or_else(|| OracleError::WorkflowNotFound(workflow_id.to_string()))?;
        wf.consented = true;
        Ok(())
    }

    /// Revoke consent for a workflow. Returns an error if the workflow does not exist.
    pub fn revoke_consent(&mut self, workflow_id: &str) -> Result<(), OracleError> {
        let wf = self.workflows.get_mut(workflow_id)
            .ok_or_else(|| OracleError::WorkflowNotFound(workflow_id.to_string()))?;
        wf.consented = false;
        Ok(())
    }

    /// List all registered workflows.
    pub fn list(&self) -> Vec<&Workflow> {
        self.workflows.values().collect()
    }

    /// List workflows for a specific actor.
    pub fn list_for_actor(&self, actor: &str) -> Vec<&Workflow> {
        self.workflows.values()
            .filter(|w| w.actor == actor)
            .collect()
    }

    /// List workflows for a specific scope.
    pub fn list_for_scope(&self, scope: &WorkflowScope) -> Vec<&Workflow> {
        self.workflows.values()
            .filter(|w| w.scope.as_ref() == Some(scope))
            .collect()
    }

    /// Count of registered workflows.
    pub fn count(&self) -> usize {
        self.workflows.len()
    }

    /// Evaluate an incoming event against all workflows.
    ///
    /// Returns a list of (workflow_id, actions) pairs for workflows that
    /// matched. Does NOT execute the actions — the caller is responsible
    /// for passing them to an [`ActionExecutor`].
    ///
    /// An audit entry is logged for every workflow that was evaluated
    /// (matched trigger but may have failed conditions or consent).
    pub fn evaluate(&mut self, event: &WorkflowEvent, timestamp: u64) -> Vec<WorkflowMatch> {
        let mut matches = Vec::new();

        // Collect evaluations first to avoid borrowing issues.
        let evaluations: Vec<(String, String, bool, bool, bool, Vec<ActionSpec>)> = self.workflows
            .values()
            .filter(|w| !w.is_scheduled()) // Skip scheduled workflows.
            .filter(|w| w.trigger.matches(event)) // Trigger matched.
            .map(|w| {
                let conditions_met = w.conditions.iter().all(|c| c.evaluate(&event.fields));
                (
                    w.id.clone(),
                    w.actor.clone(),
                    w.enabled,
                    w.consented,
                    conditions_met,
                    w.actions.clone(),
                )
            })
            .collect();

        for (id, actor, enabled, consented, conditions_met, actions) in evaluations {
            let outcome = if !enabled {
                AuditOutcome::Disabled
            } else if !consented {
                AuditOutcome::NoConsent
            } else if !conditions_met {
                AuditOutcome::ConditionsNotMet
            } else {
                AuditOutcome::Success
            };

            let should_emit = outcome == AuditOutcome::Success;

            self.append_audit(AuditEntry {
                workflow_id: id.clone(),
                actor: actor.clone(),
                outcome,
                timestamp,
                trigger_event: Some(event.clone()),
            });

            if should_emit {
                matches.push(WorkflowMatch {
                    workflow_id: id,
                    actor,
                    actions,
                });
            }
        }

        matches
    }

    /// Evaluate an incoming event and immediately execute matched actions.
    ///
    /// Uses the provided [`ActionExecutor`] to run actions. Logs success
    /// or failure in the audit trail.
    pub fn evaluate_and_execute(
        &mut self,
        event: &WorkflowEvent,
        timestamp: u64,
        executor: &dyn ActionExecutor,
    ) -> Vec<ExecutionResult> {
        let matched = self.evaluate(event, timestamp);
        let mut results = Vec::new();

        for wf_match in &matched {
            let context = ActionContext {
                workflow_id: wf_match.workflow_id.clone(),
                actor: wf_match.actor.clone(),
                event_fields: event.fields.clone(),
            };

            let mut success = true;
            for (i, action) in wf_match.actions.iter().enumerate() {
                if let Err(reason) = executor.execute(action, &context) {
                    // Overwrite the Success entry we logged during evaluate()
                    // with the actual failure.
                    self.append_audit(AuditEntry {
                        workflow_id: wf_match.workflow_id.clone(),
                        actor: wf_match.actor.clone(),
                        outcome: AuditOutcome::ActionFailed {
                            action_index: i,
                            reason: reason.clone(),
                        },
                        timestamp,
                        trigger_event: Some(event.clone()),
                    });

                    results.push(ExecutionResult {
                        workflow_id: wf_match.workflow_id.clone(),
                        success: false,
                        error: Some(reason),
                    });
                    success = false;
                    break; // Stop executing remaining actions for this workflow.
                }
            }

            if success {
                results.push(ExecutionResult {
                    workflow_id: wf_match.workflow_id.clone(),
                    success: true,
                    error: None,
                });
            }
        }

        results
    }

    /// Check which scheduled workflows should fire at the given time.
    ///
    /// Returns workflow IDs and their actions. The caller is responsible
    /// for executing actions and calling this at appropriate intervals.
    pub fn check_scheduled(
        &mut self,
        day_of_week: u8,
        hour: u8,
        minute: u8,
        timestamp: u64,
    ) -> Vec<WorkflowMatch> {
        let mut matches = Vec::new();

        let evaluations: Vec<(String, String, bool, bool, Vec<ActionSpec>)> = self.workflows
            .values()
            .filter(|w| w.is_scheduled())
            .filter(|w| {
                w.trigger
                    .schedule
                    .as_ref()
                    .is_some_and(|s| s.matches_time(day_of_week, hour, minute))
            })
            .map(|w| {
                (
                    w.id.clone(),
                    w.actor.clone(),
                    w.enabled,
                    w.consented,
                    w.actions.clone(),
                )
            })
            .collect();

        for (id, actor, enabled, consented, actions) in evaluations {
            let outcome = if !enabled {
                AuditOutcome::Disabled
            } else if !consented {
                AuditOutcome::NoConsent
            } else {
                AuditOutcome::Success
            };

            let should_emit = outcome == AuditOutcome::Success;

            self.append_audit(AuditEntry {
                workflow_id: id.clone(),
                actor: actor.clone(),
                outcome,
                timestamp,
                trigger_event: None,
            });

            if should_emit {
                matches.push(WorkflowMatch {
                    workflow_id: id,
                    actor,
                    actions,
                });
            }
        }

        matches
    }

    /// Get the audit log (newest first).
    pub fn audit_log(&self) -> &[AuditEntry] {
        &self.audit_log
    }

    /// Get audit entries for a specific workflow.
    pub fn audit_for_workflow(&self, workflow_id: &str) -> Vec<&AuditEntry> {
        self.audit_log
            .iter()
            .filter(|e| e.workflow_id == workflow_id)
            .collect()
    }

    /// Clear the audit log.
    pub fn clear_audit_log(&mut self) {
        self.audit_log.clear();
    }

    /// Export all workflows for persistence.
    pub fn export_workflows(&self) -> Vec<Workflow> {
        self.workflows.values().cloned().collect()
    }

    /// Import workflows (e.g., from persistence). Replaces existing.
    pub fn import_workflows(&mut self, workflows: Vec<Workflow>) {
        for wf in workflows {
            self.workflows.insert(wf.id.clone(), wf);
        }
    }

    /// Append an audit entry, pruning oldest if over limit.
    fn append_audit(&mut self, entry: AuditEntry) {
        self.audit_log.push(entry);
        if self.audit_log.len() > self.max_audit_entries {
            // Remove the oldest entries to stay within the limit.
            let excess = self.audit_log.len() - self.max_audit_entries;
            self.audit_log.drain(0..excess);
        }
    }
}

impl Default for WorkflowRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A workflow that matched an event, with its actions ready for execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowMatch {
    /// The workflow that matched.
    pub workflow_id: String,
    /// The actor who owns the workflow.
    pub actor: String,
    /// Actions to execute, in order.
    pub actions: Vec<ActionSpec>,
}

/// Result of executing a single workflow's actions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// The workflow that was executed.
    pub workflow_id: String,
    /// Whether all actions succeeded.
    pub success: bool,
    /// Error message if an action failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// MARK: - Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Helpers --

    fn asset_approved_event() -> WorkflowEvent {
        WorkflowEvent::new(30001, "alice_pubkey")
            .with_tag("t", "asset")
            .with_tag("status", "approved")
            .with_field("asset.status", "approved")
            .with_field("asset.type", "logo")
    }

    fn notify_marketing_action() -> ActionSpec {
        ActionSpec::PagerNotify {
            title: "Logo approved".into(),
            body: Some("A new logo has been approved and is ready for review.".into()),
            source_module: "bam".into(),
        }
    }

    fn email_marketing_action() -> ActionSpec {
        ActionSpec::EmailPost {
            email_id: "marketing.assetApproved".into(),
            payload: r#"{"asset_type":"logo"}"#.into(),
        }
    }

    fn logo_approval_workflow() -> Workflow {
        Workflow::new(
            "notify_marketing_on_logo",
            "Notify marketing on logo approval",
            "alice_pubkey",
            Trigger::on_kind(30001)
                .with_tag(TagMatch::new("t", "asset"))
                .with_tag(TagMatch::new("status", "approved")),
            vec![notify_marketing_action(), email_marketing_action()],
        )
        .with_condition(Condition::Equals {
            field: "asset.status".into(),
            value: "approved".into(),
        })
        .with_condition(Condition::Equals {
            field: "asset.type".into(),
            value: "logo".into(),
        })
        .with_scope(WorkflowScope::Community("design_team".into()))
        .with_consent()
    }

    /// A test executor that records what was executed.
    struct RecordingExecutor {
        log: std::sync::Mutex<Vec<ActionSpec>>,
    }

    impl RecordingExecutor {
        fn new() -> Self {
            Self {
                log: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn executed(&self) -> Vec<ActionSpec> {
            self.log.lock().unwrap().clone()
        }
    }

    impl ActionExecutor for RecordingExecutor {
        fn execute(&self, action: &ActionSpec, _context: &ActionContext) -> Result<(), String> {
            self.log.lock().unwrap().push(action.clone());
            Ok(())
        }
    }

    /// An executor that fails on a specific action index.
    struct FailingExecutor {
        fail_at: usize,
        counter: std::sync::atomic::AtomicUsize,
    }

    impl FailingExecutor {
        fn new(fail_at: usize) -> Self {
            Self {
                fail_at,
                counter: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl ActionExecutor for FailingExecutor {
        fn execute(&self, _action: &ActionSpec, _context: &ActionContext) -> Result<(), String> {
            let current = self.counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if current == self.fail_at {
                Err("intentional failure".into())
            } else {
                Ok(())
            }
        }
    }

    // -- Trigger tests --

    #[test]
    fn trigger_kind_match() {
        let trigger = Trigger::on_kind(30001);
        let event = WorkflowEvent::new(30001, "alice");
        assert!(trigger.matches(&event));
    }

    #[test]
    fn trigger_kind_mismatch() {
        let trigger = Trigger::on_kind(30001);
        let event = WorkflowEvent::new(1, "alice");
        assert!(!trigger.matches(&event));
    }

    #[test]
    fn trigger_author_match() {
        let trigger = Trigger::on_author("alice");
        let event = WorkflowEvent::new(1, "alice");
        assert!(trigger.matches(&event));
    }

    #[test]
    fn trigger_author_mismatch() {
        let trigger = Trigger::on_author("alice");
        let event = WorkflowEvent::new(1, "bob");
        assert!(!trigger.matches(&event));
    }

    #[test]
    fn trigger_tag_match() {
        let trigger = Trigger::on_kind(1)
            .with_tag(TagMatch::new("t", "asset"));
        let event = WorkflowEvent::new(1, "alice")
            .with_tag("t", "asset");
        assert!(trigger.matches(&event));
    }

    #[test]
    fn trigger_tag_mismatch() {
        let trigger = Trigger::on_kind(1)
            .with_tag(TagMatch::new("t", "asset"));
        let event = WorkflowEvent::new(1, "alice")
            .with_tag("t", "post");
        assert!(!trigger.matches(&event));
    }

    #[test]
    fn trigger_multiple_tags_all_required() {
        let trigger = Trigger::on_kind(1)
            .with_tag(TagMatch::new("t", "asset"))
            .with_tag(TagMatch::new("status", "approved"));

        let partial = WorkflowEvent::new(1, "alice")
            .with_tag("t", "asset");
        assert!(!trigger.matches(&partial));

        let full = WorkflowEvent::new(1, "alice")
            .with_tag("t", "asset")
            .with_tag("status", "approved");
        assert!(trigger.matches(&full));
    }

    #[test]
    fn trigger_scheduled_never_matches_events() {
        let trigger = Trigger::on_schedule(Schedule::daily(9, 0, "Daily 9am"));
        let event = WorkflowEvent::new(1, "alice");
        assert!(!trigger.matches(&event));
    }

    #[test]
    fn trigger_wildcard_matches_any_event() {
        let trigger = Trigger {
            kind: None,
            tags: Vec::new(),
            author: None,
            schedule: None,
        };
        let event = WorkflowEvent::new(42, "bob");
        assert!(trigger.matches(&event));
    }

    // -- Schedule tests --

    #[test]
    fn schedule_weekly() {
        let sched = Schedule::weekly(1, 9, 0, "Monday 9am"); // Monday
        assert!(sched.matches_time(1, 9, 0));
        assert!(!sched.matches_time(2, 9, 0)); // Tuesday
        assert!(!sched.matches_time(1, 10, 0)); // Wrong hour
    }

    #[test]
    fn schedule_daily() {
        let sched = Schedule::daily(17, 30, "5:30pm daily");
        assert!(sched.matches_time(0, 17, 30)); // Sunday
        assert!(sched.matches_time(4, 17, 30)); // Thursday
        assert!(!sched.matches_time(0, 17, 31)); // Wrong minute
    }

    #[test]
    fn schedule_hourly() {
        let sched = Schedule::hourly(15, "Quarter past every hour");
        assert!(sched.matches_time(0, 0, 15));
        assert!(sched.matches_time(3, 14, 15));
        assert!(!sched.matches_time(0, 0, 16));
    }

    // -- Condition tests --

    #[test]
    fn condition_equals() {
        let cond = Condition::Equals {
            field: "status".into(),
            value: "approved".into(),
        };
        let mut fields = HashMap::new();
        fields.insert("status".into(), "approved".into());
        assert!(cond.evaluate(&fields));

        fields.insert("status".into(), "rejected".into());
        assert!(!cond.evaluate(&fields));
    }

    #[test]
    fn condition_not_equals() {
        let cond = Condition::NotEquals {
            field: "status".into(),
            value: "rejected".into(),
        };
        let mut fields = HashMap::new();
        fields.insert("status".into(), "approved".into());
        assert!(cond.evaluate(&fields));

        fields.insert("status".into(), "rejected".into());
        assert!(!cond.evaluate(&fields));
    }

    #[test]
    fn condition_not_equals_missing_field() {
        let cond = Condition::NotEquals {
            field: "status".into(),
            value: "rejected".into(),
        };
        let fields = HashMap::new();
        // Missing field counts as "not equal."
        assert!(cond.evaluate(&fields));
    }

    #[test]
    fn condition_contains() {
        let cond = Condition::Contains {
            field: "title".into(),
            substring: "urgent".into(),
        };
        let mut fields = HashMap::new();
        fields.insert("title".into(), "This is urgent!".into());
        assert!(cond.evaluate(&fields));

        fields.insert("title".into(), "Normal update".into());
        assert!(!cond.evaluate(&fields));
    }

    #[test]
    fn condition_exists_and_not_exists() {
        let exists = Condition::Exists {
            field: "pubkey".into(),
        };
        let not_exists = Condition::NotExists {
            field: "pubkey".into(),
        };

        let mut fields = HashMap::new();
        assert!(!exists.evaluate(&fields));
        assert!(not_exists.evaluate(&fields));

        fields.insert("pubkey".into(), "abc".into());
        assert!(exists.evaluate(&fields));
        assert!(!not_exists.evaluate(&fields));
    }

    #[test]
    fn condition_all() {
        let cond = Condition::All(vec![
            Condition::Equals { field: "a".into(), value: "1".into() },
            Condition::Equals { field: "b".into(), value: "2".into() },
        ]);
        let mut fields = HashMap::new();
        fields.insert("a".into(), "1".into());
        fields.insert("b".into(), "2".into());
        assert!(cond.evaluate(&fields));

        fields.insert("b".into(), "3".into());
        assert!(!cond.evaluate(&fields));
    }

    #[test]
    fn condition_any() {
        let cond = Condition::Any(vec![
            Condition::Equals { field: "a".into(), value: "1".into() },
            Condition::Equals { field: "a".into(), value: "2".into() },
        ]);
        let mut fields = HashMap::new();
        fields.insert("a".into(), "1".into());
        assert!(cond.evaluate(&fields));

        fields.insert("a".into(), "2".into());
        assert!(cond.evaluate(&fields));

        fields.insert("a".into(), "3".into());
        assert!(!cond.evaluate(&fields));
    }

    #[test]
    fn condition_nested() {
        // (a == "1" AND b == "2") OR c == "3"
        let cond = Condition::Any(vec![
            Condition::All(vec![
                Condition::Equals { field: "a".into(), value: "1".into() },
                Condition::Equals { field: "b".into(), value: "2".into() },
            ]),
            Condition::Equals { field: "c".into(), value: "3".into() },
        ]);

        let mut fields = HashMap::new();
        fields.insert("a".into(), "1".into());
        fields.insert("b".into(), "2".into());
        assert!(cond.evaluate(&fields));

        let mut fields2 = HashMap::new();
        fields2.insert("c".into(), "3".into());
        assert!(cond.evaluate(&fields2));

        let mut fields3 = HashMap::new();
        fields3.insert("a".into(), "1".into());
        assert!(!cond.evaluate(&fields3));
    }

    // -- Workflow tests --

    #[test]
    fn workflow_should_fire_happy_path() {
        let wf = logo_approval_workflow();
        let event = asset_approved_event();
        assert!(wf.should_fire(&event));
    }

    #[test]
    fn workflow_disabled_does_not_fire() {
        let mut wf = logo_approval_workflow();
        wf.enabled = false;
        assert!(!wf.should_fire(&asset_approved_event()));
    }

    #[test]
    fn workflow_no_consent_does_not_fire() {
        let mut wf = logo_approval_workflow();
        wf.consented = false;
        assert!(!wf.should_fire(&asset_approved_event()));
    }

    #[test]
    fn workflow_condition_fails() {
        let wf = logo_approval_workflow();
        let event = WorkflowEvent::new(30001, "alice_pubkey")
            .with_tag("t", "asset")
            .with_tag("status", "approved")
            .with_field("asset.status", "approved")
            .with_field("asset.type", "banner"); // NOT a logo
        assert!(!wf.should_fire(&event));
    }

    #[test]
    fn workflow_trigger_mismatch() {
        let wf = logo_approval_workflow();
        let event = WorkflowEvent::new(1, "alice_pubkey"); // Wrong kind
        assert!(!wf.should_fire(&event));
    }

    #[test]
    fn scheduled_workflow() {
        let wf = Workflow::new(
            "weekly_report",
            "Weekly health report",
            "admin",
            Trigger::on_schedule(Schedule::weekly(1, 9, 0, "Monday 9am")),
            vec![ActionSpec::PhoneCall {
                call_id: "health.generateReport".into(),
                payload: "{}".into(),
            }],
        ).with_consent();

        assert!(wf.is_scheduled());
        assert!(wf.should_fire_at(1, 9, 0));
        assert!(!wf.should_fire_at(2, 9, 0)); // Tuesday

        // Scheduled workflows never match events.
        assert!(!wf.should_fire(&WorkflowEvent::new(1, "alice")));
    }

    // -- Registry tests --

    #[test]
    fn registry_register_and_get() {
        let mut reg = WorkflowRegistry::new();
        reg.register(logo_approval_workflow());

        assert_eq!(reg.count(), 1);
        assert!(reg.get("notify_marketing_on_logo").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn registry_unregister() {
        let mut reg = WorkflowRegistry::new();
        reg.register(logo_approval_workflow());
        let removed = reg.unregister("notify_marketing_on_logo");
        assert!(removed.is_some());
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn registry_enable_disable() {
        let mut reg = WorkflowRegistry::new();
        reg.register(logo_approval_workflow());

        reg.disable("notify_marketing_on_logo").unwrap();
        assert!(!reg.get("notify_marketing_on_logo").unwrap().enabled);

        reg.enable("notify_marketing_on_logo").unwrap();
        assert!(reg.get("notify_marketing_on_logo").unwrap().enabled);

        // Nonexistent workflow errors.
        assert!(reg.enable("nope").is_err());
        assert!(reg.disable("nope").is_err());
    }

    #[test]
    fn registry_consent() {
        let mut reg = WorkflowRegistry::new();
        let mut wf = logo_approval_workflow();
        wf.consented = false;
        reg.register(wf);

        assert!(!reg.get("notify_marketing_on_logo").unwrap().consented);

        reg.grant_consent("notify_marketing_on_logo").unwrap();
        assert!(reg.get("notify_marketing_on_logo").unwrap().consented);

        reg.revoke_consent("notify_marketing_on_logo").unwrap();
        assert!(!reg.get("notify_marketing_on_logo").unwrap().consented);

        assert!(reg.grant_consent("nope").is_err());
        assert!(reg.revoke_consent("nope").is_err());
    }

    #[test]
    fn registry_evaluate_matches() {
        let mut reg = WorkflowRegistry::new();
        reg.register(logo_approval_workflow());

        let event = asset_approved_event();
        let matches = reg.evaluate(&event, 1000);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].workflow_id, "notify_marketing_on_logo");
        assert_eq!(matches[0].actions.len(), 2);
    }

    #[test]
    fn registry_evaluate_no_match() {
        let mut reg = WorkflowRegistry::new();
        reg.register(logo_approval_workflow());

        let event = WorkflowEvent::new(1, "bob"); // Wrong kind
        let matches = reg.evaluate(&event, 1000);
        assert!(matches.is_empty());
    }

    #[test]
    fn registry_evaluate_disabled_workflow() {
        let mut reg = WorkflowRegistry::new();
        let mut wf = logo_approval_workflow();
        wf.enabled = false;
        reg.register(wf);

        let event = asset_approved_event();
        let matches = reg.evaluate(&event, 1000);
        assert!(matches.is_empty());

        // Audit should record the skip.
        let audit = reg.audit_log();
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].outcome, AuditOutcome::Disabled);
    }

    #[test]
    fn registry_evaluate_no_consent() {
        let mut reg = WorkflowRegistry::new();
        let mut wf = logo_approval_workflow();
        wf.consented = false;
        reg.register(wf);

        let event = asset_approved_event();
        let matches = reg.evaluate(&event, 1000);
        assert!(matches.is_empty());

        let audit = reg.audit_log();
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].outcome, AuditOutcome::NoConsent);
    }

    #[test]
    fn registry_evaluate_conditions_not_met() {
        let mut reg = WorkflowRegistry::new();
        reg.register(logo_approval_workflow());

        // Right trigger, wrong conditions.
        let event = WorkflowEvent::new(30001, "alice_pubkey")
            .with_tag("t", "asset")
            .with_tag("status", "approved")
            .with_field("asset.status", "approved")
            .with_field("asset.type", "banner");

        let matches = reg.evaluate(&event, 1000);
        assert!(matches.is_empty());

        let audit = reg.audit_log();
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].outcome, AuditOutcome::ConditionsNotMet);
    }

    #[test]
    fn registry_evaluate_and_execute() {
        let mut reg = WorkflowRegistry::new();
        reg.register(logo_approval_workflow());

        let executor = RecordingExecutor::new();
        let event = asset_approved_event();
        let results = reg.evaluate_and_execute(&event, 1000, &executor);

        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert_eq!(executor.executed().len(), 2);
    }

    #[test]
    fn registry_evaluate_and_execute_action_failure() {
        let mut reg = WorkflowRegistry::new();
        reg.register(logo_approval_workflow());

        let executor = FailingExecutor::new(1); // Fail on second action.
        let event = asset_approved_event();
        let results = reg.evaluate_and_execute(&event, 1000, &executor);

        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert_eq!(results[0].error.as_deref(), Some("intentional failure"));

        // Audit log should have the failure recorded.
        let failures: Vec<_> = reg.audit_log().iter()
            .filter(|e| matches!(e.outcome, AuditOutcome::ActionFailed { .. }))
            .collect();
        assert_eq!(failures.len(), 1);
    }

    #[test]
    fn registry_scheduled_evaluation() {
        let mut reg = WorkflowRegistry::new();
        reg.register(
            Workflow::new(
                "weekly_report",
                "Weekly health report",
                "admin",
                Trigger::on_schedule(Schedule::weekly(1, 9, 0, "Monday 9am")),
                vec![ActionSpec::PhoneCall {
                    call_id: "health.generateReport".into(),
                    payload: "{}".into(),
                }],
            ).with_consent(),
        );

        let matches = reg.check_scheduled(1, 9, 0, 2000);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].workflow_id, "weekly_report");

        let no_match = reg.check_scheduled(2, 9, 0, 2000);
        assert!(no_match.is_empty());
    }

    #[test]
    fn registry_scheduled_skips_without_consent() {
        let mut reg = WorkflowRegistry::new();
        reg.register(
            Workflow::new(
                "weekly_report",
                "Weekly health report",
                "admin",
                Trigger::on_schedule(Schedule::weekly(1, 9, 0, "Monday 9am")),
                vec![ActionSpec::Log {
                    level: "info".into(),
                    message: "Report generated".into(),
                }],
            ), // No consent!
        );

        let matches = reg.check_scheduled(1, 9, 0, 2000);
        assert!(matches.is_empty());

        let audit = reg.audit_log();
        assert_eq!(audit[0].outcome, AuditOutcome::NoConsent);
    }

    #[test]
    fn registry_list_for_actor() {
        let mut reg = WorkflowRegistry::new();
        reg.register(logo_approval_workflow()); // actor: alice_pubkey
        reg.register(
            Workflow::new("other", "Other", "bob_pubkey", Trigger::on_kind(1), vec![])
                .with_consent(),
        );

        let alice_wfs = reg.list_for_actor("alice_pubkey");
        assert_eq!(alice_wfs.len(), 1);
        assert_eq!(alice_wfs[0].id, "notify_marketing_on_logo");

        let bob_wfs = reg.list_for_actor("bob_pubkey");
        assert_eq!(bob_wfs.len(), 1);
    }

    #[test]
    fn registry_list_for_scope() {
        let mut reg = WorkflowRegistry::new();
        reg.register(logo_approval_workflow()); // scope: Community("design_team")
        reg.register(
            Workflow::new("global_wf", "Global", "alice", Trigger::on_kind(1), vec![])
                .with_scope(WorkflowScope::Global)
                .with_consent(),
        );

        let community_wfs = reg.list_for_scope(&WorkflowScope::Community("design_team".into()));
        assert_eq!(community_wfs.len(), 1);

        let global_wfs = reg.list_for_scope(&WorkflowScope::Global);
        assert_eq!(global_wfs.len(), 1);
    }

    #[test]
    fn registry_audit_for_workflow() {
        let mut reg = WorkflowRegistry::new();
        reg.register(logo_approval_workflow());
        reg.register(
            Workflow::new("other", "Other", "bob", Trigger::on_kind(1), vec![])
                .with_consent(),
        );

        let event = asset_approved_event();
        reg.evaluate(&event, 1000);

        let event2 = WorkflowEvent::new(1, "bob");
        reg.evaluate(&event2, 2000);

        let logo_audit = reg.audit_for_workflow("notify_marketing_on_logo");
        assert_eq!(logo_audit.len(), 1);
        assert_eq!(logo_audit[0].workflow_id, "notify_marketing_on_logo");
    }

    #[test]
    fn registry_audit_pruning() {
        let mut reg = WorkflowRegistry::with_max_audit(5);
        reg.register(
            Workflow::new("w", "W", "alice", Trigger::on_kind(1), vec![])
                .with_consent(),
        );

        for i in 0..10 {
            let event = WorkflowEvent::new(1, "alice");
            reg.evaluate(&event, i);
        }

        assert_eq!(reg.audit_log().len(), 5);
        // The oldest entries should have been pruned.
        assert_eq!(reg.audit_log()[0].timestamp, 5);
    }

    #[test]
    fn registry_export_import() {
        let mut reg = WorkflowRegistry::new();
        reg.register(logo_approval_workflow());

        let exported = reg.export_workflows();
        assert_eq!(exported.len(), 1);

        let mut reg2 = WorkflowRegistry::new();
        reg2.import_workflows(exported);
        assert_eq!(reg2.count(), 1);
        assert!(reg2.get("notify_marketing_on_logo").is_some());
    }

    #[test]
    fn registry_clear_audit_log() {
        let mut reg = WorkflowRegistry::new();
        reg.register(
            Workflow::new("w", "W", "alice", Trigger::on_kind(1), vec![])
                .with_consent(),
        );
        reg.evaluate(&WorkflowEvent::new(1, "alice"), 1000);
        assert!(!reg.audit_log().is_empty());

        reg.clear_audit_log();
        assert!(reg.audit_log().is_empty());
    }

    // -- Serde round-trip tests --

    #[test]
    fn trigger_serde() {
        let trigger = Trigger::on_kind(30001)
            .with_tag(TagMatch::new("t", "asset"))
            .with_author("alice");
        let json = serde_json::to_string(&trigger).unwrap();
        let loaded: Trigger = serde_json::from_str(&json).unwrap();
        assert_eq!(trigger, loaded);
    }

    #[test]
    fn schedule_serde() {
        let sched = Schedule::weekly(1, 9, 0, "Monday 9am");
        let json = serde_json::to_string(&sched).unwrap();
        let loaded: Schedule = serde_json::from_str(&json).unwrap();
        assert_eq!(sched, loaded);
    }

    #[test]
    fn condition_serde() {
        let cond = Condition::All(vec![
            Condition::Equals { field: "a".into(), value: "1".into() },
            Condition::Any(vec![
                Condition::Exists { field: "b".into() },
                Condition::Contains { field: "c".into(), substring: "hello".into() },
            ]),
        ]);
        let json = serde_json::to_string(&cond).unwrap();
        let loaded: Condition = serde_json::from_str(&json).unwrap();
        assert_eq!(cond, loaded);
    }

    #[test]
    fn action_spec_serde_phone() {
        let action = ActionSpec::PhoneCall {
            call_id: "vault.getEntries".into(),
            payload: r#"{"limit":10}"#.into(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let loaded: ActionSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(action, loaded);
    }

    #[test]
    fn action_spec_serde_email() {
        let action = ActionSpec::EmailPost {
            email_id: "crdt.documentChanged".into(),
            payload: r#"{"id":"abc"}"#.into(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let loaded: ActionSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(action, loaded);
    }

    #[test]
    fn action_spec_serde_pager() {
        let action = ActionSpec::PagerNotify {
            title: "Alert".into(),
            body: Some("Something happened".into()),
            source_module: "crown".into(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let loaded: ActionSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(action, loaded);
    }

    #[test]
    fn action_spec_serde_publish() {
        let action = ActionSpec::PublishEvent {
            kind: 1,
            content: "hello".into(),
            tags: vec![("t".into(), "post".into())],
        };
        let json = serde_json::to_string(&action).unwrap();
        let loaded: ActionSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(action, loaded);
    }

    #[test]
    fn action_spec_serde_log() {
        let action = ActionSpec::Log {
            level: "info".into(),
            message: "Workflow fired".into(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let loaded: ActionSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(action, loaded);
    }

    #[test]
    fn workflow_serde() {
        let wf = logo_approval_workflow();
        let json = serde_json::to_string(&wf).unwrap();
        let loaded: Workflow = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, "notify_marketing_on_logo");
        assert_eq!(loaded.actions.len(), 2);
        assert!(loaded.consented);
    }

    #[test]
    fn workflow_scope_serde() {
        for scope in [
            WorkflowScope::Global,
            WorkflowScope::Community("design_team".into()),
            WorkflowScope::User("alice".into()),
        ] {
            let json = serde_json::to_string(&scope).unwrap();
            let loaded: WorkflowScope = serde_json::from_str(&json).unwrap();
            assert_eq!(scope, loaded);
        }
    }

    #[test]
    fn audit_entry_serde() {
        let entry = AuditEntry {
            workflow_id: "w1".into(),
            actor: "alice".into(),
            outcome: AuditOutcome::Success,
            timestamp: 1000,
            trigger_event: Some(WorkflowEvent::new(1, "alice")),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let loaded: AuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.workflow_id, "w1");
        assert_eq!(loaded.outcome, AuditOutcome::Success);
    }

    #[test]
    fn audit_outcome_serde() {
        for outcome in [
            AuditOutcome::Success,
            AuditOutcome::ConditionsNotMet,
            AuditOutcome::Disabled,
            AuditOutcome::NoConsent,
            AuditOutcome::ActionFailed {
                action_index: 2,
                reason: "boom".into(),
            },
        ] {
            let json = serde_json::to_string(&outcome).unwrap();
            let loaded: AuditOutcome = serde_json::from_str(&json).unwrap();
            assert_eq!(outcome, loaded);
        }
    }

    #[test]
    fn workflow_event_serde() {
        let event = asset_approved_event();
        let json = serde_json::to_string(&event).unwrap();
        let loaded: WorkflowEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.kind, 30001);
        assert_eq!(loaded.author, "alice_pubkey");
    }

    #[test]
    fn workflow_match_serde() {
        let wm = WorkflowMatch {
            workflow_id: "w1".into(),
            actor: "alice".into(),
            actions: vec![notify_marketing_action()],
        };
        let json = serde_json::to_string(&wm).unwrap();
        let loaded: WorkflowMatch = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.workflow_id, "w1");
    }

    #[test]
    fn execution_result_serde() {
        let r = ExecutionResult {
            workflow_id: "w1".into(),
            success: false,
            error: Some("timeout".into()),
        };
        let json = serde_json::to_string(&r).unwrap();
        let loaded: ExecutionResult = serde_json::from_str(&json).unwrap();
        assert!(!loaded.success);
        assert_eq!(loaded.error.as_deref(), Some("timeout"));
    }

    #[test]
    fn action_context_serde() {
        let ctx = ActionContext {
            workflow_id: "w1".into(),
            actor: "alice".into(),
            event_fields: {
                let mut m = HashMap::new();
                m.insert("key".into(), "val".into());
                m
            },
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let loaded: ActionContext = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.workflow_id, "w1");
    }

    #[test]
    fn tag_match_serde() {
        let tm = TagMatch::new("t", "asset");
        let json = serde_json::to_string(&tm).unwrap();
        let loaded: TagMatch = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.key, "t");
        assert_eq!(loaded.value, "asset");
    }

    // -- Multiple workflows test --

    #[test]
    fn multiple_workflows_match_same_event() {
        let mut reg = WorkflowRegistry::new();

        // Two workflows, same trigger, different actions.
        reg.register(
            Workflow::new(
                "wf1",
                "Workflow 1",
                "alice",
                Trigger::on_kind(1),
                vec![ActionSpec::Log {
                    level: "info".into(),
                    message: "wf1 fired".into(),
                }],
            ).with_consent(),
        );
        reg.register(
            Workflow::new(
                "wf2",
                "Workflow 2",
                "alice",
                Trigger::on_kind(1),
                vec![ActionSpec::Log {
                    level: "info".into(),
                    message: "wf2 fired".into(),
                }],
            ).with_consent(),
        );

        let event = WorkflowEvent::new(1, "alice");
        let matches = reg.evaluate(&event, 1000);
        assert_eq!(matches.len(), 2);
    }

    // -- Consent enforcement test --

    #[test]
    fn consent_is_required_for_execution() {
        let mut reg = WorkflowRegistry::new();
        reg.register(
            Workflow::new(
                "unconsented",
                "No Consent",
                "alice",
                Trigger::on_kind(1),
                vec![ActionSpec::PagerNotify {
                    title: "Should not fire".into(),
                    body: None,
                    source_module: "test".into(),
                }],
            ), // NOT consented
        );

        let executor = RecordingExecutor::new();
        let event = WorkflowEvent::new(1, "alice");
        let results = reg.evaluate_and_execute(&event, 1000, &executor);

        assert!(results.is_empty());
        assert!(executor.executed().is_empty());
    }

    // -- Edge cases --

    #[test]
    fn empty_conditions_always_pass() {
        let wf = Workflow::new(
            "no_conds",
            "No Conditions",
            "alice",
            Trigger::on_kind(1),
            vec![],
        ).with_consent();

        let event = WorkflowEvent::new(1, "alice");
        assert!(wf.should_fire(&event));
    }

    #[test]
    fn empty_actions_workflow_still_matches() {
        let mut reg = WorkflowRegistry::new();
        reg.register(
            Workflow::new("empty_actions", "No Actions", "alice", Trigger::on_kind(1), vec![])
                .with_consent(),
        );

        let event = WorkflowEvent::new(1, "alice");
        let matches = reg.evaluate(&event, 1000);
        assert_eq!(matches.len(), 1);
        assert!(matches[0].actions.is_empty());
    }

    #[test]
    fn get_mut_updates_workflow() {
        let mut reg = WorkflowRegistry::new();
        reg.register(
            Workflow::new("w", "W", "alice", Trigger::on_kind(1), vec![])
                .with_consent(),
        );

        let wf = reg.get_mut("w").unwrap();
        wf.name = "Updated Name".into();

        assert_eq!(reg.get("w").unwrap().name, "Updated Name");
    }

    #[test]
    fn unregister_nonexistent_returns_none() {
        let mut reg = WorkflowRegistry::new();
        assert!(reg.unregister("nope").is_none());
    }

    #[test]
    fn workflow_with_description() {
        let wf = Workflow::new("w", "W", "alice", Trigger::on_kind(1), vec![])
            .with_description("A test workflow");
        assert_eq!(wf.description.as_deref(), Some("A test workflow"));
    }
}
