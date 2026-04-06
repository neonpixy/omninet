use serde::{Deserialize, Serialize};

/// Describes a Phone call a module handles.
///
/// ```ignore
/// CallDescriptor::new("vault.lock", "Lock the vault")
///     .with_request_schema(r#"{"password": "string"}"#)
///     .with_response_schema(r#"null"#)
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallDescriptor {
    call_id: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_schema: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_schema: Option<String>,
}

impl CallDescriptor {
    /// Create a call descriptor with an ID and human-readable description.
    pub fn new(call_id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            call_id: call_id.into(),
            description: description.into(),
            request_schema: None,
            response_schema: None,
        }
    }

    /// Attach an optional JSON schema describing the request payload.
    pub fn with_request_schema(mut self, schema: impl Into<String>) -> Self {
        self.request_schema = Some(schema.into());
        self
    }

    /// Attach an optional JSON schema describing the response payload.
    pub fn with_response_schema(mut self, schema: impl Into<String>) -> Self {
        self.response_schema = Some(schema.into());
        self
    }

    /// The routing key for this call (e.g., `"vault.lock"`).
    pub fn call_id(&self) -> &str {
        &self.call_id
    }

    /// Human-readable description of what this call does.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Optional JSON schema for the request payload.
    pub fn request_schema(&self) -> Option<&str> {
        self.request_schema.as_deref()
    }

    /// Optional JSON schema for the response payload.
    pub fn response_schema(&self) -> Option<&str> {
        self.response_schema.as_deref()
    }
}

/// Describes an Email event a module emits or subscribes to.
///
/// ```ignore
/// EventDescriptor::new("crown.profileChanged", "Profile was updated")
///     .with_payload_schema(r#"{"crown_id": "string", "field": "string"}"#)
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventDescriptor {
    email_id: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload_schema: Option<String>,
}

impl EventDescriptor {
    /// Create an event descriptor with an ID and human-readable description.
    pub fn new(email_id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            email_id: email_id.into(),
            description: description.into(),
            payload_schema: None,
        }
    }

    /// Attach an optional JSON schema describing the event payload.
    pub fn with_payload_schema(mut self, schema: impl Into<String>) -> Self {
        self.payload_schema = Some(schema.into());
        self
    }

    /// The routing key for this event (e.g., `"crown.profileChanged"`).
    pub fn email_id(&self) -> &str {
        &self.email_id
    }

    /// Human-readable description of what this event means.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Optional JSON schema for the event payload.
    pub fn payload_schema(&self) -> Option<&str> {
        self.payload_schema.as_deref()
    }
}

/// A module's self-description of its message-passing capabilities.
///
/// ```ignore
/// ModuleCatalog::new()
///     .with_call(CallDescriptor::new("crown.getProfile", "Get user profile"))
///     .with_emitted_event(EventDescriptor::new("crown.profileChanged", "Profile updated"))
///     .with_subscribed_event(EventDescriptor::new("globe.eventReceived", "Relay events"))
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ModuleCatalog {
    calls_handled: Vec<CallDescriptor>,
    events_emitted: Vec<EventDescriptor>,
    events_subscribed: Vec<EventDescriptor>,
    channels_supported: Vec<ChannelDescriptor>,
}

impl ModuleCatalog {
    /// Create an empty catalog with no capabilities declared.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a single Phone call this module handles.
    pub fn with_call(mut self, call: CallDescriptor) -> Self {
        self.calls_handled.push(call);
        self
    }

    /// Add multiple Phone calls this module handles.
    pub fn with_calls(mut self, calls: Vec<CallDescriptor>) -> Self {
        self.calls_handled.extend(calls);
        self
    }

    /// Add a single Email event this module emits.
    pub fn with_emitted_event(mut self, event: EventDescriptor) -> Self {
        self.events_emitted.push(event);
        self
    }

    /// Add multiple Email events this module emits.
    pub fn with_emitted_events(mut self, events: Vec<EventDescriptor>) -> Self {
        self.events_emitted.extend(events);
        self
    }

    /// Add a single Email event this module subscribes to.
    pub fn with_subscribed_event(mut self, event: EventDescriptor) -> Self {
        self.events_subscribed.push(event);
        self
    }

    /// Add multiple Email events this module subscribes to.
    pub fn with_subscribed_events(mut self, events: Vec<EventDescriptor>) -> Self {
        self.events_subscribed.extend(events);
        self
    }

    /// Add a single Communicator channel this module supports.
    pub fn with_channel(mut self, channel: ChannelDescriptor) -> Self {
        self.channels_supported.push(channel);
        self
    }

    /// Add multiple Communicator channels this module supports.
    pub fn with_channels(mut self, channels: Vec<ChannelDescriptor>) -> Self {
        self.channels_supported.extend(channels);
        self
    }

    /// Phone calls this module handles.
    pub fn calls_handled(&self) -> &[CallDescriptor] {
        &self.calls_handled
    }

    /// Email events this module emits.
    pub fn events_emitted(&self) -> &[EventDescriptor] {
        &self.events_emitted
    }

    /// Email events this module subscribes to.
    pub fn events_subscribed(&self) -> &[EventDescriptor] {
        &self.events_subscribed
    }

    /// Communicator channels this module supports.
    pub fn channels_supported(&self) -> &[ChannelDescriptor] {
        &self.channels_supported
    }
}

/// Describes a Communicator channel a module supports.
///
/// ```ignore
/// ChannelDescriptor::new("voice.call", "Voice calls")
///     .with_group_support(true)
///     .with_max_participants(8)
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelDescriptor {
    channel_id: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    supports_group: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_participants: Option<usize>,
}

impl ChannelDescriptor {
    /// Create a channel descriptor with an ID and human-readable description.
    pub fn new(channel_id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            channel_id: channel_id.into(),
            description: description.into(),
            supports_group: None,
            max_participants: None,
        }
    }

    /// Declare whether this channel supports group sessions (more than 2 participants).
    pub fn with_group_support(mut self, supports: bool) -> Self {
        self.supports_group = Some(supports);
        self
    }

    /// Set the maximum number of participants this channel supports.
    pub fn with_max_participants(mut self, max: usize) -> Self {
        self.max_participants = Some(max);
        self
    }

    /// The routing key for this channel (e.g., `"voice.call"`).
    pub fn channel_id(&self) -> &str {
        &self.channel_id
    }

    /// Human-readable description of what this channel is for.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Whether this channel supports group sessions, if declared.
    pub fn supports_group(&self) -> Option<bool> {
        self.supports_group
    }

    /// Maximum participant count, if declared.
    pub fn max_participants(&self) -> Option<usize> {
        self.max_participants
    }
}

/// A directed edge in the message-passing graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageEdge {
    /// The module that initiates this message. Empty string for call edges
    /// (where the caller is unknown).
    pub from_module: String,
    /// The module that receives this message.
    pub to_module: String,
    /// The call ID or event ID being routed.
    pub message_id: String,
    /// Whether this edge represents a Phone call or an Email event.
    pub edge_type: EdgeType,
}

/// The type of message-passing edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EdgeType {
    /// A Phone RPC call (request/response).
    Call,
    /// An Email pub/sub event (fire-and-forget).
    Event,
}

/// The complete message-passing graph, computed from registered catalogs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageTopology {
    /// All directed edges in the communication graph.
    pub edges: Vec<MessageEdge>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_descriptor_defaults() {
        let call = CallDescriptor::new("vault.lock", "Lock the vault");
        assert_eq!(call.call_id(), "vault.lock");
        assert_eq!(call.description(), "Lock the vault");
        assert!(call.request_schema().is_none());
        assert!(call.response_schema().is_none());
    }

    #[test]
    fn call_descriptor_builder() {
        let call = CallDescriptor::new("vault.lock", "Lock the vault")
            .with_request_schema(r#"{"password": "string"}"#)
            .with_response_schema(r#"null"#);

        assert_eq!(call.request_schema(), Some(r#"{"password": "string"}"#));
        assert_eq!(call.response_schema(), Some("null"));
    }

    #[test]
    fn call_descriptor_serde() {
        let call = CallDescriptor::new("vault.lock", "Lock the vault")
            .with_request_schema(r#"{"password": "string"}"#);

        let json = serde_json::to_string(&call).unwrap();
        let deserialized: CallDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(call, deserialized);
    }

    #[test]
    fn call_descriptor_serde_skips_none() {
        let call = CallDescriptor::new("vault.lock", "Lock the vault");
        let json = serde_json::to_string(&call).unwrap();
        assert!(!json.contains("request_schema"));
        assert!(!json.contains("response_schema"));
    }

    #[test]
    fn event_descriptor_defaults() {
        let event = EventDescriptor::new("crown.profileChanged", "Profile was updated");
        assert_eq!(event.email_id(), "crown.profileChanged");
        assert_eq!(event.description(), "Profile was updated");
        assert!(event.payload_schema().is_none());
    }

    #[test]
    fn event_descriptor_builder() {
        let event = EventDescriptor::new("crown.profileChanged", "Profile was updated")
            .with_payload_schema(r#"{"crown_id": "string"}"#);

        assert_eq!(event.payload_schema(), Some(r#"{"crown_id": "string"}"#));
    }

    #[test]
    fn event_descriptor_serde() {
        let event = EventDescriptor::new("crown.profileChanged", "Profile updated")
            .with_payload_schema(r#"{"crown_id": "string"}"#);

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: EventDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn module_catalog_empty() {
        let catalog = ModuleCatalog::new();
        assert!(catalog.calls_handled().is_empty());
        assert!(catalog.events_emitted().is_empty());
        assert!(catalog.events_subscribed().is_empty());
    }

    #[test]
    fn module_catalog_single_builders() {
        let catalog = ModuleCatalog::new()
            .with_call(CallDescriptor::new("crown.getProfile", "Get profile"))
            .with_emitted_event(EventDescriptor::new("crown.profileChanged", "Profile updated"))
            .with_subscribed_event(EventDescriptor::new("globe.eventReceived", "Relay events"));

        assert_eq!(catalog.calls_handled().len(), 1);
        assert_eq!(catalog.events_emitted().len(), 1);
        assert_eq!(catalog.events_subscribed().len(), 1);
        assert_eq!(catalog.calls_handled()[0].call_id(), "crown.getProfile");
    }

    #[test]
    fn module_catalog_batch_builders() {
        let catalog = ModuleCatalog::new()
            .with_calls(vec![
                CallDescriptor::new("vault.lock", "Lock"),
                CallDescriptor::new("vault.unlock", "Unlock"),
            ])
            .with_emitted_events(vec![
                EventDescriptor::new("vault.locked", "Vault locked"),
                EventDescriptor::new("vault.unlocked", "Vault unlocked"),
            ])
            .with_subscribed_events(vec![
                EventDescriptor::new("crown.profileChanged", "Identity changed"),
            ]);

        assert_eq!(catalog.calls_handled().len(), 2);
        assert_eq!(catalog.events_emitted().len(), 2);
        assert_eq!(catalog.events_subscribed().len(), 1);
    }

    #[test]
    fn module_catalog_serde() {
        let catalog = ModuleCatalog::new()
            .with_call(CallDescriptor::new("crown.getProfile", "Get profile"))
            .with_emitted_event(EventDescriptor::new("crown.profileChanged", "Updated"));

        let json = serde_json::to_string(&catalog).unwrap();
        let deserialized: ModuleCatalog = serde_json::from_str(&json).unwrap();
        assert_eq!(catalog, deserialized);
    }

    #[test]
    fn edge_type_serde_camel_case() {
        let json = serde_json::to_string(&EdgeType::Call).unwrap();
        assert_eq!(json, "\"call\"");

        let json = serde_json::to_string(&EdgeType::Event).unwrap();
        assert_eq!(json, "\"event\"");

        let deserialized: EdgeType = serde_json::from_str("\"call\"").unwrap();
        assert_eq!(deserialized, EdgeType::Call);
    }

    #[test]
    fn message_edge_serde() {
        let edge = MessageEdge {
            from_module: "crown".to_string(),
            to_module: "globe".to_string(),
            message_id: "crown.profileChanged".to_string(),
            edge_type: EdgeType::Event,
        };

        let json = serde_json::to_string(&edge).unwrap();
        let deserialized: MessageEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(edge, deserialized);
    }

    #[test]
    fn message_topology_serde() {
        let topology = MessageTopology {
            edges: vec![
                MessageEdge {
                    from_module: "crown".to_string(),
                    to_module: "globe".to_string(),
                    message_id: "crown.profileChanged".to_string(),
                    edge_type: EdgeType::Event,
                },
                MessageEdge {
                    from_module: String::new(),
                    to_module: "vault".to_string(),
                    message_id: "vault.lock".to_string(),
                    edge_type: EdgeType::Call,
                },
            ],
        };

        let json = serde_json::to_string(&topology).unwrap();
        let deserialized: MessageTopology = serde_json::from_str(&json).unwrap();
        assert_eq!(topology, deserialized);
    }
}
