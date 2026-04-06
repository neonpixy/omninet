//! Recovery flow — restore identity on a new device.
//!
//! "Welcome back" → enter 12 words → Crown restored → Omnibus connects →
//! Harbor syncs content. Feels like signing into iCloud.
//!
//! Recovery methods implement `RecoveryMethod`. Launch with BIP-39 words.
//! Future: social recovery (trusted contacts), hardware key, platform
//! keychain sync. New methods register without changing Oracle.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::OracleError;

/// The result of a recovery attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryResult {
    /// Identity restored successfully.
    Restored {
        /// The recovered public key.
        pubkey: String,
    },
    /// Recovery failed.
    Failed(String),
    /// Recovery requires additional input (e.g., "enter word 7").
    NeedsInput(String),
}

/// Current status of the recovery flow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryStatus {
    /// Not started.
    Idle,
    /// Waiting for user input (method-specific).
    AwaitingInput,
    /// Verifying recovery material.
    Verifying,
    /// Identity restored, syncing content.
    Syncing,
    /// Recovery complete.
    Complete,
    /// Recovery failed.
    Failed(String),
}

/// A pluggable identity recovery strategy.
///
/// BIP-39 is the launch method. Social recovery, hardware keys,
/// and platform keychain sync are future implementations.
pub trait RecoveryMethod: Send + Sync {
    /// Unique identifier for this method (e.g., "bip39", "social", "hardware_key").
    fn id(&self) -> &str;

    /// Human-readable name.
    fn name(&self) -> &str;

    /// Description of what the user needs to do.
    fn instructions(&self) -> &str;

    /// Attempt recovery with the given input.
    ///
    /// For BIP-39: input contains `"words" → "abandon abandon ... zoo"`.
    /// For social recovery: input contains `"vouchers" → "pubkey1,pubkey2,..."`.
    fn recover(&self, input: &HashMap<String, String>) -> RecoveryResult;
}

/// Orchestrates the recovery process.
///
/// Holds registered recovery methods and manages the flow state.
pub struct RecoveryFlow {
    /// Available recovery methods.
    methods: Vec<Box<dyn RecoveryMethod>>,
    /// Current status.
    status: RecoveryStatus,
    /// The method being used (index into `methods`).
    active_method: Option<usize>,
    /// Recovered pubkey (set on success).
    recovered_pubkey: Option<String>,
}

impl RecoveryFlow {
    /// Create a new recovery flow.
    pub fn new() -> Self {
        Self {
            methods: Vec::new(),
            status: RecoveryStatus::Idle,
            active_method: None,
            recovered_pubkey: None,
        }
    }

    /// Register a recovery method.
    pub fn register(&mut self, method: Box<dyn RecoveryMethod>) {
        self.methods.push(method);
    }

    /// List available recovery methods.
    pub fn available_methods(&self) -> Vec<MethodInfo> {
        self.methods
            .iter()
            .enumerate()
            .map(|(i, m)| MethodInfo {
                index: i,
                id: m.id().to_string(),
                name: m.name().to_string(),
                instructions: m.instructions().to_string(),
            })
            .collect()
    }

    /// Select a recovery method by index.
    pub fn select_method(&mut self, index: usize) -> Result<(), OracleError> {
        if index >= self.methods.len() {
            return Err(OracleError::RecoveryFailed(format!(
                "method index {index} out of range (have {})",
                self.methods.len()
            )));
        }
        self.active_method = Some(index);
        self.status = RecoveryStatus::AwaitingInput;
        Ok(())
    }

    /// Attempt recovery with the active method and provided input.
    pub fn attempt(
        &mut self,
        input: &HashMap<String, String>,
    ) -> Result<RecoveryResult, OracleError> {
        let index = self.active_method.ok_or_else(|| {
            OracleError::RecoveryFailed("no method selected".into())
        })?;

        self.status = RecoveryStatus::Verifying;
        let result = self.methods[index].recover(input);

        match &result {
            RecoveryResult::Restored { pubkey } => {
                self.recovered_pubkey = Some(pubkey.clone());
                self.status = RecoveryStatus::Complete;
            }
            RecoveryResult::Failed(reason) => {
                self.status = RecoveryStatus::Failed(reason.clone());
            }
            RecoveryResult::NeedsInput(_) => {
                self.status = RecoveryStatus::AwaitingInput;
            }
        }

        Ok(result)
    }

    /// Current status.
    pub fn status(&self) -> &RecoveryStatus {
        &self.status
    }

    /// The recovered public key (if recovery succeeded).
    pub fn recovered_pubkey(&self) -> Option<&str> {
        self.recovered_pubkey.as_deref()
    }

    /// Reset the flow to idle state.
    pub fn reset(&mut self) {
        self.status = RecoveryStatus::Idle;
        self.active_method = None;
        self.recovered_pubkey = None;
    }

    /// Number of registered methods.
    pub fn method_count(&self) -> usize {
        self.methods.len()
    }
}

impl Default for RecoveryFlow {
    fn default() -> Self {
        Self::new()
    }
}

/// Info about a recovery method (for UI display).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MethodInfo {
    /// Index in the method list (for selection).
    pub index: usize,
    /// Method identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// User instructions.
    pub instructions: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Bip39Method;
    impl RecoveryMethod for Bip39Method {
        fn id(&self) -> &str { "bip39" }
        fn name(&self) -> &str { "Recovery Words" }
        fn instructions(&self) -> &str { "Enter your 12 recovery words" }

        fn recover(&self, input: &HashMap<String, String>) -> RecoveryResult {
            match input.get("words") {
                Some(words) => {
                    let count = words.split_whitespace().count();
                    if count == 12 {
                        RecoveryResult::Restored {
                            pubkey: "recovered_pubkey_abc".into(),
                        }
                    } else {
                        RecoveryResult::Failed(format!(
                            "expected 12 words, got {count}"
                        ))
                    }
                }
                None => RecoveryResult::NeedsInput(
                    "Please provide your 12 recovery words".into(),
                ),
            }
        }
    }

    struct SocialMethod;
    impl RecoveryMethod for SocialMethod {
        fn id(&self) -> &str { "social" }
        fn name(&self) -> &str { "Trusted Contacts" }
        fn instructions(&self) -> &str { "Ask 3 trusted contacts to vouch for you" }

        fn recover(&self, input: &HashMap<String, String>) -> RecoveryResult {
            match input.get("vouchers") {
                Some(v) => {
                    let count = v.split(',').count();
                    if count >= 3 {
                        RecoveryResult::Restored {
                            pubkey: "social_recovered_key".into(),
                        }
                    } else {
                        RecoveryResult::Failed(format!(
                            "need 3 vouchers, got {count}"
                        ))
                    }
                }
                None => RecoveryResult::NeedsInput(
                    "Provide voucher pubkeys".into(),
                ),
            }
        }
    }

    #[test]
    fn empty_flow() {
        let flow = RecoveryFlow::new();
        assert_eq!(flow.method_count(), 0);
        assert_eq!(*flow.status(), RecoveryStatus::Idle);
        assert!(flow.recovered_pubkey().is_none());
    }

    #[test]
    fn register_methods() {
        let mut flow = RecoveryFlow::new();
        flow.register(Box::new(Bip39Method));
        flow.register(Box::new(SocialMethod));

        assert_eq!(flow.method_count(), 2);
        let methods = flow.available_methods();
        assert_eq!(methods[0].id, "bip39");
        assert_eq!(methods[1].id, "social");
    }

    #[test]
    fn successful_bip39_recovery() {
        let mut flow = RecoveryFlow::new();
        flow.register(Box::new(Bip39Method));

        flow.select_method(0).unwrap();
        assert_eq!(*flow.status(), RecoveryStatus::AwaitingInput);

        let mut input = HashMap::new();
        input.insert(
            "words".into(),
            "one two three four five six seven eight nine ten eleven twelve".into(),
        );

        let result = flow.attempt(&input).unwrap();
        assert!(matches!(result, RecoveryResult::Restored { .. }));
        assert_eq!(*flow.status(), RecoveryStatus::Complete);
        assert_eq!(flow.recovered_pubkey(), Some("recovered_pubkey_abc"));
    }

    #[test]
    fn failed_recovery_wrong_word_count() {
        let mut flow = RecoveryFlow::new();
        flow.register(Box::new(Bip39Method));
        flow.select_method(0).unwrap();

        let mut input = HashMap::new();
        input.insert("words".into(), "one two three".into());

        let result = flow.attempt(&input).unwrap();
        assert!(matches!(result, RecoveryResult::Failed(_)));
        assert!(matches!(flow.status(), RecoveryStatus::Failed(_)));
    }

    #[test]
    fn needs_input() {
        let mut flow = RecoveryFlow::new();
        flow.register(Box::new(Bip39Method));
        flow.select_method(0).unwrap();

        let input = HashMap::new(); // No "words" key.
        let result = flow.attempt(&input).unwrap();
        assert!(matches!(result, RecoveryResult::NeedsInput(_)));
        assert_eq!(*flow.status(), RecoveryStatus::AwaitingInput);
    }

    #[test]
    fn select_invalid_method_errors() {
        let mut flow = RecoveryFlow::new();
        flow.register(Box::new(Bip39Method));

        assert!(flow.select_method(5).is_err());
    }

    #[test]
    fn attempt_without_selection_errors() {
        let mut flow = RecoveryFlow::new();
        flow.register(Box::new(Bip39Method));

        let input = HashMap::new();
        assert!(flow.attempt(&input).is_err());
    }

    #[test]
    fn social_recovery() {
        let mut flow = RecoveryFlow::new();
        flow.register(Box::new(SocialMethod));
        flow.select_method(0).unwrap();

        let mut input = HashMap::new();
        input.insert("vouchers".into(), "alice,bob,carol".into());

        let result = flow.attempt(&input).unwrap();
        assert!(matches!(result, RecoveryResult::Restored { .. }));
        assert_eq!(flow.recovered_pubkey(), Some("social_recovered_key"));
    }

    #[test]
    fn reset_flow() {
        let mut flow = RecoveryFlow::new();
        flow.register(Box::new(Bip39Method));
        flow.select_method(0).unwrap();

        let mut input = HashMap::new();
        input.insert(
            "words".into(),
            "one two three four five six seven eight nine ten eleven twelve".into(),
        );
        flow.attempt(&input).unwrap();
        assert_eq!(*flow.status(), RecoveryStatus::Complete);

        flow.reset();
        assert_eq!(*flow.status(), RecoveryStatus::Idle);
        assert!(flow.recovered_pubkey().is_none());
        assert!(flow.available_methods().len() == 1); // Methods still registered.
    }

    #[test]
    fn method_info_serde() {
        let info = MethodInfo {
            index: 0,
            id: "bip39".into(),
            name: "Recovery Words".into(),
            instructions: "Enter words".into(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let loaded: MethodInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, "bip39");
    }

    #[test]
    fn recovery_status_serde() {
        let status = RecoveryStatus::Failed("bad words".into());
        let json = serde_json::to_string(&status).unwrap();
        let loaded: RecoveryStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, loaded);
    }

    #[test]
    fn recovery_result_serde() {
        let result = RecoveryResult::Restored {
            pubkey: "abc".into(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let loaded: RecoveryResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, loaded);
    }
}
