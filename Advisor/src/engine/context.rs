use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Everything the LLM needs to generate a response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerationContext {
    /// System prompt
    pub system_prompt: Option<String>,
    /// Conversation history
    pub conversation_history: Vec<ConversationMessage>,
    /// What the advisor is currently focused on
    pub attention_focus: Vec<String>,
    /// Temperature (0.0 = deterministic, 1.0 = creative)
    pub temperature: f64,
    /// Maximum tokens to generate
    pub max_tokens: Option<usize>,
}

impl GenerationContext {
    pub fn new() -> Self {
        Self {
            system_prompt: None,
            conversation_history: Vec::new(),
            attention_focus: Vec::new(),
            temperature: 0.7,
            max_tokens: None,
        }
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn with_message(mut self, message: ConversationMessage) -> Self {
        self.conversation_history.push(message);
        self
    }

    pub fn with_focus(mut self, focus: Vec<String>) -> Self {
        self.attention_focus = focus;
        self
    }

    pub fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = temperature.clamp(0.0, 1.0);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Estimated token count (rough: 4 chars ≈ 1 token).
    pub fn estimated_tokens(&self) -> usize {
        let mut chars = 0;
        if let Some(ref sp) = self.system_prompt {
            chars += sp.len();
        }
        for msg in &self.conversation_history {
            chars += msg.content.len();
        }
        chars / 4
    }
}

impl Default for GenerationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConversationMessage {
    pub id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

impl ConversationMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::System,
            content: content.into(),
            timestamp: Utc::now(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content: content.into(),
            timestamp: Utc::now(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::Assistant,
            content: content.into(),
            timestamp: Utc::now(),
        }
    }
}

/// Message role in a conversation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// The result of an LLM generation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerationResult {
    /// Generated text content
    pub content: String,
    /// Tokens consumed (if reported by provider)
    pub tokens_used: Option<usize>,
    /// Why generation stopped
    pub finish_reason: FinishReason,
    /// Which provider generated this
    pub provider_id: String,
}

/// Why an LLM generation stopped.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FinishReason {
    /// Completed naturally
    Complete,
    /// Hit the max token limit
    MaxTokens,
    /// Wants to call a tool/skill
    ToolCall,
    /// Error during generation
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_builder() {
        let ctx = GenerationContext::new()
            .with_system_prompt("You are an advisor")
            .with_message(ConversationMessage::user("hello"))
            .with_temperature(0.5)
            .with_max_tokens(1000)
            .with_focus(vec!["design".into()]);

        assert!(ctx.system_prompt.is_some());
        assert_eq!(ctx.conversation_history.len(), 1);
        assert!((ctx.temperature - 0.5).abs() < f64::EPSILON);
        assert_eq!(ctx.max_tokens, Some(1000));
    }

    #[test]
    fn temperature_clamped() {
        let ctx = GenerationContext::new().with_temperature(2.0);
        assert!((ctx.temperature - 1.0).abs() < f64::EPSILON);

        let ctx2 = GenerationContext::new().with_temperature(-0.5);
        assert!((ctx2.temperature - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn estimated_tokens() {
        let ctx = GenerationContext::new()
            .with_system_prompt("a".repeat(400))
            .with_message(ConversationMessage::user("b".repeat(400)));
        assert_eq!(ctx.estimated_tokens(), 200);
    }

    #[test]
    fn conversation_message_factories() {
        let sys = ConversationMessage::system("be helpful");
        assert_eq!(sys.role, MessageRole::System);

        let usr = ConversationMessage::user("hi");
        assert_eq!(usr.role, MessageRole::User);

        let ast = ConversationMessage::assistant("hello");
        assert_eq!(ast.role, MessageRole::Assistant);
    }

    #[test]
    fn generation_result() {
        let result = GenerationResult {
            content: "I think...".into(),
            tokens_used: Some(42),
            finish_reason: FinishReason::Complete,
            provider_id: "claude".into(),
        };
        assert_eq!(result.finish_reason, FinishReason::Complete);
    }

    #[test]
    fn context_serialization_roundtrip() {
        let ctx = GenerationContext::new()
            .with_system_prompt("test")
            .with_message(ConversationMessage::user("hello"));
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: GenerationContext = serde_json::from_str(&json).unwrap();
        assert_eq!(ctx, deserialized);
    }
}
