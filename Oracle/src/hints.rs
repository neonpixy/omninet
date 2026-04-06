//! Contextual hints — guidance that appears when needed, disappears when not.
//!
//! Any crate can register hints via the `OracleHint` trait. Oracle evaluates
//! them against user context and presents only what's relevant. Dismissed
//! hints stay dismissed.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Context about the user's current state. Passed to hints for evaluation.
///
/// Keys are free-form strings. Common keys:
/// - `"has_backup"` — "true" / "false"
/// - `"follower_count"` — "0", "5", etc.
/// - `"community_count"` — number of communities joined
/// - `"app"` — current app name
/// - `"view"` — current view/screen
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HintContext {
    /// Key-value pairs describing the user's state.
    pub values: HashMap<String, String>,
}

impl HintContext {
    /// Create an empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a context value.
    pub fn set(&mut self, key: &str, value: &str) -> &mut Self {
        self.values.insert(key.into(), value.into());
        self
    }

    /// Get a context value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    /// Check if a boolean context value is true.
    pub fn is_true(&self, key: &str) -> bool {
        self.values.get(key).is_some_and(|v| v == "true")
    }
}

/// A hint's priority (higher = more important, shown first).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum HintPriority {
    /// Nice to know. Shown when nothing more important is pending.
    Low = 0,
    /// Helpful guidance. Most hints are this level.
    Medium = 1,
    /// Important action needed (e.g., "backup your recovery words").
    High = 2,
    /// Critical safety issue (e.g., "your account may be compromised").
    Critical = 3,
}

/// What happens when the user acts on a hint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HintAction {
    /// Navigate to a specific view.
    Navigate { app: String, view: String },
    /// Open a URL (e.g., documentation).
    OpenUrl(String),
    /// Dismiss (no action needed — informational hint).
    Dismiss,
    /// Custom action identified by a string key.
    Custom(String),
}

/// A contextual guidance hint. Any crate can implement this trait.
///
/// Oracle evaluates all registered hints against the user's context
/// each time the UI requests hints.
pub trait OracleHint: Send + Sync {
    /// Unique identifier for this hint.
    fn id(&self) -> &str;

    /// Whether this hint should be shown given the current context.
    fn should_show(&self, context: &HintContext) -> bool;

    /// The hint message (human-readable).
    fn message(&self) -> &str;

    /// What happens when the user acts on this hint.
    fn action(&self) -> HintAction;

    /// Priority level.
    fn priority(&self) -> HintPriority {
        HintPriority::Medium
    }
}

/// A static hint (data-driven, no trait object needed).
///
/// For simple hints that don't need custom `should_show` logic,
/// use a `StaticHint` with a `required_context` map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaticHint {
    /// Unique identifier.
    pub id: String,
    /// The message to show.
    pub message: String,
    /// Priority.
    pub priority: HintPriority,
    /// Action when tapped.
    pub action: HintAction,
    /// Context requirements: all must match for the hint to show.
    /// Key → expected value. If the context value doesn't match, hint is hidden.
    pub required_context: HashMap<String, String>,
}

impl OracleHint for StaticHint {
    fn id(&self) -> &str { &self.id }
    fn message(&self) -> &str { &self.message }
    fn action(&self) -> HintAction { self.action.clone() }
    fn priority(&self) -> HintPriority { self.priority }

    fn should_show(&self, context: &HintContext) -> bool {
        self.required_context.iter().all(|(key, expected)| {
            context.get(key) == Some(expected.as_str())
        })
    }
}

/// A hint ready to be displayed (evaluated and filtered).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActiveHint {
    /// The hint's ID.
    pub id: String,
    /// The message.
    pub message: String,
    /// Priority.
    pub priority: HintPriority,
    /// Action.
    pub action: HintAction,
}

/// Evaluates registered hints against user context.
///
/// Tracks dismissed hints so they don't reappear.
pub struct HintEngine {
    /// Registered hints.
    hints: Vec<Box<dyn OracleHint>>,
    /// IDs of dismissed hints.
    dismissed: HashSet<String>,
}

impl HintEngine {
    /// Create a new hint engine.
    pub fn new() -> Self {
        Self {
            hints: Vec::new(),
            dismissed: HashSet::new(),
        }
    }

    /// Register a hint.
    pub fn register(&mut self, hint: Box<dyn OracleHint>) {
        self.hints.push(hint);
    }

    /// Register a static (data-driven) hint.
    pub fn register_static(&mut self, hint: StaticHint) {
        self.hints.push(Box::new(hint));
    }

    /// Evaluate all hints against the current context.
    ///
    /// Returns hints that should be shown, sorted by priority (highest first).
    /// Dismissed hints are excluded.
    pub fn evaluate(&self, context: &HintContext) -> Vec<ActiveHint> {
        let mut active: Vec<ActiveHint> = self
            .hints
            .iter()
            .filter(|h| {
                !self.dismissed.contains(h.id()) && h.should_show(context)
            })
            .map(|h| ActiveHint {
                id: h.id().to_string(),
                message: h.message().to_string(),
                priority: h.priority(),
                action: h.action(),
            })
            .collect();

        // Sort by priority (highest first).
        active.sort_by(|a, b| b.priority.cmp(&a.priority));
        active
    }

    /// Dismiss a hint (it won't show again).
    pub fn dismiss(&mut self, hint_id: &str) {
        self.dismissed.insert(hint_id.into());
    }

    /// Un-dismiss a hint (it can show again).
    pub fn undismiss(&mut self, hint_id: &str) {
        self.dismissed.remove(hint_id);
    }

    /// Whether a hint is dismissed.
    pub fn is_dismissed(&self, hint_id: &str) -> bool {
        self.dismissed.contains(hint_id)
    }

    /// Number of registered hints.
    pub fn hint_count(&self) -> usize {
        self.hints.len()
    }

    /// All dismissed hint IDs.
    pub fn dismissed_ids(&self) -> Vec<String> {
        self.dismissed.iter().cloned().collect()
    }

    /// Restore dismissed state (from persistence).
    pub fn restore_dismissed(&mut self, ids: &[String]) {
        for id in ids {
            self.dismissed.insert(id.clone());
        }
    }
}

impl Default for HintEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backup_hint() -> StaticHint {
        StaticHint {
            id: "backup_reminder".into(),
            message: "Your recovery words aren't backed up".into(),
            priority: HintPriority::High,
            action: HintAction::Navigate {
                app: "omny".into(),
                view: "backup".into(),
            },
            required_context: {
                let mut m = HashMap::new();
                m.insert("has_backup".into(), "false".into());
                m
            },
        }
    }

    fn community_hint() -> StaticHint {
        StaticHint {
            id: "explore_communities".into(),
            message: "You have no followers yet — want to explore communities?".into(),
            priority: HintPriority::Medium,
            action: HintAction::Navigate {
                app: "plexus".into(),
                view: "discover".into(),
            },
            required_context: {
                let mut m = HashMap::new();
                m.insert("follower_count".into(), "0".into());
                m
            },
        }
    }

    struct DynamicHint;
    impl OracleHint for DynamicHint {
        fn id(&self) -> &str { "dynamic_test" }
        fn message(&self) -> &str { "Dynamic hint based on custom logic" }
        fn action(&self) -> HintAction { HintAction::Dismiss }
        fn priority(&self) -> HintPriority { HintPriority::Low }
        fn should_show(&self, context: &HintContext) -> bool {
            // Show only when community count > 0.
            context.get("community_count")
                .and_then(|v| v.parse::<u32>().ok())
                .is_some_and(|c| c > 0)
        }
    }

    #[test]
    fn context_basics() {
        let mut ctx = HintContext::new();
        ctx.set("has_backup", "false");
        ctx.set("follower_count", "0");

        assert_eq!(ctx.get("has_backup"), Some("false"));
        assert!(!ctx.is_true("has_backup"));

        ctx.set("has_backup", "true");
        assert!(ctx.is_true("has_backup"));

        assert_eq!(ctx.get("nonexistent"), None);
    }

    #[test]
    fn static_hint_shows_when_context_matches() {
        let hint = backup_hint();
        let mut ctx = HintContext::new();
        ctx.set("has_backup", "false");
        assert!(hint.should_show(&ctx));
    }

    #[test]
    fn static_hint_hidden_when_context_doesnt_match() {
        let hint = backup_hint();
        let mut ctx = HintContext::new();
        ctx.set("has_backup", "true");
        assert!(!hint.should_show(&ctx));
    }

    #[test]
    fn static_hint_hidden_when_context_missing() {
        let hint = backup_hint();
        let ctx = HintContext::new(); // No "has_backup" key.
        assert!(!hint.should_show(&ctx));
    }

    #[test]
    fn engine_evaluates_hints() {
        let mut engine = HintEngine::new();
        engine.register_static(backup_hint());
        engine.register_static(community_hint());

        let mut ctx = HintContext::new();
        ctx.set("has_backup", "false");
        ctx.set("follower_count", "0");

        let active = engine.evaluate(&ctx);
        assert_eq!(active.len(), 2);
        // High priority first.
        assert_eq!(active[0].id, "backup_reminder");
        assert_eq!(active[0].priority, HintPriority::High);
        assert_eq!(active[1].id, "explore_communities");
    }

    #[test]
    fn engine_filters_non_matching() {
        let mut engine = HintEngine::new();
        engine.register_static(backup_hint());

        let mut ctx = HintContext::new();
        ctx.set("has_backup", "true"); // Backup done — hint shouldn't show.

        let active = engine.evaluate(&ctx);
        assert!(active.is_empty());
    }

    #[test]
    fn dismiss_and_undismiss() {
        let mut engine = HintEngine::new();
        engine.register_static(backup_hint());

        let mut ctx = HintContext::new();
        ctx.set("has_backup", "false");

        assert_eq!(engine.evaluate(&ctx).len(), 1);

        engine.dismiss("backup_reminder");
        assert!(engine.is_dismissed("backup_reminder"));
        assert!(engine.evaluate(&ctx).is_empty());

        engine.undismiss("backup_reminder");
        assert!(!engine.is_dismissed("backup_reminder"));
        assert_eq!(engine.evaluate(&ctx).len(), 1);
    }

    #[test]
    fn dynamic_hint() {
        let mut engine = HintEngine::new();
        engine.register(Box::new(DynamicHint));

        let mut ctx = HintContext::new();
        ctx.set("community_count", "0");
        assert!(engine.evaluate(&ctx).is_empty());

        ctx.set("community_count", "3");
        assert_eq!(engine.evaluate(&ctx).len(), 1);
    }

    #[test]
    fn restore_dismissed() {
        let mut engine = HintEngine::new();
        engine.register_static(backup_hint());
        engine.register_static(community_hint());

        engine.restore_dismissed(&["backup_reminder".into()]);
        assert!(engine.is_dismissed("backup_reminder"));
        assert!(!engine.is_dismissed("explore_communities"));
    }

    #[test]
    fn hint_priority_ordering() {
        assert!(HintPriority::Critical > HintPriority::High);
        assert!(HintPriority::High > HintPriority::Medium);
        assert!(HintPriority::Medium > HintPriority::Low);
    }

    #[test]
    fn hint_action_serde() {
        let action = HintAction::Navigate {
            app: "plexus".into(),
            view: "discover".into(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let loaded: HintAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, loaded);
    }

    #[test]
    fn active_hint_serde() {
        let hint = ActiveHint {
            id: "test".into(),
            message: "Hello".into(),
            priority: HintPriority::High,
            action: HintAction::Dismiss,
        };
        let json = serde_json::to_string(&hint).unwrap();
        let loaded: ActiveHint = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, "test");
        assert_eq!(loaded.priority, HintPriority::High);
    }

    #[test]
    fn static_hint_serde() {
        let hint = backup_hint();
        let json = serde_json::to_string(&hint).unwrap();
        let loaded: StaticHint = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, "backup_reminder");
        assert_eq!(loaded.priority, HintPriority::High);
    }

    #[test]
    fn engine_hint_count() {
        let mut engine = HintEngine::new();
        assert_eq!(engine.hint_count(), 0);
        engine.register_static(backup_hint());
        assert_eq!(engine.hint_count(), 1);
        engine.register(Box::new(DynamicHint));
        assert_eq!(engine.hint_count(), 2);
    }

    #[test]
    fn dismissed_ids() {
        let mut engine = HintEngine::new();
        engine.dismiss("a");
        engine.dismiss("b");
        let mut ids = engine.dismissed_ids();
        ids.sort();
        assert_eq!(ids, vec!["a", "b"]);
    }
}
