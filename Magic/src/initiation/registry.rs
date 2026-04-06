use std::collections::HashMap;

use crate::error::MagicError;
use crate::ideation::DocumentState;
use super::action::Action;

/// A custom action handler. Receives the document state, returns an Action to execute.
pub type ActionHandler = Box<dyn Fn(&DocumentState) -> Result<Action, MagicError> + Send + Sync>;

/// Registry for named custom actions beyond the 5 built-in types.
pub struct ActionRegistry {
    handlers: HashMap<String, ActionHandler>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: impl Into<String>, handler: ActionHandler) {
        self.handlers.insert(name.into(), handler);
    }

    pub fn get(&self, name: &str) -> Option<&ActionHandler> {
        self.handlers.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.handlers.keys()
    }

    pub fn count(&self) -> usize {
        self.handlers.len()
    }
}

impl Default for ActionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ideas::Digit;
    use x::Value;

    #[test]
    fn register_and_retrieve() {
        let mut reg = ActionRegistry::new();
        reg.register(
            "add-text",
            Box::new(|_state| {
                let d =
                    Digit::new("text".into(), Value::from("auto"), "cpub1test".into()).unwrap();
                Ok(Action::insert(d, None))
            }),
        );
        assert!(reg.contains("add-text"));
        let handler = reg.get("add-text").unwrap();
        let state = DocumentState::new("cpub1test");
        let action = handler(&state).unwrap();
        assert!(matches!(action, Action::InsertDigit { .. }));
    }

    #[test]
    fn get_returns_none_for_unknown() {
        let reg = ActionRegistry::new();
        assert!(reg.get("nope").is_none());
    }

    #[test]
    fn contains_check() {
        let mut reg = ActionRegistry::new();
        assert!(!reg.contains("x"));
        reg.register("x", Box::new(|_| Ok(Action::delete(uuid::Uuid::new_v4()))));
        assert!(reg.contains("x"));
    }

    #[test]
    fn count() {
        let mut reg = ActionRegistry::new();
        assert_eq!(reg.count(), 0);
        reg.register("a", Box::new(|_| Ok(Action::delete(uuid::Uuid::new_v4()))));
        reg.register("b", Box::new(|_| Ok(Action::delete(uuid::Uuid::new_v4()))));
        assert_eq!(reg.count(), 2);
    }

    #[test]
    fn names_iteration() {
        let mut reg = ActionRegistry::new();
        reg.register("alpha", Box::new(|_| Ok(Action::delete(uuid::Uuid::new_v4()))));
        reg.register("beta", Box::new(|_| Ok(Action::delete(uuid::Uuid::new_v4()))));
        let names: Vec<_> = reg.names().collect();
        assert_eq!(names.len(), 2);
    }
}
