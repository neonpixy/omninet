use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A request to execute a skill.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillCall {
    /// Call ID (unique per invocation)
    pub id: String,
    /// Which skill to invoke
    pub skill_id: String,
    /// Arguments as key-value pairs
    pub arguments: HashMap<String, serde_json::Value>,
}

impl SkillCall {
    pub fn new(id: impl Into<String>, skill_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            skill_id: skill_id.into(),
            arguments: HashMap::new(),
        }
    }

    pub fn with_argument(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.arguments.insert(key.into(), value.into());
        self
    }

    /// Extract a string argument by key.
    pub fn get_string(&self, key: &str) -> Result<String, crate::AdvisorError> {
        self.arguments
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| crate::AdvisorError::InvalidSkillParameters {
                id: self.skill_id.clone(),
                reason: format!("missing or non-string parameter: {key}"),
            })
    }

    /// Extract a numeric argument by key.
    pub fn get_number(&self, key: &str) -> Result<f64, crate::AdvisorError> {
        self.arguments
            .get(key)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| crate::AdvisorError::InvalidSkillParameters {
                id: self.skill_id.clone(),
                reason: format!("missing or non-number parameter: {key}"),
            })
    }

    /// Extract an optional string argument by key.
    pub fn get_string_opt(&self, key: &str) -> Option<String> {
        self.arguments
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Extract an optional numeric argument by key.
    pub fn get_number_opt(&self, key: &str) -> Option<f64> {
        self.arguments.get(key).and_then(|v| v.as_f64())
    }

    /// Extract the arguments as a serde_json::Value map (for forwarding).
    pub fn arguments_as_value(&self) -> serde_json::Value {
        serde_json::to_value(&self.arguments).unwrap_or(serde_json::Value::Null)
    }
}

/// The result of executing a skill.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillResult {
    pub success: bool,
    pub output: String,
    pub data: HashMap<String, String>,
}

impl SkillResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            data: HashMap::new(),
        }
    }

    pub fn failure(output: impl Into<String>) -> Self {
        Self {
            success: false,
            output: output.into(),
            data: HashMap::new(),
        }
    }

    pub fn with_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }
}

/// Whether a skill execution was approved, rejected, or needs approval.
///
/// Covenant: consent required for actions that affect others.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SkillValidationResult {
    /// Approved, with optional disclosures to show the user
    Approved(Vec<String>),
    /// Rejected with reason
    Rejected(String),
    /// Needs human approval before proceeding
    NeedsApproval(String),
}

impl SkillValidationResult {
    pub fn is_approved(&self) -> bool {
        matches!(self, SkillValidationResult::Approved(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_call_builder() {
        let call = SkillCall::new("call-1", "web.search")
            .with_argument("query", serde_json::Value::String("rust lang".into()))
            .with_argument("limit", serde_json::json!(10));
        assert_eq!(call.skill_id, "web.search");
        assert_eq!(call.arguments.len(), 2);
    }

    #[test]
    fn skill_result_success() {
        let result = SkillResult::success("found 3 results")
            .with_data("count", "3")
            .with_data("source", "web");
        assert!(result.success);
        assert_eq!(result.data.len(), 2);
    }

    #[test]
    fn skill_result_failure() {
        let result = SkillResult::failure("network timeout");
        assert!(!result.success);
    }

    #[test]
    fn skill_validation() {
        let approved = SkillValidationResult::Approved(vec!["will send email".into()]);
        assert!(approved.is_approved());

        let rejected = SkillValidationResult::Rejected("unauthorized".into());
        assert!(!rejected.is_approved());

        let needs = SkillValidationResult::NeedsApproval("send message?".into());
        assert!(!needs.is_approved());
    }

    #[test]
    fn skill_call_serialization_roundtrip() {
        let call = SkillCall::new("c1", "test")
            .with_argument("key", serde_json::Value::String("val".into()));
        let json = serde_json::to_string(&call).unwrap();
        let deserialized: SkillCall = serde_json::from_str(&json).unwrap();
        assert_eq!(call, deserialized);
    }

    #[test]
    fn get_string_extracts_value() {
        let call = SkillCall::new("c1", "test")
            .with_argument("name", serde_json::Value::String("Alice".into()));
        assert_eq!(call.get_string("name").unwrap(), "Alice");
    }

    #[test]
    fn get_string_missing_key_fails() {
        let call = SkillCall::new("c1", "test");
        assert!(call.get_string("missing").is_err());
    }

    #[test]
    fn get_number_extracts_value() {
        let call = SkillCall::new("c1", "test")
            .with_argument("count", serde_json::json!(42));
        assert!((call.get_number("count").unwrap() - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn get_number_missing_key_fails() {
        let call = SkillCall::new("c1", "test");
        assert!(call.get_number("missing").is_err());
    }

    #[test]
    fn get_string_opt_returns_none_for_missing() {
        let call = SkillCall::new("c1", "test");
        assert!(call.get_string_opt("missing").is_none());
    }

    #[test]
    fn get_number_opt_returns_none_for_missing() {
        let call = SkillCall::new("c1", "test");
        assert!(call.get_number_opt("missing").is_none());
    }

    #[test]
    fn arguments_as_value_returns_object() {
        let call = SkillCall::new("c1", "test")
            .with_argument("key", serde_json::Value::String("val".into()));
        let val = call.arguments_as_value();
        assert!(val.is_object());
    }
}
