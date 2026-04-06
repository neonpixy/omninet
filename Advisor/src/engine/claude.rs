use serde::{Deserialize, Serialize};

use super::capabilities::ProviderCapabilities;
#[cfg(test)]
use super::context::ConversationMessage;
use super::context::{FinishReason, GenerationContext, GenerationResult, MessageRole};
use super::provider::{CognitiveProvider, ProviderStatus};
use crate::skill::call::SkillCall;
use crate::skill::definition::SkillDefinition;

// ── ClaudeProvider ──────────────────────────────────────────────

/// Claude API provider — describes the Anthropic Claude backend.
///
/// This is a descriptive provider: it declares what Claude IS (capabilities,
/// status, identity). It does NOT make HTTP calls. The platform layer
/// reads `ClaudeRequest`, makes the call, and feeds back `ClaudeResponse`.
#[derive(Debug, Clone)]
pub struct ClaudeProvider {
    /// Model identifier (e.g., "claude-sonnet-4-6")
    model: String,
    /// Whether the API key has been configured
    api_key_configured: bool,
}

impl ClaudeProvider {
    /// Create a new Claude provider.
    ///
    /// # Examples
    /// ```
    /// use advisor::engine::claude::ClaudeProvider;
    /// use advisor::CognitiveProvider;
    ///
    /// let provider = ClaudeProvider::new("claude-sonnet-4-6", true);
    /// assert!(provider.status().is_available());
    /// ```
    pub fn new(model: impl Into<String>, api_key_configured: bool) -> Self {
        Self {
            model: model.into(),
            api_key_configured,
        }
    }

    /// The model this provider targets.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Update the API key configured status.
    pub fn set_api_key_configured(&mut self, configured: bool) {
        self.api_key_configured = configured;
    }
}

impl CognitiveProvider for ClaudeProvider {
    fn id(&self) -> &str {
        "anthropic.claude"
    }

    fn display_name(&self) -> &str {
        "Claude"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::STREAMING
            | ProviderCapabilities::TOOL_CALLING
            | ProviderCapabilities::LARGE_CONTEXT
            | ProviderCapabilities::STRUCTURED_OUTPUT
    }

    fn status(&self) -> ProviderStatus {
        if self.api_key_configured {
            ProviderStatus::Available
        } else {
            ProviderStatus::RequiresSetup {
                message: "Anthropic API key not configured".into(),
            }
        }
    }

    fn is_cloud(&self) -> bool {
        true
    }
}

// ── Request/Response Format Types ───────────────────────────────

/// Canonical Claude API request format.
///
/// The platform layer reads this struct and makes the HTTP call.
/// Fields match the Anthropic Messages API so the platform layer
/// can serialize this directly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeRequest {
    /// Model identifier (e.g., "claude-sonnet-4-6")
    pub model: String,
    /// Maximum tokens to generate
    pub max_tokens: u32,
    /// System prompt (separate from messages per Anthropic API)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Conversation messages
    pub messages: Vec<ClaudeMessage>,
    /// Sampling temperature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Tool definitions available for this request
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ClaudeTool>,
}

/// A single message in the Claude API format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeMessage {
    /// "user" or "assistant" (system is separate in Claude API)
    pub role: String,
    /// Message content — text or structured blocks
    pub content: ClaudeContent,
}

/// Content of a Claude message — either plain text or structured blocks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ClaudeContent {
    /// Simple text content
    Text(String),
    /// Structured content blocks (tool use, tool results, etc.)
    Blocks(Vec<ClaudeContentBlock>),
}

/// A single content block within a Claude message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ClaudeContentBlock {
    /// A text block
    #[serde(rename = "text")]
    Text { text: String },
    /// A tool use request from the assistant
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// A tool result provided by the user
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

/// A tool definition in the Claude API format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeTool {
    /// Tool name (alphanumeric, underscore, hyphen only)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// JSON Schema describing the input parameters
    pub input_schema: serde_json::Value,
}

/// Canonical Claude API response format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeResponse {
    /// Response ID
    pub id: String,
    /// Response content blocks
    pub content: Vec<ClaudeResponseBlock>,
    /// Why generation stopped (e.g., "end_turn", "max_tokens", "tool_use")
    pub stop_reason: String,
    /// Token usage
    pub usage: ClaudeUsage,
}

/// A content block in a Claude response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ClaudeResponseBlock {
    /// Generated text
    #[serde(rename = "text")]
    Text { text: String },
    /// Tool use request
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

/// Token usage reported by the Claude API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// ── Conversion: GenerationContext → ClaudeRequest ───────────────

impl GenerationContext {
    /// Convert this generation context into a Claude API request.
    ///
    /// Maps conversation history to Claude's message format and converts
    /// skill definitions to Claude tool definitions. System messages in
    /// the conversation history are prepended to the system prompt.
    ///
    /// # Arguments
    /// * `model` — The Claude model to target (e.g., "claude-sonnet-4-6")
    /// * `skills` — Available skills to expose as tools
    ///
    /// # Examples
    /// ```
    /// use advisor::engine::context::{GenerationContext, ConversationMessage};
    /// use advisor::engine::claude::ClaudeRequest;
    ///
    /// let ctx = GenerationContext::new()
    ///     .with_system_prompt("You are an advisor")
    ///     .with_message(ConversationMessage::user("hello"))
    ///     .with_temperature(0.5)
    ///     .with_max_tokens(1024);
    ///
    /// let request = ctx.to_claude_request("claude-sonnet-4-6", &[]);
    /// assert_eq!(request.model, "claude-sonnet-4-6");
    /// assert_eq!(request.messages.len(), 1);
    /// ```
    #[must_use]
    pub fn to_claude_request(&self, model: &str, skills: &[SkillDefinition]) -> ClaudeRequest {
        // Collect system-role messages from conversation history and combine
        // with the explicit system prompt.
        let mut system_parts: Vec<String> = Vec::new();
        if let Some(ref sp) = self.system_prompt {
            system_parts.push(sp.clone());
        }

        let mut messages: Vec<ClaudeMessage> = Vec::new();

        for msg in &self.conversation_history {
            match msg.role {
                MessageRole::System => {
                    // Claude API treats system as a top-level field, not a message.
                    // Append system-role messages to the system prompt.
                    system_parts.push(msg.content.clone());
                }
                MessageRole::User => {
                    messages.push(ClaudeMessage {
                        role: "user".into(),
                        content: ClaudeContent::Text(msg.content.clone()),
                    });
                }
                MessageRole::Assistant => {
                    messages.push(ClaudeMessage {
                        role: "assistant".into(),
                        content: ClaudeContent::Text(msg.content.clone()),
                    });
                }
            }
        }

        let system = if system_parts.is_empty() {
            None
        } else {
            Some(system_parts.join("\n\n"))
        };

        let tools: Vec<ClaudeTool> = skills.iter().map(|s| s.to_claude_tool()).collect();

        let max_tokens = self.max_tokens.unwrap_or(4096) as u32;

        // Only include temperature if it differs from 1.0 (API default)
        let temperature = Some(self.temperature);

        ClaudeRequest {
            model: model.into(),
            max_tokens,
            system,
            messages,
            temperature,
            tools,
        }
    }
}

// ── Conversion: ClaudeResponse → GenerationResult ───────────────

impl ClaudeResponse {
    /// Convert a Claude API response into a GenerationResult.
    ///
    /// Extracts text from text blocks and determines the finish reason
    /// from the stop_reason field.
    ///
    /// # Arguments
    /// * `provider_id` — The provider ID to tag the result with
    #[must_use]
    pub fn to_generation_result(&self, provider_id: &str) -> GenerationResult {
        // Concatenate all text blocks
        let content: String = self
            .content
            .iter()
            .filter_map(|block| match block {
                ClaudeResponseBlock::Text { text } => Some(text.as_str()),
                ClaudeResponseBlock::ToolUse { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("");

        let finish_reason = match self.stop_reason.as_str() {
            "end_turn" => FinishReason::Complete,
            "max_tokens" => FinishReason::MaxTokens,
            "tool_use" => FinishReason::ToolCall,
            _ => FinishReason::Error,
        };

        let tokens_used =
            Some((self.usage.input_tokens + self.usage.output_tokens) as usize);

        GenerationResult {
            content,
            tokens_used,
            finish_reason,
            provider_id: provider_id.into(),
        }
    }

    /// Extract skill calls from tool_use blocks in the response.
    ///
    /// Each `ToolUse` block becomes a `SkillCall`. The tool name is
    /// unsanitized back to the original skill ID format (underscores → dots).
    #[must_use]
    pub fn to_skill_calls(&self) -> Vec<SkillCall> {
        self.content
            .iter()
            .filter_map(|block| match block {
                ClaudeResponseBlock::ToolUse { id, name, input } => {
                    Some(tool_use_to_skill_call(id, name, input))
                }
                ClaudeResponseBlock::Text { .. } => None,
            })
            .collect()
    }
}

// ── Conversion: SkillDefinition → ClaudeTool ────────────────────

impl SkillDefinition {
    /// Convert this skill definition to a Claude tool definition.
    ///
    /// Maps parameter types to JSON Schema types:
    /// - "string" → `{"type": "string"}`
    /// - "number" → `{"type": "number"}`
    /// - "boolean" → `{"type": "boolean"}`
    /// - "integer" → `{"type": "integer"}`
    /// - "array" → `{"type": "array"}`
    /// - "object" → `{"type": "object"}`
    /// - anything else → `{"type": "string"}` (safe fallback)
    ///
    /// # Examples
    /// ```
    /// use advisor::skill::{SkillDefinition, SkillParameter};
    ///
    /// let skill = SkillDefinition::new("web.search", "Web Search", "Search the web")
    ///     .with_parameter(SkillParameter::new("query", "string", "Search query", true));
    ///
    /// let tool = skill.to_claude_tool();
    /// assert_eq!(tool.name, "web_search");
    /// ```
    #[must_use]
    pub fn to_claude_tool(&self) -> ClaudeTool {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for param in &self.parameters {
            let json_type = param_type_to_json_schema_type(&param.param_type);
            let mut prop = serde_json::Map::new();
            prop.insert("type".into(), serde_json::Value::String(json_type));
            prop.insert(
                "description".into(),
                serde_json::Value::String(param.description.clone()),
            );
            properties.insert(param.name.clone(), serde_json::Value::Object(prop));

            if param.required {
                required.push(serde_json::Value::String(param.name.clone()));
            }
        }

        let input_schema = serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required,
        });

        ClaudeTool {
            name: self.sanitized_id(),
            description: self.description.clone(),
            input_schema,
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────

/// Map a skill parameter type string to the corresponding JSON Schema type.
fn param_type_to_json_schema_type(param_type: &str) -> String {
    match param_type {
        "string" => "string",
        "number" => "number",
        "boolean" => "boolean",
        "integer" => "integer",
        "array" => "array",
        "object" => "object",
        // Safe fallback — unknown types become strings
        _ => "string",
    }
    .into()
}

/// Convert a Claude tool_use block into a SkillCall.
fn tool_use_to_skill_call(id: &str, name: &str, input: &serde_json::Value) -> SkillCall {
    let skill_id = SkillDefinition::unsanitize_id(name);

    let arguments = match input.as_object() {
        Some(obj) => obj
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        None => std::collections::HashMap::new(),
    };

    SkillCall {
        id: id.into(),
        skill_id,
        arguments,
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::provider::ProviderInfo;
    use crate::skill::definition::SkillParameter;

    // ── Provider tests ──────────────────────────────────────────

    #[test]
    fn claude_provider_configured() {
        let provider = ClaudeProvider::new("claude-sonnet-4-6", true);
        assert_eq!(provider.id(), "anthropic.claude");
        assert_eq!(provider.display_name(), "Claude");
        assert!(provider.is_cloud());
        assert!(provider.status().is_available());
        assert!(provider
            .capabilities()
            .contains(ProviderCapabilities::STREAMING));
        assert!(provider
            .capabilities()
            .contains(ProviderCapabilities::TOOL_CALLING));
        assert!(provider
            .capabilities()
            .contains(ProviderCapabilities::LARGE_CONTEXT));
        assert!(provider
            .capabilities()
            .contains(ProviderCapabilities::STRUCTURED_OUTPUT));
    }

    #[test]
    fn claude_provider_unconfigured() {
        let provider = ClaudeProvider::new("claude-sonnet-4-6", false);
        assert!(!provider.status().is_available());
        match provider.status() {
            ProviderStatus::RequiresSetup { message } => {
                assert!(message.contains("API key"));
            }
            other => panic!("Expected RequiresSetup, got {:?}", other),
        }
    }

    #[test]
    fn claude_provider_set_api_key() {
        let mut provider = ClaudeProvider::new("claude-sonnet-4-6", false);
        assert!(!provider.status().is_available());
        provider.set_api_key_configured(true);
        assert!(provider.status().is_available());
    }

    #[test]
    fn claude_provider_info() {
        let provider = ClaudeProvider::new("claude-sonnet-4-6", true);
        let info = ProviderInfo::from_provider(&provider);
        assert_eq!(info.id, "anthropic.claude");
        assert_eq!(info.display_name, "Claude");
        assert!(info.is_cloud);
    }

    #[test]
    fn claude_provider_model() {
        let provider = ClaudeProvider::new("claude-opus-4-6", true);
        assert_eq!(provider.model(), "claude-opus-4-6");
    }

    // ── GenerationContext → ClaudeRequest ───────────────────────

    #[test]
    fn context_to_claude_request_basic() {
        let ctx = GenerationContext::new()
            .with_system_prompt("You are an advisor")
            .with_message(ConversationMessage::user("hello"))
            .with_message(ConversationMessage::assistant("hi there"))
            .with_temperature(0.5)
            .with_max_tokens(1024);

        let request = ctx.to_claude_request("claude-sonnet-4-6", &[]);

        assert_eq!(request.model, "claude-sonnet-4-6");
        assert_eq!(request.max_tokens, 1024);
        assert_eq!(request.system, Some("You are an advisor".into()));
        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.messages[0].role, "user");
        assert_eq!(request.messages[1].role, "assistant");
        assert_eq!(request.temperature, Some(0.5));
        assert!(request.tools.is_empty());
    }

    #[test]
    fn context_to_claude_request_system_messages_merged() {
        let ctx = GenerationContext::new()
            .with_system_prompt("Base system prompt")
            .with_message(ConversationMessage::system("Extra context"))
            .with_message(ConversationMessage::user("hello"));

        let request = ctx.to_claude_request("claude-sonnet-4-6", &[]);

        // System messages merge into the system field
        assert_eq!(
            request.system,
            Some("Base system prompt\n\nExtra context".into())
        );
        // Only the user message should be in messages
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, "user");
    }

    #[test]
    fn context_to_claude_request_no_system() {
        let ctx = GenerationContext::new()
            .with_message(ConversationMessage::user("hi"));

        let request = ctx.to_claude_request("claude-sonnet-4-6", &[]);
        assert!(request.system.is_none());
    }

    #[test]
    fn context_to_claude_request_default_max_tokens() {
        let ctx = GenerationContext::new();
        let request = ctx.to_claude_request("claude-sonnet-4-6", &[]);
        assert_eq!(request.max_tokens, 4096);
    }

    #[test]
    fn context_to_claude_request_with_skills() {
        let skills = vec![
            SkillDefinition::new("web.search", "Web Search", "Search the web")
                .with_parameter(SkillParameter::new("query", "string", "Search query", true))
                .with_parameter(SkillParameter::new("limit", "number", "Max results", false)),
        ];

        let ctx = GenerationContext::new()
            .with_message(ConversationMessage::user("search for rust"));

        let request = ctx.to_claude_request("claude-sonnet-4-6", &skills);

        assert_eq!(request.tools.len(), 1);
        assert_eq!(request.tools[0].name, "web_search");
        assert_eq!(request.tools[0].description, "Search the web");
    }

    // ── ClaudeResponse → GenerationResult ───────────────────────

    #[test]
    fn response_to_generation_result_text() {
        let response = ClaudeResponse {
            id: "msg_123".into(),
            content: vec![ClaudeResponseBlock::Text {
                text: "I think this is a great idea.".into(),
            }],
            stop_reason: "end_turn".into(),
            usage: ClaudeUsage {
                input_tokens: 100,
                output_tokens: 50,
            },
        };

        let result = response.to_generation_result("anthropic.claude");

        assert_eq!(result.content, "I think this is a great idea.");
        assert_eq!(result.tokens_used, Some(150));
        assert_eq!(result.finish_reason, FinishReason::Complete);
        assert_eq!(result.provider_id, "anthropic.claude");
    }

    #[test]
    fn response_to_generation_result_max_tokens() {
        let response = ClaudeResponse {
            id: "msg_456".into(),
            content: vec![ClaudeResponseBlock::Text {
                text: "truncated response".into(),
            }],
            stop_reason: "max_tokens".into(),
            usage: ClaudeUsage {
                input_tokens: 200,
                output_tokens: 100,
            },
        };

        let result = response.to_generation_result("anthropic.claude");
        assert_eq!(result.finish_reason, FinishReason::MaxTokens);
    }

    #[test]
    fn response_to_generation_result_tool_call() {
        let response = ClaudeResponse {
            id: "msg_789".into(),
            content: vec![
                ClaudeResponseBlock::Text {
                    text: "Let me search for that.".into(),
                },
                ClaudeResponseBlock::ToolUse {
                    id: "toolu_1".into(),
                    name: "web_search".into(),
                    input: serde_json::json!({"query": "rust lang"}),
                },
            ],
            stop_reason: "tool_use".into(),
            usage: ClaudeUsage {
                input_tokens: 150,
                output_tokens: 75,
            },
        };

        let result = response.to_generation_result("anthropic.claude");
        // Text blocks are concatenated, tool_use blocks are skipped in content
        assert_eq!(result.content, "Let me search for that.");
        assert_eq!(result.finish_reason, FinishReason::ToolCall);
    }

    #[test]
    fn response_to_generation_result_unknown_stop_reason() {
        let response = ClaudeResponse {
            id: "msg_err".into(),
            content: vec![],
            stop_reason: "something_unknown".into(),
            usage: ClaudeUsage {
                input_tokens: 10,
                output_tokens: 0,
            },
        };

        let result = response.to_generation_result("anthropic.claude");
        assert_eq!(result.finish_reason, FinishReason::Error);
        assert!(result.content.is_empty());
    }

    // ── SkillDefinition → ClaudeTool ────────────────────────────

    #[test]
    fn skill_to_claude_tool_basic() {
        let skill = SkillDefinition::new("web.search", "Web Search", "Search the web")
            .with_parameter(SkillParameter::new("query", "string", "Search query", true))
            .with_parameter(SkillParameter::new("limit", "number", "Max results", false));

        let tool = skill.to_claude_tool();

        assert_eq!(tool.name, "web_search"); // sanitized
        assert_eq!(tool.description, "Search the web");

        // Verify JSON Schema structure
        let schema = &tool.input_schema;
        assert_eq!(schema["type"], "object");

        let props = schema["properties"].as_object().expect("properties should be object");
        assert_eq!(props.len(), 2);

        // query parameter
        assert_eq!(props["query"]["type"], "string");
        assert_eq!(props["query"]["description"], "Search query");

        // limit parameter
        assert_eq!(props["limit"]["type"], "number");
        assert_eq!(props["limit"]["description"], "Max results");

        // required array
        let required = schema["required"].as_array().expect("required should be array");
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "query");
    }

    #[test]
    fn skill_to_claude_tool_all_param_types() {
        let skill = SkillDefinition::new("test", "Test", "Test all types")
            .with_parameter(SkillParameter::new("a", "string", "a string", true))
            .with_parameter(SkillParameter::new("b", "number", "a number", true))
            .with_parameter(SkillParameter::new("c", "boolean", "a bool", true))
            .with_parameter(SkillParameter::new("d", "integer", "an int", true))
            .with_parameter(SkillParameter::new("e", "array", "a list", true))
            .with_parameter(SkillParameter::new("f", "object", "an obj", true))
            .with_parameter(SkillParameter::new("g", "unknown_type", "unknown", false));

        let tool = skill.to_claude_tool();
        let props = tool.input_schema["properties"]
            .as_object()
            .expect("properties");

        assert_eq!(props["a"]["type"], "string");
        assert_eq!(props["b"]["type"], "number");
        assert_eq!(props["c"]["type"], "boolean");
        assert_eq!(props["d"]["type"], "integer");
        assert_eq!(props["e"]["type"], "array");
        assert_eq!(props["f"]["type"], "object");
        assert_eq!(props["g"]["type"], "string"); // fallback
    }

    #[test]
    fn skill_to_claude_tool_no_params() {
        let skill = SkillDefinition::new("noop", "No-op", "Does nothing");
        let tool = skill.to_claude_tool();

        let props = tool.input_schema["properties"]
            .as_object()
            .expect("properties");
        assert!(props.is_empty());

        let required = tool.input_schema["required"]
            .as_array()
            .expect("required");
        assert!(required.is_empty());
    }

    // ── Tool-use response → SkillCall ───────────────────────────

    #[test]
    fn tool_use_response_to_skill_calls() {
        let response = ClaudeResponse {
            id: "msg_tool".into(),
            content: vec![
                ClaudeResponseBlock::Text {
                    text: "I'll search for that.".into(),
                },
                ClaudeResponseBlock::ToolUse {
                    id: "toolu_abc123".into(),
                    name: "web_search".into(),
                    input: serde_json::json!({"query": "rust programming", "limit": 5}),
                },
            ],
            stop_reason: "tool_use".into(),
            usage: ClaudeUsage {
                input_tokens: 100,
                output_tokens: 50,
            },
        };

        let calls = response.to_skill_calls();
        assert_eq!(calls.len(), 1);

        let call = &calls[0];
        assert_eq!(call.id, "toolu_abc123");
        assert_eq!(call.skill_id, "web.search"); // unsanitized
        assert_eq!(
            call.arguments.get("query"),
            Some(&serde_json::Value::String("rust programming".into()))
        );
        assert_eq!(
            call.arguments.get("limit"),
            Some(&serde_json::json!(5))
        );
    }

    #[test]
    fn tool_use_response_multiple_calls() {
        let response = ClaudeResponse {
            id: "msg_multi".into(),
            content: vec![
                ClaudeResponseBlock::ToolUse {
                    id: "toolu_1".into(),
                    name: "calendar_check".into(),
                    input: serde_json::json!({"date": "2025-01-01"}),
                },
                ClaudeResponseBlock::ToolUse {
                    id: "toolu_2".into(),
                    name: "memory_search".into(),
                    input: serde_json::json!({"query": "birthday"}),
                },
            ],
            stop_reason: "tool_use".into(),
            usage: ClaudeUsage {
                input_tokens: 50,
                output_tokens: 30,
            },
        };

        let calls = response.to_skill_calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].skill_id, "calendar.check");
        assert_eq!(calls[1].skill_id, "memory.search");
    }

    #[test]
    fn tool_use_response_no_tools() {
        let response = ClaudeResponse {
            id: "msg_text".into(),
            content: vec![ClaudeResponseBlock::Text {
                text: "Just text, no tools.".into(),
            }],
            stop_reason: "end_turn".into(),
            usage: ClaudeUsage {
                input_tokens: 20,
                output_tokens: 10,
            },
        };

        let calls = response.to_skill_calls();
        assert!(calls.is_empty());
    }

    #[test]
    fn tool_use_with_empty_input() {
        let response = ClaudeResponse {
            id: "msg_empty".into(),
            content: vec![ClaudeResponseBlock::ToolUse {
                id: "toolu_empty".into(),
                name: "noop".into(),
                input: serde_json::json!({}),
            }],
            stop_reason: "tool_use".into(),
            usage: ClaudeUsage {
                input_tokens: 10,
                output_tokens: 5,
            },
        };

        let calls = response.to_skill_calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].arguments.is_empty());
    }

    // ── Serialization round-trips ───────────────────────────────

    #[test]
    fn claude_request_serialization_roundtrip() {
        let request = ClaudeRequest {
            model: "claude-sonnet-4-6".into(),
            max_tokens: 1024,
            system: Some("You are an advisor".into()),
            messages: vec![ClaudeMessage {
                role: "user".into(),
                content: ClaudeContent::Text("hello".into()),
            }],
            temperature: Some(0.7),
            tools: vec![],
        };

        let json = serde_json::to_string(&request).expect("serialize request");
        let deserialized: ClaudeRequest =
            serde_json::from_str(&json).expect("deserialize request");
        assert_eq!(request, deserialized);
    }

    #[test]
    fn claude_response_serialization_roundtrip() {
        let response = ClaudeResponse {
            id: "msg_123".into(),
            content: vec![
                ClaudeResponseBlock::Text {
                    text: "here is my response".into(),
                },
                ClaudeResponseBlock::ToolUse {
                    id: "toolu_1".into(),
                    name: "test".into(),
                    input: serde_json::json!({"key": "value"}),
                },
            ],
            stop_reason: "tool_use".into(),
            usage: ClaudeUsage {
                input_tokens: 100,
                output_tokens: 50,
            },
        };

        let json = serde_json::to_string(&response).expect("serialize response");
        let deserialized: ClaudeResponse =
            serde_json::from_str(&json).expect("deserialize response");
        assert_eq!(response, deserialized);
    }

    #[test]
    fn claude_content_blocks_serialization() {
        let msg = ClaudeMessage {
            role: "user".into(),
            content: ClaudeContent::Blocks(vec![
                ClaudeContentBlock::Text {
                    text: "here are the results".into(),
                },
                ClaudeContentBlock::ToolResult {
                    tool_use_id: "toolu_1".into(),
                    content: "search returned 3 items".into(),
                },
            ]),
        };

        let json = serde_json::to_string(&msg).expect("serialize blocks");
        let deserialized: ClaudeMessage =
            serde_json::from_str(&json).expect("deserialize blocks");
        assert_eq!(msg, deserialized);
    }
}
