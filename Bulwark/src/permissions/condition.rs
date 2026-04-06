use serde::{Deserialize, Serialize};

use super::permission::Permission;

/// A conditional permission — "can do X IF conditions are met."
///
/// Example: "can download IF asset_status == approved"
/// Example: "can view IF watermark == true"
///
/// Conditions are evaluated by the app at check time. Bulwark provides
/// the data structure; the app provides the context values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConditionalPermission {
    /// The underlying permission (action + resource).
    pub permission: Permission,
    /// Conditions that must ALL be met for the permission to apply.
    pub conditions: Vec<Condition>,
}

impl ConditionalPermission {
    pub fn new(permission: Permission) -> Self {
        Self {
            permission,
            conditions: Vec::new(),
        }
    }

    /// Add a condition (builder pattern).
    pub fn with_condition(mut self, condition: Condition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Whether all conditions are satisfied given the context.
    ///
    /// The context is a set of key-value pairs provided by the app at check time.
    /// Each condition specifies a field name, an operator, and an expected value.
    pub fn evaluate(&self, context: &PermissionContext) -> bool {
        self.conditions.iter().all(|c| c.evaluate(context))
    }

    /// Whether this conditional permission grants the requested action + resource,
    /// given the context.
    pub fn allows(
        &self,
        action: &super::permission::Action,
        resource: &super::permission::ResourceScope,
        context: &PermissionContext,
    ) -> bool {
        self.permission.covers(action, resource) && self.evaluate(context)
    }
}

/// A single condition: field + operator + value.
///
/// Fields and values are strings because resource attributes are app-defined.
/// The app knows what "asset_status" means; Bulwark just checks the condition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Condition {
    /// The field name to check (e.g., "asset_status", "watermark").
    pub field: String,
    /// The comparison operator.
    pub operator: ConditionOp,
    /// The expected value.
    pub value: String,
}

impl Condition {
    pub fn new(field: impl Into<String>, operator: ConditionOp, value: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            operator,
            value: value.into(),
        }
    }

    /// Convenience: field == value.
    pub fn equals(field: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(field, ConditionOp::Equals, value)
    }

    /// Convenience: field != value.
    pub fn not_equals(field: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(field, ConditionOp::NotEquals, value)
    }

    /// Evaluate this condition against a context.
    pub fn evaluate(&self, context: &PermissionContext) -> bool {
        match context.get(&self.field) {
            None => false, // missing field = condition not met
            Some(actual) => self.operator.compare(actual, &self.value),
        }
    }
}

/// Comparison operators for conditions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ConditionOp {
    /// Field equals value.
    Equals,
    /// Field does not equal value.
    NotEquals,
    /// Field is in a comma-separated list of values.
    In,
    /// Field is not in a comma-separated list of values.
    NotIn,
    /// Field exists (value is ignored in comparison).
    Exists,
}

impl ConditionOp {
    /// Compare an actual value against an expected value using this operator.
    pub fn compare(&self, actual: &str, expected: &str) -> bool {
        match self {
            ConditionOp::Equals => actual == expected,
            ConditionOp::NotEquals => actual != expected,
            ConditionOp::In => expected.split(',').any(|v| v.trim() == actual),
            ConditionOp::NotIn => !expected.split(',').any(|v| v.trim() == actual),
            ConditionOp::Exists => true, // field was found, that's enough
        }
    }
}

/// Context values provided by the app at permission check time.
///
/// Simple key-value pairs. The app fills these in with resource attributes
/// (e.g., `{"asset_status": "approved", "watermark": "true"}`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PermissionContext {
    pub values: std::collections::HashMap<String, String>,
}

impl PermissionContext {
    pub fn new() -> Self {
        Self {
            values: std::collections::HashMap::new(),
        }
    }

    /// Set a context value.
    pub fn set(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.insert(key.into(), value.into());
        self
    }

    /// Get a context value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::permission::{Action, Permission, ResourceScope};

    #[test]
    fn condition_equals() {
        let cond = Condition::equals("status", "approved");
        let ctx = PermissionContext::new().set("status", "approved");
        assert!(cond.evaluate(&ctx));

        let ctx_wrong = PermissionContext::new().set("status", "draft");
        assert!(!cond.evaluate(&ctx_wrong));
    }

    #[test]
    fn condition_not_equals() {
        let cond = Condition::not_equals("status", "draft");
        let ctx = PermissionContext::new().set("status", "approved");
        assert!(cond.evaluate(&ctx));

        let ctx_draft = PermissionContext::new().set("status", "draft");
        assert!(!cond.evaluate(&ctx_draft));
    }

    #[test]
    fn condition_in() {
        let cond = Condition::new("role", ConditionOp::In, "designer, reviewer, admin");
        let ctx = PermissionContext::new().set("role", "reviewer");
        assert!(cond.evaluate(&ctx));

        let ctx_other = PermissionContext::new().set("role", "external");
        assert!(!cond.evaluate(&ctx_other));
    }

    #[test]
    fn condition_not_in() {
        let cond = Condition::new("role", ConditionOp::NotIn, "banned, suspended");
        let ctx = PermissionContext::new().set("role", "member");
        assert!(cond.evaluate(&ctx));

        let ctx_banned = PermissionContext::new().set("role", "banned");
        assert!(!cond.evaluate(&ctx_banned));
    }

    #[test]
    fn condition_exists() {
        let cond = Condition::new("watermark", ConditionOp::Exists, "");
        let ctx = PermissionContext::new().set("watermark", "true");
        assert!(cond.evaluate(&ctx));

        let ctx_missing = PermissionContext::new();
        assert!(!cond.evaluate(&ctx_missing));
    }

    #[test]
    fn missing_field_fails_condition() {
        let cond = Condition::equals("status", "approved");
        let ctx = PermissionContext::new(); // no "status" key
        assert!(!cond.evaluate(&ctx));
    }

    #[test]
    fn conditional_permission_all_conditions_must_pass() {
        let cp = ConditionalPermission::new(Permission::from_strings("download", "brand.logo"))
            .with_condition(Condition::equals("asset_status", "approved"))
            .with_condition(Condition::not_equals("format", "raw"));

        // Both conditions met
        let ctx = PermissionContext::new()
            .set("asset_status", "approved")
            .set("format", "png");
        assert!(cp.evaluate(&ctx));

        // First fails
        let ctx_draft = PermissionContext::new()
            .set("asset_status", "draft")
            .set("format", "png");
        assert!(!cp.evaluate(&ctx_draft));

        // Second fails
        let ctx_raw = PermissionContext::new()
            .set("asset_status", "approved")
            .set("format", "raw");
        assert!(!cp.evaluate(&ctx_raw));
    }

    #[test]
    fn conditional_permission_no_conditions_always_passes() {
        let cp = ConditionalPermission::new(Permission::from_strings("view", "brand"));
        let ctx = PermissionContext::new();
        assert!(cp.evaluate(&ctx)); // no conditions = always true
    }

    #[test]
    fn conditional_permission_allows() {
        let cp = ConditionalPermission::new(Permission::from_strings("download", "brand"))
            .with_condition(Condition::equals("status", "approved"));

        let ctx = PermissionContext::new().set("status", "approved");

        // Right action + resource + conditions
        assert!(cp.allows(&Action::download(), &ResourceScope::new("brand.logo"), &ctx));

        // Wrong action
        assert!(!cp.allows(&Action::upload(), &ResourceScope::new("brand.logo"), &ctx));

        // Wrong context
        let ctx_bad = PermissionContext::new().set("status", "draft");
        assert!(!cp.allows(&Action::download(), &ResourceScope::new("brand.logo"), &ctx_bad));
    }

    #[test]
    fn permission_context_builder() {
        let ctx = PermissionContext::new()
            .set("a", "1")
            .set("b", "2")
            .set("c", "3");
        assert_eq!(ctx.get("a"), Some("1"));
        assert_eq!(ctx.get("b"), Some("2"));
        assert_eq!(ctx.get("c"), Some("3"));
        assert_eq!(ctx.get("d"), None);
    }
}
