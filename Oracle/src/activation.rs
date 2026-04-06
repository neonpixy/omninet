//! Activation flow — state machine for onboarding.
//!
//! Each step implements `ActivationStep`. Steps can be added, removed,
//! or reordered without changing Oracle core. Different apps define
//! different flows:
//!
//! - Plexus: full flow (identity → community → social)
//! - Luminaria: lightweight (identity → create, community deferred)
//! - Satchel: wallet-focused (identity → recovery emphasis → wallet)

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::OracleError;

/// Unique identifier for a step (e.g., "create_identity", "backup_words").
pub type StepId = String;

/// The result of executing an activation step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepResult {
    /// Step completed successfully.
    Completed,
    /// Step was skipped (only if `can_skip()` returned true).
    Skipped,
    /// Step failed with a reason.
    Failed(String),
}

/// Current status of a step in the flow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    /// Not yet started.
    Pending,
    /// Currently executing.
    InProgress,
    /// Completed successfully.
    Completed,
    /// Skipped by the user.
    Skipped,
    /// Failed (can be retried).
    Failed(String),
    /// Rolled back after a later step failed.
    RolledBack,
}

impl StepStatus {
    /// Whether this step is in a terminal state (completed or skipped).
    pub fn is_done(&self) -> bool {
        matches!(self, Self::Completed | Self::Skipped)
    }
}

/// A pluggable activation step.
///
/// Implementations provide the actual logic. Oracle just orchestrates.
/// Crown implements this for identity creation. Sentinal for BIP-39 backup.
/// Globe for network connection. Each app registers its own steps.
pub trait ActivationStep: Send + Sync {
    /// Unique identifier for this step.
    fn id(&self) -> &str;

    /// Human-readable name shown to the user.
    fn name(&self) -> &str;

    /// Brief description of what this step does.
    fn description(&self) -> &str;

    /// Whether this step can be skipped. Default: false.
    fn can_skip(&self) -> bool {
        false
    }

    /// Execute the step. Returns success/failure.
    ///
    /// The context map provides data from previous steps (e.g., the
    /// pubkey from identity creation is available to later steps).
    fn execute(&self, context: &mut HashMap<String, String>) -> StepResult;

    /// Roll back the step if a later step fails. Default: no-op.
    fn rollback(&self, _context: &mut HashMap<String, String>) -> Result<(), String> {
        Ok(())
    }
}

/// Metadata for a registered step (without the trait object).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StepInfo {
    /// Step identifier.
    pub id: StepId,
    /// Display name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Whether skippable.
    pub can_skip: bool,
    /// Current status.
    pub status: StepStatus,
}

/// Configuration for an activation flow.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlowConfig {
    /// Flow name (e.g., "plexus_full", "luminaria_light").
    pub name: String,
    /// Whether to auto-rollback completed steps on failure.
    pub rollback_on_failure: bool,
}

impl Default for FlowConfig {
    fn default() -> Self {
        Self {
            name: "default".into(),
            rollback_on_failure: true,
        }
    }
}

/// The activation flow state machine.
///
/// Orchestrates a sequence of `ActivationStep` implementations.
/// Tracks progress, handles skips, and supports rollback.
pub struct ActivationFlow {
    /// Registered steps in order.
    steps: Vec<Box<dyn ActivationStep>>,
    /// Status of each step, keyed by step ID.
    status: HashMap<StepId, StepStatus>,
    /// Shared context between steps (key-value pairs).
    context: HashMap<String, String>,
    /// Index of the current step (0-based).
    current: usize,
    /// Flow configuration.
    config: FlowConfig,
}

impl ActivationFlow {
    /// Create a new empty flow with default config.
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            status: HashMap::new(),
            context: HashMap::new(),
            current: 0,
            config: FlowConfig::default(),
        }
    }

    /// Create a flow with custom config.
    pub fn with_config(config: FlowConfig) -> Self {
        Self {
            steps: Vec::new(),
            status: HashMap::new(),
            context: HashMap::new(),
            current: 0,
            config,
        }
    }

    /// Add a step to the end of the flow.
    pub fn add_step(&mut self, step: Box<dyn ActivationStep>) {
        let id = step.id().to_string();
        self.status.insert(id, StepStatus::Pending);
        self.steps.push(step);
    }

    /// Execute the next pending step.
    ///
    /// Returns the step's result and its ID. Returns `None` if all steps
    /// are done. On failure with `rollback_on_failure`, rolls back
    /// completed steps in reverse order.
    pub fn advance(&mut self) -> Result<Option<(StepId, StepResult)>, OracleError> {
        if self.steps.is_empty() {
            return Err(OracleError::EmptyFlow);
        }

        if self.current >= self.steps.len() {
            return Ok(None); // All done.
        }

        let step = &self.steps[self.current];
        let id = step.id().to_string();

        self.status.insert(id.clone(), StepStatus::InProgress);

        let result = step.execute(&mut self.context);

        match &result {
            StepResult::Completed => {
                self.status.insert(id.clone(), StepStatus::Completed);
                self.current += 1;
            }
            StepResult::Skipped => {
                self.status.insert(id.clone(), StepStatus::Skipped);
                self.current += 1;
            }
            StepResult::Failed(reason) => {
                self.status
                    .insert(id.clone(), StepStatus::Failed(reason.clone()));

                if self.config.rollback_on_failure {
                    self.rollback_completed()?;
                }
            }
        }

        Ok(Some((id, result)))
    }

    /// Skip the current step (if skippable).
    pub fn skip_current(&mut self) -> Result<StepId, OracleError> {
        if self.current >= self.steps.len() {
            return Err(OracleError::InvalidState("no current step".into()));
        }

        let step = &self.steps[self.current];
        let id = step.id().to_string();

        if !step.can_skip() {
            return Err(OracleError::CannotSkip(id));
        }

        self.status.insert(id.clone(), StepStatus::Skipped);
        self.current += 1;
        Ok(id)
    }

    /// Get info about all steps.
    pub fn steps_info(&self) -> Vec<StepInfo> {
        self.steps
            .iter()
            .map(|step| {
                let id = step.id().to_string();
                StepInfo {
                    id: id.clone(),
                    name: step.name().to_string(),
                    description: step.description().to_string(),
                    can_skip: step.can_skip(),
                    status: self.status.get(&id).cloned().unwrap_or(StepStatus::Pending),
                }
            })
            .collect()
    }

    /// Whether all steps are done (completed or skipped).
    pub fn is_complete(&self) -> bool {
        self.current >= self.steps.len()
            && self.status.values().all(|s| s.is_done())
    }

    /// Current step index (0-based).
    pub fn current_step(&self) -> usize {
        self.current
    }

    /// Total number of steps.
    pub fn total_steps(&self) -> usize {
        self.steps.len()
    }

    /// Progress as a fraction (0.0 to 1.0).
    pub fn progress(&self) -> f64 {
        if self.steps.is_empty() {
            return 0.0;
        }
        self.current as f64 / self.steps.len() as f64
    }

    /// Read a value from the shared context.
    pub fn context_get(&self, key: &str) -> Option<&String> {
        self.context.get(key)
    }

    /// Set a value in the shared context (for pre-seeding).
    pub fn context_set(&mut self, key: &str, value: &str) {
        self.context.insert(key.into(), value.into());
    }

    /// Get the status of a specific step.
    pub fn step_status(&self, step_id: &str) -> Option<&StepStatus> {
        self.status.get(step_id)
    }

    /// Roll back all completed steps in reverse order.
    fn rollback_completed(&mut self) -> Result<(), OracleError> {
        // Walk backwards through completed steps.
        for i in (0..self.current).rev() {
            let step = &self.steps[i];
            let id = step.id().to_string();

            if let Some(StepStatus::Completed) = self.status.get(&id) {
                if let Err(reason) = step.rollback(&mut self.context) {
                    return Err(OracleError::RollbackFailed {
                        step: id,
                        reason,
                    });
                }
                self.status.insert(id, StepStatus::RolledBack);
            }
        }
        self.current = 0;
        Ok(())
    }
}

impl Default for ActivationFlow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Test step implementations --

    struct CreateIdentityStep;
    impl ActivationStep for CreateIdentityStep {
        fn id(&self) -> &str { "create_identity" }
        fn name(&self) -> &str { "Create Identity" }
        fn description(&self) -> &str { "Generate your cryptographic identity" }
        fn execute(&self, ctx: &mut HashMap<String, String>) -> StepResult {
            ctx.insert("pubkey".into(), "abc123".into());
            StepResult::Completed
        }
        fn rollback(&self, ctx: &mut HashMap<String, String>) -> Result<(), String> {
            ctx.remove("pubkey");
            Ok(())
        }
    }

    struct BackupWordsStep;
    impl ActivationStep for BackupWordsStep {
        fn id(&self) -> &str { "backup_words" }
        fn name(&self) -> &str { "Backup Recovery Words" }
        fn description(&self) -> &str { "Write down your 12 recovery words" }
        fn can_skip(&self) -> bool { true }
        fn execute(&self, ctx: &mut HashMap<String, String>) -> StepResult {
            ctx.insert("backup_done".into(), "true".into());
            StepResult::Completed
        }
    }

    struct ConnectStep;
    impl ActivationStep for ConnectStep {
        fn id(&self) -> &str { "connect" }
        fn name(&self) -> &str { "Connect to Network" }
        fn description(&self) -> &str { "Join the Omnidea network" }
        fn execute(&self, ctx: &mut HashMap<String, String>) -> StepResult {
            if ctx.contains_key("pubkey") {
                ctx.insert("connected".into(), "true".into());
                StepResult::Completed
            } else {
                StepResult::Failed("no identity".into())
            }
        }
    }

    struct FailingStep;
    impl ActivationStep for FailingStep {
        fn id(&self) -> &str { "failing" }
        fn name(&self) -> &str { "Failing Step" }
        fn description(&self) -> &str { "Always fails" }
        fn execute(&self, _ctx: &mut HashMap<String, String>) -> StepResult {
            StepResult::Failed("intentional failure".into())
        }
    }

    struct UnskippableStep;
    impl ActivationStep for UnskippableStep {
        fn id(&self) -> &str { "unskippable" }
        fn name(&self) -> &str { "Required Step" }
        fn description(&self) -> &str { "Cannot be skipped" }
        fn execute(&self, _ctx: &mut HashMap<String, String>) -> StepResult {
            StepResult::Completed
        }
    }

    fn full_flow() -> ActivationFlow {
        let mut flow = ActivationFlow::new();
        flow.add_step(Box::new(CreateIdentityStep));
        flow.add_step(Box::new(BackupWordsStep));
        flow.add_step(Box::new(ConnectStep));
        flow
    }

    #[test]
    fn empty_flow_errors() {
        let mut flow = ActivationFlow::new();
        assert!(flow.advance().is_err());
    }

    #[test]
    fn full_flow_completes() {
        let mut flow = full_flow();
        assert_eq!(flow.total_steps(), 3);
        assert!(!flow.is_complete());

        // Step 1: Create identity.
        let (id, result) = flow.advance().unwrap().unwrap();
        assert_eq!(id, "create_identity");
        assert_eq!(result, StepResult::Completed);
        assert_eq!(flow.context_get("pubkey"), Some(&"abc123".into()));

        // Step 2: Backup words.
        let (id, result) = flow.advance().unwrap().unwrap();
        assert_eq!(id, "backup_words");
        assert_eq!(result, StepResult::Completed);

        // Step 3: Connect.
        let (id, result) = flow.advance().unwrap().unwrap();
        assert_eq!(id, "connect");
        assert_eq!(result, StepResult::Completed);

        // All done.
        assert!(flow.is_complete());
        assert!(flow.advance().unwrap().is_none());
        assert!((flow.progress() - 1.0).abs() < 0.001);
    }

    #[test]
    fn skip_step() {
        let mut flow = full_flow();
        flow.advance().unwrap(); // Complete identity.

        // Skip backup words (skippable).
        let skipped = flow.skip_current().unwrap();
        assert_eq!(skipped, "backup_words");
        assert_eq!(
            flow.step_status("backup_words"),
            Some(&StepStatus::Skipped)
        );

        // Connect should still work.
        let (_, result) = flow.advance().unwrap().unwrap();
        assert_eq!(result, StepResult::Completed);
        assert!(flow.is_complete());
    }

    #[test]
    fn cannot_skip_required_step() {
        let mut flow = ActivationFlow::new();
        flow.add_step(Box::new(UnskippableStep));

        let result = flow.skip_current();
        assert!(result.is_err());
    }

    #[test]
    fn failure_triggers_rollback() {
        let mut flow = ActivationFlow::with_config(FlowConfig {
            rollback_on_failure: true,
            ..Default::default()
        });
        flow.add_step(Box::new(CreateIdentityStep));
        flow.add_step(Box::new(FailingStep));

        // Step 1 completes.
        flow.advance().unwrap();
        assert_eq!(flow.context_get("pubkey"), Some(&"abc123".into()));

        // Step 2 fails → rollback.
        let (id, result) = flow.advance().unwrap().unwrap();
        assert_eq!(id, "failing");
        assert!(matches!(result, StepResult::Failed(_)));

        // Identity step was rolled back.
        assert_eq!(
            flow.step_status("create_identity"),
            Some(&StepStatus::RolledBack)
        );
        // Context was cleaned by rollback.
        assert!(flow.context_get("pubkey").is_none());
        // Current reset to 0.
        assert_eq!(flow.current_step(), 0);
    }

    #[test]
    fn no_rollback_when_disabled() {
        let mut flow = ActivationFlow::with_config(FlowConfig {
            rollback_on_failure: false,
            ..Default::default()
        });
        flow.add_step(Box::new(CreateIdentityStep));
        flow.add_step(Box::new(FailingStep));

        flow.advance().unwrap(); // Step 1 completes.
        flow.advance().unwrap(); // Step 2 fails.

        // Identity step NOT rolled back.
        assert_eq!(
            flow.step_status("create_identity"),
            Some(&StepStatus::Completed)
        );
        assert_eq!(flow.context_get("pubkey"), Some(&"abc123".into()));
    }

    #[test]
    fn steps_info() {
        let flow = full_flow();
        let info = flow.steps_info();

        assert_eq!(info.len(), 3);
        assert_eq!(info[0].id, "create_identity");
        assert_eq!(info[0].name, "Create Identity");
        assert!(!info[0].can_skip);
        assert_eq!(info[0].status, StepStatus::Pending);

        assert_eq!(info[1].id, "backup_words");
        assert!(info[1].can_skip);
    }

    #[test]
    fn progress_tracking() {
        let mut flow = full_flow();
        assert!((flow.progress() - 0.0).abs() < 0.001);

        flow.advance().unwrap();
        assert!((flow.progress() - 1.0 / 3.0).abs() < 0.01);

        flow.advance().unwrap();
        assert!((flow.progress() - 2.0 / 3.0).abs() < 0.01);

        flow.advance().unwrap();
        assert!((flow.progress() - 1.0).abs() < 0.001);
    }

    #[test]
    fn context_pre_seeding() {
        let mut flow = ActivationFlow::new();
        flow.add_step(Box::new(ConnectStep));

        // Pre-seed the context with a pubkey.
        flow.context_set("pubkey", "pre_seeded_key");

        let (_, result) = flow.advance().unwrap().unwrap();
        assert_eq!(result, StepResult::Completed);
    }

    #[test]
    fn step_status_lookup() {
        let mut flow = full_flow();
        assert_eq!(
            flow.step_status("create_identity"),
            Some(&StepStatus::Pending)
        );

        flow.advance().unwrap();
        assert_eq!(
            flow.step_status("create_identity"),
            Some(&StepStatus::Completed)
        );

        assert_eq!(flow.step_status("nonexistent"), None);
    }

    #[test]
    fn step_info_serde() {
        let info = StepInfo {
            id: "test".into(),
            name: "Test Step".into(),
            description: "A test".into(),
            can_skip: true,
            status: StepStatus::Completed,
        };
        let json = serde_json::to_string(&info).unwrap();
        let loaded: StepInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, "test");
        assert!(loaded.can_skip);
        assert_eq!(loaded.status, StepStatus::Completed);
    }

    #[test]
    fn flow_config_serde() {
        let config = FlowConfig {
            name: "plexus_full".into(),
            rollback_on_failure: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: FlowConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.name, "plexus_full");
        assert!(loaded.rollback_on_failure);
    }

    #[test]
    fn step_status_is_done() {
        assert!(StepStatus::Completed.is_done());
        assert!(StepStatus::Skipped.is_done());
        assert!(!StepStatus::Pending.is_done());
        assert!(!StepStatus::InProgress.is_done());
        assert!(!StepStatus::Failed("err".into()).is_done());
        assert!(!StepStatus::RolledBack.is_done());
    }

    #[test]
    fn connect_fails_without_identity() {
        let mut flow = ActivationFlow::with_config(FlowConfig {
            rollback_on_failure: false,
            ..Default::default()
        });
        flow.add_step(Box::new(ConnectStep));

        let (_, result) = flow.advance().unwrap().unwrap();
        assert!(matches!(result, StepResult::Failed(_)));
    }

    #[test]
    fn skip_past_end_errors() {
        let mut flow = full_flow();
        flow.advance().unwrap();
        flow.advance().unwrap();
        flow.advance().unwrap();
        // All done — skip should error.
        assert!(flow.skip_current().is_err());
    }
}
