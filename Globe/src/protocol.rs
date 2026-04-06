use serde_json::Value;

use crate::error::GlobeError;
use crate::event::OmniEvent;
use crate::filter::OmniFilter;

/// A message from client to relay (ORP wire format).
///
/// Serialized as JSON arrays:
/// - `["EVENT", <event>]`
/// - `["REQ", "<sub_id>", <filter>, ...]`
/// - `["CLOSE", "<sub_id>"]`
/// - `["AUTH", <event>]`
#[derive(Clone, Debug, PartialEq)]
pub enum ClientMessage {
    /// Publish an event.
    Event(OmniEvent),
    /// Subscribe with one or more filters (OR'd). Events matching any filter are delivered.
    Req {
        /// Client-chosen subscription ID for correlating responses.
        subscription_id: String,
        /// One or more filters. An event matching any filter is delivered.
        filters: Vec<OmniFilter>,
    },
    /// Close a subscription.
    Close(String),
    /// Authentication response (signed event).
    Auth(OmniEvent),
}

impl ClientMessage {
    /// Parse a JSON string from a client (used by relay servers).
    pub fn from_json(text: &str) -> Result<Self, GlobeError> {
        let arr: Vec<Value> =
            serde_json::from_str(text).map_err(|e| GlobeError::InvalidMessage(e.to_string()))?;

        let msg_type = arr
            .first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| GlobeError::InvalidMessage("missing message type".into()))?;

        match msg_type {
            "EVENT" => {
                let event: OmniEvent = serde_json::from_value(
                    arr.get(1)
                        .cloned()
                        .ok_or_else(|| GlobeError::InvalidMessage("EVENT: missing event".into()))?,
                )
                .map_err(|e| GlobeError::InvalidMessage(format!("EVENT: {e}")))?;
                Ok(ClientMessage::Event(event))
            }
            "REQ" => {
                let sub_id = arr
                    .get(1)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| GlobeError::InvalidMessage("REQ: missing sub_id".into()))?
                    .to_string();
                let mut filters = Vec::new();
                for val in arr.iter().skip(2) {
                    let filter: OmniFilter = serde_json::from_value(val.clone())
                        .map_err(|e| GlobeError::InvalidMessage(format!("REQ filter: {e}")))?;
                    filters.push(filter);
                }
                Ok(ClientMessage::Req {
                    subscription_id: sub_id,
                    filters,
                })
            }
            "CLOSE" => {
                let sub_id = arr
                    .get(1)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| GlobeError::InvalidMessage("CLOSE: missing sub_id".into()))?
                    .to_string();
                Ok(ClientMessage::Close(sub_id))
            }
            "AUTH" => {
                let event: OmniEvent = serde_json::from_value(
                    arr.get(1)
                        .cloned()
                        .ok_or_else(|| GlobeError::InvalidMessage("AUTH: missing event".into()))?,
                )
                .map_err(|e| GlobeError::InvalidMessage(format!("AUTH: {e}")))?;
                Ok(ClientMessage::Auth(event))
            }
            other => Err(GlobeError::InvalidMessage(format!(
                "unknown client message type: {other}"
            ))),
        }
    }

    /// Encode to JSON string for sending over WebSocket.
    pub fn to_json(&self) -> Result<String, GlobeError> {
        let arr: Vec<Value> = match self {
            ClientMessage::Event(event) => {
                vec![Value::String("EVENT".into()), serde_json::to_value(event)?]
            }
            ClientMessage::Req {
                subscription_id,
                filters,
            } => {
                let mut arr = vec![
                    Value::String("REQ".into()),
                    Value::String(subscription_id.clone()),
                ];
                for filter in filters {
                    arr.push(serde_json::to_value(filter)?);
                }
                arr
            }
            ClientMessage::Close(sub_id) => {
                vec![Value::String("CLOSE".into()), Value::String(sub_id.clone())]
            }
            ClientMessage::Auth(event) => {
                vec![Value::String("AUTH".into()), serde_json::to_value(event)?]
            }
        };
        Ok(serde_json::to_string(&arr)?)
    }
}

/// A message from relay to client (ORP wire format).
///
/// Decoded from JSON arrays:
/// - `["EVENT", "<sub_id>", <event>]`
/// - `["STORED", "<sub_id>"]`
/// - `["OK", "<event_id>", <success>, "<message?>"]`
/// - `["NOTICE", "<message>"]`
/// - `["CLOSED", "<sub_id>", "<reason?>"]`
/// - `["AUTH", "<challenge>"]`
#[derive(Clone, Debug, PartialEq)]
pub enum RelayMessage {
    /// An event matching a subscription.
    Event {
        /// Which subscription this event matched.
        subscription_id: String,
        /// The matched event.
        event: OmniEvent,
    },
    /// End of stored events — live stream begins. Contains the subscription ID.
    Stored(String),
    /// Publish acknowledgment — tells the client whether their event was accepted.
    Ok {
        /// The event ID being acknowledged.
        event_id: String,
        /// Whether the relay stored the event.
        success: bool,
        /// Optional reason for rejection (only meaningful when `success` is false).
        message: Option<String>,
    },
    /// Human-readable relay notice (informational, not an error).
    Notice(String),
    /// Relay closed a subscription (e.g., rate limited, policy change).
    Closed {
        /// Which subscription was closed.
        subscription_id: String,
        /// Why the subscription was closed, if the relay provided a reason.
        reason: Option<String>,
    },
    /// Authentication challenge.
    Auth(String),
    /// A search result with relevance scoring (Semantic Protocol).
    SearchResult {
        /// Which subscription generated this result.
        subscription_id: String,
        /// The matching event.
        event: OmniEvent,
        /// How well the event matched the query (0.0 = barely, 1.0 = perfect).
        relevance: f64,
        /// Text snippet with matched terms highlighted, if available.
        snippet: Option<String>,
        /// Related concept suggestions for query refinement.
        suggestions: Vec<String>,
    },
}

impl RelayMessage {
    /// Encode to JSON string for sending to a client (used by relay servers).
    pub fn to_json(&self) -> Result<String, GlobeError> {
        let arr: Vec<Value> = match self {
            RelayMessage::Event {
                subscription_id,
                event,
            } => vec![
                Value::String("EVENT".into()),
                Value::String(subscription_id.clone()),
                serde_json::to_value(event)?,
            ],
            RelayMessage::Stored(sub_id) => {
                vec![Value::String("STORED".into()), Value::String(sub_id.clone())]
            }
            RelayMessage::Ok {
                event_id,
                success,
                message,
            } => {
                let mut arr = vec![
                    Value::String("OK".into()),
                    Value::String(event_id.clone()),
                    Value::Bool(*success),
                ];
                if let Some(msg) = message {
                    arr.push(Value::String(msg.clone()));
                }
                arr
            }
            RelayMessage::Notice(msg) => {
                vec![Value::String("NOTICE".into()), Value::String(msg.clone())]
            }
            RelayMessage::Closed {
                subscription_id,
                reason,
            } => {
                let mut arr = vec![
                    Value::String("CLOSED".into()),
                    Value::String(subscription_id.clone()),
                ];
                if let Some(r) = reason {
                    arr.push(Value::String(r.clone()));
                }
                arr
            }
            RelayMessage::Auth(challenge) => {
                vec![
                    Value::String("AUTH".into()),
                    Value::String(challenge.clone()),
                ]
            }
            RelayMessage::SearchResult {
                subscription_id,
                event,
                relevance,
                snippet,
                suggestions,
            } => {
                vec![
                    Value::String("SEARCH_RESULT".into()),
                    Value::String(subscription_id.clone()),
                    serde_json::to_value(event)?,
                    Value::Number(serde_json::Number::from_f64(*relevance).unwrap_or_else(|| {
                        // Safety: 0.0 is a finite f64, from_f64 only fails for NaN/Inf.
                        serde_json::Number::from_f64(0.0).expect("0.0 is a valid JSON number")
                    })),
                    match snippet {
                        Some(s) => Value::String(s.clone()),
                        None => Value::Null,
                    },
                    serde_json::to_value(suggestions)?,
                ]
            }
        };
        Ok(serde_json::to_string(&arr)?)
    }

    /// Parse a JSON string from the relay into a RelayMessage.
    pub fn from_json(text: &str) -> Result<Self, GlobeError> {
        let arr: Vec<Value> =
            serde_json::from_str(text).map_err(|e| GlobeError::InvalidMessage(e.to_string()))?;

        let msg_type = arr
            .first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| GlobeError::InvalidMessage("missing message type".into()))?;

        match msg_type {
            "EVENT" => {
                let sub_id = arr
                    .get(1)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| GlobeError::InvalidMessage("EVENT: missing sub_id".into()))?
                    .to_string();
                let event: OmniEvent = serde_json::from_value(
                    arr.get(2)
                        .cloned()
                        .ok_or_else(|| GlobeError::InvalidMessage("EVENT: missing event".into()))?,
                )
                .map_err(|e| GlobeError::InvalidMessage(format!("EVENT: {e}")))?;
                Ok(RelayMessage::Event {
                    subscription_id: sub_id,
                    event,
                })
            }
            "STORED" => {
                let sub_id = arr
                    .get(1)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| GlobeError::InvalidMessage("STORED: missing sub_id".into()))?
                    .to_string();
                Ok(RelayMessage::Stored(sub_id))
            }
            "OK" => {
                let event_id = arr
                    .get(1)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| GlobeError::InvalidMessage("OK: missing event_id".into()))?
                    .to_string();
                let success = arr
                    .get(2)
                    .and_then(|v| v.as_bool())
                    .ok_or_else(|| GlobeError::InvalidMessage("OK: missing success".into()))?;
                let message = arr.get(3).and_then(|v| v.as_str()).map(|s| s.to_string());
                Ok(RelayMessage::Ok {
                    event_id,
                    success,
                    message,
                })
            }
            "NOTICE" => {
                let message = arr
                    .get(1)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| GlobeError::InvalidMessage("NOTICE: missing message".into()))?
                    .to_string();
                Ok(RelayMessage::Notice(message))
            }
            "CLOSED" => {
                let sub_id = arr
                    .get(1)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| GlobeError::InvalidMessage("CLOSED: missing sub_id".into()))?
                    .to_string();
                let reason = arr.get(2).and_then(|v| v.as_str()).map(|s| s.to_string());
                Ok(RelayMessage::Closed {
                    subscription_id: sub_id,
                    reason,
                })
            }
            "AUTH" => {
                let challenge = arr
                    .get(1)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        GlobeError::InvalidMessage("AUTH: missing challenge".into())
                    })?
                    .to_string();
                Ok(RelayMessage::Auth(challenge))
            }
            "SEARCH_RESULT" => {
                let sub_id = arr
                    .get(1)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        GlobeError::InvalidMessage("SEARCH_RESULT: missing sub_id".into())
                    })?
                    .to_string();
                let event: OmniEvent = serde_json::from_value(
                    arr.get(2).cloned().ok_or_else(|| {
                        GlobeError::InvalidMessage("SEARCH_RESULT: missing event".into())
                    })?,
                )
                .map_err(|e| GlobeError::InvalidMessage(format!("SEARCH_RESULT: {e}")))?;
                let relevance = arr
                    .get(3)
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let snippet = arr.get(4).and_then(|v| v.as_str()).map(|s| s.to_string());
                let suggestions: Vec<String> = arr
                    .get(5)
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                Ok(RelayMessage::SearchResult {
                    subscription_id: sub_id,
                    event,
                    relevance,
                    snippet,
                    suggestions,
                })
            }
            other => Err(GlobeError::InvalidMessage(format!(
                "unknown message type: {other}"
            ))),
        }
    }
}

/// Binary frame type indicators.
pub mod binary {
    /// MessagePack-encoded OmniEvent.
    pub const MSGPACK_EVENT: u8 = 0x01;
    /// Raw binary blob (content-addressed).
    pub const RAW_BLOB: u8 = 0x02;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn test_event() -> OmniEvent {
        OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at: Utc::now().timestamp(),
            kind: 1,
            tags: vec![],
            content: "hello".into(),
            sig: "c".repeat(128),
        }
    }

    #[test]
    fn client_event_encodes_as_array() {
        let msg = ClientMessage::Event(test_event());
        let json = msg.to_json().unwrap();
        let arr: Vec<Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(arr[0], "EVENT");
    }

    #[test]
    fn client_req_encodes_with_filters() {
        let msg = ClientMessage::Req {
            subscription_id: "sub-1".into(),
            filters: vec![OmniFilter {
                kinds: Some(vec![1]),
                ..Default::default()
            }],
        };
        let json = msg.to_json().unwrap();
        let arr: Vec<Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(arr[0], "REQ");
        assert_eq!(arr[1], "sub-1");
        assert!(arr[2].is_object());
    }

    #[test]
    fn client_close_encodes() {
        let msg = ClientMessage::Close("sub-1".into());
        let json = msg.to_json().unwrap();
        let arr: Vec<Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(arr[0], "CLOSE");
        assert_eq!(arr[1], "sub-1");
    }

    #[test]
    fn client_auth_encodes() {
        let msg = ClientMessage::Auth(test_event());
        let json = msg.to_json().unwrap();
        let arr: Vec<Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(arr[0], "AUTH");
    }

    #[test]
    fn relay_event_decodes() {
        let event = test_event();
        let json = format!(
            r#"["EVENT","sub-1",{}]"#,
            serde_json::to_string(&event).unwrap()
        );
        let msg = RelayMessage::from_json(&json).unwrap();
        match msg {
            RelayMessage::Event {
                subscription_id,
                event: e,
            } => {
                assert_eq!(subscription_id, "sub-1");
                assert_eq!(e.content, "hello");
            }
            _ => panic!("expected Event"),
        }
    }

    #[test]
    fn relay_stored_decodes() {
        let msg = RelayMessage::from_json(r#"["STORED","sub-1"]"#).unwrap();
        assert_eq!(msg, RelayMessage::Stored("sub-1".into()));
    }

    #[test]
    fn relay_ok_decodes_with_message() {
        let msg =
            RelayMessage::from_json(r#"["OK","event123",true,"accepted"]"#).unwrap();
        match msg {
            RelayMessage::Ok {
                event_id,
                success,
                message,
            } => {
                assert_eq!(event_id, "event123");
                assert!(success);
                assert_eq!(message, Some("accepted".into()));
            }
            _ => panic!("expected Ok"),
        }
    }

    #[test]
    fn relay_ok_decodes_without_message() {
        let msg = RelayMessage::from_json(r#"["OK","event123",false]"#).unwrap();
        match msg {
            RelayMessage::Ok { message, .. } => assert!(message.is_none()),
            _ => panic!("expected Ok"),
        }
    }

    #[test]
    fn relay_notice_decodes() {
        let msg = RelayMessage::from_json(r#"["NOTICE","slow down"]"#).unwrap();
        assert_eq!(msg, RelayMessage::Notice("slow down".into()));
    }

    #[test]
    fn relay_closed_decodes() {
        let msg = RelayMessage::from_json(r#"["CLOSED","sub-1","rate limited"]"#).unwrap();
        match msg {
            RelayMessage::Closed {
                subscription_id,
                reason,
            } => {
                assert_eq!(subscription_id, "sub-1");
                assert_eq!(reason, Some("rate limited".into()));
            }
            _ => panic!("expected Closed"),
        }
    }

    #[test]
    fn relay_auth_decodes() {
        let msg = RelayMessage::from_json(r#"["AUTH","challenge123"]"#).unwrap();
        assert_eq!(msg, RelayMessage::Auth("challenge123".into()));
    }

    #[test]
    fn unknown_message_type_errors() {
        let result = RelayMessage::from_json(r#"["UNKNOWN","data"]"#);
        assert!(result.is_err());
    }

    // -- Server-side: ClientMessage::from_json --

    #[test]
    fn client_event_round_trip() {
        let original = ClientMessage::Event(test_event());
        let json = original.to_json().unwrap();
        let parsed = ClientMessage::from_json(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn client_req_round_trip() {
        let original = ClientMessage::Req {
            subscription_id: "sub-1".into(),
            filters: vec![OmniFilter {
                kinds: Some(vec![1, 7000]),
                ..Default::default()
            }],
        };
        let json = original.to_json().unwrap();
        let parsed = ClientMessage::from_json(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn client_close_round_trip() {
        let original = ClientMessage::Close("sub-42".into());
        let json = original.to_json().unwrap();
        let parsed = ClientMessage::from_json(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn client_auth_round_trip() {
        let original = ClientMessage::Auth(test_event());
        let json = original.to_json().unwrap();
        let parsed = ClientMessage::from_json(&json).unwrap();
        assert_eq!(original, parsed);
    }

    // -- Server-side: RelayMessage::to_json --

    #[test]
    fn relay_event_round_trip() {
        let original = RelayMessage::Event {
            subscription_id: "sub-1".into(),
            event: test_event(),
        };
        let json = original.to_json().unwrap();
        let parsed = RelayMessage::from_json(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn relay_ok_round_trip() {
        let original = RelayMessage::Ok {
            event_id: "abc".into(),
            success: true,
            message: Some("accepted".into()),
        };
        let json = original.to_json().unwrap();
        let parsed = RelayMessage::from_json(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn relay_stored_round_trip() {
        let original = RelayMessage::Stored("sub-5".into());
        let json = original.to_json().unwrap();
        let parsed = RelayMessage::from_json(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn search_result_round_trip() {
        let original = RelayMessage::SearchResult {
            subscription_id: "search-1".into(),
            event: test_event(),
            relevance: 0.85,
            snippet: Some("matched **text**".into()),
            suggestions: vec!["related".into(), "concept".into()],
        };
        let json = original.to_json().unwrap();
        let parsed = RelayMessage::from_json(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn search_result_without_snippet() {
        let original = RelayMessage::SearchResult {
            subscription_id: "search-2".into(),
            event: test_event(),
            relevance: 0.5,
            snippet: None,
            suggestions: vec![],
        };
        let json = original.to_json().unwrap();
        let parsed = RelayMessage::from_json(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn search_result_decode_from_raw_json() {
        let event = test_event();
        let event_json = serde_json::to_string(&event).unwrap();
        let json = format!(
            r#"["SEARCH_RESULT","sub-1",{},0.9,"snippet text",["suggestion"]]"#,
            event_json
        );
        let msg = RelayMessage::from_json(&json).unwrap();
        match msg {
            RelayMessage::SearchResult {
                subscription_id,
                relevance,
                snippet,
                suggestions,
                ..
            } => {
                assert_eq!(subscription_id, "sub-1");
                assert!((relevance - 0.9).abs() < f64::EPSILON);
                assert_eq!(snippet, Some("snippet text".into()));
                assert_eq!(suggestions, vec!["suggestion"]);
            }
            _ => panic!("expected SearchResult"),
        }
    }
}
