use thiserror::Error;
use url::Url;

/// All errors that can occur in Globe.
///
/// Errors are classified for retry logic:
/// - [`is_retryable`](GlobeError::is_retryable): transient failures worth retrying
/// - [`is_configuration_error`](GlobeError::is_configuration_error): permanent issues needing human intervention
#[derive(Error, Debug)]
pub enum GlobeError {
    // -- Connection --

    /// No relay connection is available to process the request.
    #[error("not connected to any relay")]
    NotConnected,

    /// WebSocket connection attempt to a relay failed.
    #[error("connection to {url} failed: {reason}")]
    ConnectionFailed { url: Url, reason: String },

    /// WebSocket connection attempt timed out before completing.
    #[error("connection to {url} timed out")]
    ConnectionTimeout { url: Url },

    /// Relay explicitly rejected the connection (e.g., at capacity).
    #[error("connection to {url} rejected: {reason}")]
    ConnectionRejected { url: Url, reason: String },

    /// An existing relay connection was closed (by the relay or by network failure).
    #[error("connection to {url} closed{}", reason.as_ref().map(|r| format!(": {r}")).unwrap_or_default())]
    ConnectionClosed { url: Url, reason: Option<String> },

    // -- Protocol --

    /// A message from the relay could not be parsed (bad JSON, missing fields).
    #[error("invalid message: {0}")]
    InvalidMessage(String),

    /// A structurally valid message arrived in an unexpected context.
    #[error("unexpected message: {0}")]
    UnexpectedMessage(String),

    /// A general ORP protocol violation.
    #[error("protocol error: {0}")]
    ProtocolError(String),

    // -- Publishing --

    /// An event could not be sent to the relay (transport failure).
    #[error("publish failed: {0}")]
    PublishFailed(String),

    /// The relay received the event but declined to store it.
    #[error("event {event_id} rejected: {reason}")]
    EventRejected { event_id: String, reason: String },

    /// An event failed structural validation before signing or publishing.
    #[error("invalid event: {}", .0.join(", "))]
    InvalidEvent(Vec<String>),

    // -- Subscription --

    /// A subscription request failed (e.g., channel closed).
    #[error("subscription failed: {0}")]
    SubscriptionFailed(String),

    /// The relay closed a subscription (e.g., rate limited, policy).
    #[error("subscription {subscription_id} closed{}", reason.as_ref().map(|r| format!(": {r}")).unwrap_or_default())]
    SubscriptionClosed {
        subscription_id: String,
        reason: Option<String>,
    },

    // -- Signing --

    /// Event signing failed (e.g., no private key available).
    #[error("signing failed: {0}")]
    SigningFailed(String),

    /// Event signature did not match the expected value.
    #[error("signature verification failed")]
    VerificationFailed,

    // -- Auth --

    /// The relay requires authentication before accepting commands.
    #[error("relay requires authentication")]
    AuthRequired,

    /// Challenge-response authentication failed.
    #[error("authentication failed: {0}")]
    AuthFailed(String),

    // -- Naming --

    /// A domain name lookup found no matching record.
    #[error("name not found: {0}")]
    NameNotFound(String),

    /// A name claim was rejected because another author claimed it first.
    #[error("name already claimed: {0}")]
    NameAlreadyClaimed(String),

    // -- Config --

    /// A configuration value is invalid (zero capacity, backwards ranges, etc.).
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    // -- General --

    /// The Globe client has not been started yet.
    #[error("globe not started")]
    NotStarted,

    /// The Globe client is shutting down and can no longer process requests.
    #[error("globe is shutting down")]
    Shutdown,

    // -- Storage --

    /// A relay-side SQLite/SQLCipher storage operation failed.
    #[error("storage error: {reason}")]
    StorageError { reason: String },

    // -- Bridged errors --

    /// An underlying I/O error (TCP, filesystem, etc.).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// An error propagated from the Crown identity crate.
    #[error("crown error: {0}")]
    Crown(#[from] crown::CrownError),

    /// A WebSocket-level error (handshake failure, frame error, etc.).
    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
}

impl GlobeError {
    /// Whether this error is transient and worth retrying.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            GlobeError::NotConnected
                | GlobeError::ConnectionFailed { .. }
                | GlobeError::ConnectionTimeout { .. }
                | GlobeError::ConnectionClosed { .. }
                | GlobeError::PublishFailed(_)
                | GlobeError::SubscriptionFailed(_)
        )
    }

    /// Whether this error indicates a configuration problem.
    pub fn is_configuration_error(&self) -> bool {
        matches!(
            self,
            GlobeError::InvalidConfig(_) | GlobeError::InvalidEvent(_) | GlobeError::NotStarted
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_contains_context() {
        let url = Url::parse("wss://relay.example.com").unwrap();
        let err = GlobeError::ConnectionFailed {
            url: url.clone(),
            reason: "refused".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("relay.example.com"));
        assert!(msg.contains("refused"));
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GlobeError>();
    }

    #[test]
    fn retryable_errors() {
        assert!(GlobeError::NotConnected.is_retryable());
        assert!(GlobeError::ConnectionTimeout {
            url: Url::parse("wss://r.com").unwrap()
        }
        .is_retryable());
        assert!(GlobeError::PublishFailed("test".into()).is_retryable());
        assert!(!GlobeError::InvalidConfig("bad".into()).is_retryable());
        assert!(!GlobeError::VerificationFailed.is_retryable());
    }

    #[test]
    fn configuration_errors() {
        assert!(GlobeError::InvalidConfig("bad".into()).is_configuration_error());
        assert!(GlobeError::NotStarted.is_configuration_error());
        assert!(GlobeError::InvalidEvent(vec!["bad id".into()]).is_configuration_error());
        assert!(!GlobeError::NotConnected.is_configuration_error());
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let globe_err: GlobeError = io_err.into();
        assert!(matches!(globe_err, GlobeError::Io(_)));
    }

    #[test]
    fn from_crown_error() {
        let crown_err = crown::CrownError::Locked;
        let globe_err: GlobeError = crown_err.into();
        assert!(matches!(globe_err, GlobeError::Crown(_)));
    }

    #[test]
    fn invalid_event_displays_all_reasons() {
        let err = GlobeError::InvalidEvent(vec![
            "bad id".into(),
            "bad sig".into(),
            "future timestamp".into(),
        ]);
        let msg = err.to_string();
        assert!(msg.contains("bad id"));
        assert!(msg.contains("bad sig"));
        assert!(msg.contains("future timestamp"));
    }
}
