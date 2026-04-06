use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::communicator_types::{Confidentiality, PrivacyRoute};
use crate::error::CommunicatorError;

/// A real-time communication channel type.
///
/// Modules define channel types by implementing this trait. The `CHANNEL_ID`
/// is the routing key (convention: `"domain.type"`, e.g. `"voice.call"`,
/// `"music.stream"`).
///
/// ```ignore
/// struct VoiceCall;
/// impl CommunicatorChannel for VoiceCall {
///     const CHANNEL_ID: &'static str = "voice.call";
/// }
/// ```
pub trait CommunicatorChannel: Send + Sync {
    /// The routing key (convention: `"domain.type"`).
    const CHANNEL_ID: &'static str;
}

/// Status of a real-time communication session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionStatus {
    /// Offer sent, waiting for acceptance.
    Offering,
    /// Session is active and streaming.
    Active,
    /// Session ended gracefully.
    Ended,
    /// Session failed with an error.
    Failed,
}

/// A real-time communication session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommunicatorSession {
    /// Unique identifier for this session.
    pub session_id: String,
    /// The channel type (e.g., `"voice.call"`, `"music.stream"`).
    pub channel_id: String,
    /// Crown IDs of all participants.
    pub participants: Vec<String>,
    /// When this session was created.
    pub created_at: DateTime<Utc>,
    /// Current lifecycle state.
    pub status: SessionStatus,
    /// Optional privacy route for intermediary-routed sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_route: Option<PrivacyRoute>,
    /// Confidentiality level for this session.
    #[serde(default)]
    pub confidentiality: Confidentiality,
}

impl CommunicatorSession {
    /// Returns `true` if this session uses privacy features.
    ///
    /// A session is considered private if its confidentiality is `High`
    /// or if a `privacy_route` is set.
    #[must_use]
    pub fn is_private(&self) -> bool {
        self.confidentiality == Confidentiality::High || self.privacy_route.is_some()
    }

    /// Builder method: attach a privacy route and confidentiality level.
    #[must_use]
    pub fn with_privacy(mut self, route: PrivacyRoute, confidentiality: Confidentiality) -> Self {
        self.privacy_route = Some(route);
        self.confidentiality = confidentiality;
        self
    }
}

/// Raw handler for communicator session events.
type RawSessionHandler =
    Box<dyn Fn(&str, &str, &[u8]) -> Result<(), CommunicatorError> + Send + Sync>;

/// The Communicator — Equipment's fifth primitive.
///
/// Manages real-time communication sessions (voice calls, music streams, etc).
/// Like Phone and Email, it's sync, string-routed, and zero internal deps.
///
/// The Communicator tracks session state. The actual audio/video transport
/// happens in Globe (binary frames) and _Codecs (encoding/decoding).
pub struct Communicator {
    sessions: Mutex<HashMap<String, CommunicatorSession>>,
    handlers: Mutex<HashMap<String, RawSessionHandler>>,
}

impl Communicator {
    /// Create a new Communicator with no sessions or handlers.
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            handlers: Mutex::new(HashMap::new()),
        }
    }

    /// Register a handler for a channel type.
    ///
    /// The handler is called when a session event occurs for this channel.
    /// Arguments: (session_id, event_type, payload_bytes).
    /// Event types: "offer", "accept", "end", "data".
    pub fn register<C: CommunicatorChannel>(
        &self,
        handler: impl Fn(&str, &str, &[u8]) -> Result<(), CommunicatorError> + Send + Sync + 'static,
    ) {
        self.handlers
            .lock()
            .expect("handlers mutex poisoned")
            .insert(C::CHANNEL_ID.to_string(), Box::new(handler));
    }

    /// Register a raw handler by string channel ID.
    pub fn register_raw(
        &self,
        channel_id: impl Into<String>,
        handler: impl Fn(&str, &str, &[u8]) -> Result<(), CommunicatorError> + Send + Sync + 'static,
    ) {
        self.handlers
            .lock()
            .expect("handlers mutex poisoned")
            .insert(channel_id.into(), Box::new(handler));
    }

    /// Create a new session offer.
    ///
    /// Returns the session with `Offering` status.
    pub fn offer(
        &self,
        channel_id: &str,
        participants: Vec<String>,
    ) -> Result<CommunicatorSession, CommunicatorError> {
        if participants.is_empty() {
            return Err(CommunicatorError::NoParticipants);
        }

        let session = CommunicatorSession {
            session_id: Uuid::new_v4().to_string(),
            channel_id: channel_id.to_string(),
            participants,
            created_at: Utc::now(),
            status: SessionStatus::Offering,
            privacy_route: None,
            confidentiality: Confidentiality::default(),
        };

        self.sessions
            .lock()
            .expect("sessions mutex poisoned")
            .insert(session.session_id.clone(), session.clone());

        // Notify handler if registered.
        self.notify_handler(&session.channel_id, &session.session_id, "offer", &[]);

        Ok(session)
    }

    /// Accept an offered session, transitioning it to `Active`.
    pub fn accept(&self, session_id: &str) -> Result<CommunicatorSession, CommunicatorError> {
        let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| CommunicatorError::SessionNotFound(session_id.to_string()))?;

        if session.status != SessionStatus::Offering {
            return Err(CommunicatorError::InvalidTransition {
                session_id: session_id.to_string(),
                from: session.status,
                to: SessionStatus::Active,
            });
        }

        session.status = SessionStatus::Active;
        let session = session.clone();
        drop(sessions);

        self.notify_handler(&session.channel_id, &session.session_id, "accept", &[]);

        Ok(session)
    }

    /// End a session gracefully.
    pub fn end(&self, session_id: &str) -> Result<CommunicatorSession, CommunicatorError> {
        let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| CommunicatorError::SessionNotFound(session_id.to_string()))?;

        if session.status == SessionStatus::Ended || session.status == SessionStatus::Failed {
            return Err(CommunicatorError::InvalidTransition {
                session_id: session_id.to_string(),
                from: session.status,
                to: SessionStatus::Ended,
            });
        }

        session.status = SessionStatus::Ended;
        let session = session.clone();
        drop(sessions);

        self.notify_handler(&session.channel_id, &session.session_id, "end", &[]);

        Ok(session)
    }

    /// Mark a session as failed.
    pub fn fail(
        &self,
        session_id: &str,
        reason: &str,
    ) -> Result<CommunicatorSession, CommunicatorError> {
        let mut sessions = self.sessions.lock().expect("sessions mutex poisoned");
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| CommunicatorError::SessionNotFound(session_id.to_string()))?;

        session.status = SessionStatus::Failed;
        let session = session.clone();
        drop(sessions);

        self.notify_handler(
            &session.channel_id,
            &session.session_id,
            "fail",
            reason.as_bytes(),
        );

        Ok(session)
    }

    /// Deliver data to a session's handler.
    pub fn deliver(
        &self,
        session_id: &str,
        data: &[u8],
    ) -> Result<(), CommunicatorError> {
        let sessions = self.sessions.lock().expect("sessions mutex poisoned");
        let session = sessions
            .get(session_id)
            .ok_or_else(|| CommunicatorError::SessionNotFound(session_id.to_string()))?;

        if session.status != SessionStatus::Active {
            return Err(CommunicatorError::SessionNotActive(session_id.to_string()));
        }

        let channel_id = session.channel_id.clone();
        let sid = session.session_id.clone();
        drop(sessions);

        // Handler lock is separate from sessions lock — no deadlock risk.
        let handlers = self.handlers.lock().expect("handlers mutex poisoned");
        if let Some(h) = handlers.get(&channel_id) {
            h(&sid, "data", data)?;
        }

        Ok(())
    }

    /// Get a session by ID.
    pub fn session(&self, session_id: &str) -> Option<CommunicatorSession> {
        self.sessions.lock().expect("sessions mutex poisoned").get(session_id).cloned()
    }

    /// Get all active sessions (Offering or Active).
    pub fn active_sessions(&self) -> Vec<CommunicatorSession> {
        self.sessions
            .lock()
            .expect("sessions mutex poisoned")
            .values()
            .filter(|s| s.status == SessionStatus::Offering || s.status == SessionStatus::Active)
            .cloned()
            .collect()
    }

    /// Get all sessions for a channel type.
    pub fn sessions_for_channel(&self, channel_id: &str) -> Vec<CommunicatorSession> {
        self.sessions
            .lock()
            .expect("sessions mutex poisoned")
            .values()
            .filter(|s| s.channel_id == channel_id)
            .cloned()
            .collect()
    }

    /// All registered channel IDs.
    pub fn registered_channel_ids(&self) -> Vec<String> {
        self.handlers.lock().expect("handlers mutex poisoned").keys().cloned().collect()
    }

    /// Check if a handler is registered for a channel ID.
    pub fn has_handler(&self, channel_id: &str) -> bool {
        self.handlers.lock().expect("handlers mutex poisoned").contains_key(channel_id)
    }

    /// Unregister a handler by channel ID.
    pub fn unregister(&self, channel_id: &str) {
        self.handlers.lock().expect("handlers mutex poisoned").remove(channel_id);
    }

    /// Remove ended/failed sessions older than `max_age`.
    pub fn prune(&self, max_age: chrono::Duration) {
        let cutoff = Utc::now() - max_age;
        self.sessions.lock().expect("sessions mutex poisoned").retain(|_, s| {
            if s.status == SessionStatus::Ended || s.status == SessionStatus::Failed {
                s.created_at > cutoff
            } else {
                true
            }
        });
    }

    fn notify_handler(&self, channel_id: &str, session_id: &str, event_type: &str, data: &[u8]) {
        let handlers = self.handlers.lock().expect("handlers mutex poisoned");
        if let Some(handler) = handlers.get(channel_id) {
            let _ = handler(session_id, event_type, data);
        }
    }
}

impl Default for Communicator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::communicator_types::{Confidentiality, PrivacyRoute};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct VoiceCall;
    impl CommunicatorChannel for VoiceCall {
        const CHANNEL_ID: &'static str = "voice.call";
    }

    struct MusicStream;
    impl CommunicatorChannel for MusicStream {
        const CHANNEL_ID: &'static str = "music.stream";
    }

    #[test]
    fn create_and_accept_session() {
        let comm = Communicator::new();
        let session = comm
            .offer("voice.call", vec!["alice".into(), "bob".into()])
            .unwrap();
        assert_eq!(session.status, SessionStatus::Offering);
        assert_eq!(session.channel_id, "voice.call");
        assert_eq!(session.participants.len(), 2);

        let active = comm.accept(&session.session_id).unwrap();
        assert_eq!(active.status, SessionStatus::Active);
    }

    #[test]
    fn end_session() {
        let comm = Communicator::new();
        let session = comm
            .offer("voice.call", vec!["alice".into()])
            .unwrap();
        comm.accept(&session.session_id).unwrap();
        let ended = comm.end(&session.session_id).unwrap();
        assert_eq!(ended.status, SessionStatus::Ended);
    }

    #[test]
    fn fail_session() {
        let comm = Communicator::new();
        let session = comm
            .offer("voice.call", vec!["alice".into()])
            .unwrap();
        comm.accept(&session.session_id).unwrap();
        let failed = comm.fail(&session.session_id, "network error").unwrap();
        assert_eq!(failed.status, SessionStatus::Failed);
    }

    #[test]
    fn cannot_accept_ended_session() {
        let comm = Communicator::new();
        let session = comm
            .offer("voice.call", vec!["alice".into()])
            .unwrap();
        comm.accept(&session.session_id).unwrap();
        comm.end(&session.session_id).unwrap();
        let result = comm.accept(&session.session_id);
        assert!(matches!(
            result,
            Err(CommunicatorError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn cannot_end_ended_session() {
        let comm = Communicator::new();
        let session = comm
            .offer("voice.call", vec!["alice".into()])
            .unwrap();
        comm.end(&session.session_id).unwrap();
        let result = comm.end(&session.session_id);
        assert!(matches!(
            result,
            Err(CommunicatorError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn session_not_found() {
        let comm = Communicator::new();
        let result = comm.accept("nonexistent");
        assert!(matches!(
            result,
            Err(CommunicatorError::SessionNotFound(_))
        ));
    }

    #[test]
    fn no_participants_rejected() {
        let comm = Communicator::new();
        let result = comm.offer("voice.call", vec![]);
        assert!(matches!(result, Err(CommunicatorError::NoParticipants)));
    }

    #[test]
    fn active_sessions_filter() {
        let comm = Communicator::new();
        let s1 = comm
            .offer("voice.call", vec!["alice".into()])
            .unwrap();
        let s2 = comm
            .offer("music.stream", vec!["bob".into()])
            .unwrap();
        comm.accept(&s1.session_id).unwrap();
        comm.accept(&s2.session_id).unwrap();
        comm.end(&s2.session_id).unwrap();

        let active = comm.active_sessions();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].session_id, s1.session_id);
    }

    #[test]
    fn sessions_for_channel() {
        let comm = Communicator::new();
        comm.offer("voice.call", vec!["alice".into()]).unwrap();
        comm.offer("voice.call", vec!["bob".into()]).unwrap();
        comm.offer("music.stream", vec!["charlie".into()])
            .unwrap();

        let voice = comm.sessions_for_channel("voice.call");
        assert_eq!(voice.len(), 2);
        let music = comm.sessions_for_channel("music.stream");
        assert_eq!(music.len(), 1);
    }

    #[test]
    fn register_typed_handler() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let comm = Communicator::new();
        comm.register::<VoiceCall>(move |_sid, _event, _data| {
            counter_clone.fetch_add(1, Ordering::Relaxed);
            Ok(())
        });

        assert!(comm.has_handler("voice.call"));
        assert!(!comm.has_handler("music.stream"));

        // Handler is called on offer.
        comm.offer("voice.call", vec!["alice".into()]).unwrap();
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn register_raw_handler() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let comm = Communicator::new();
        comm.register_raw("music.stream", move |_sid, _event, _data| {
            counter_clone.fetch_add(1, Ordering::Relaxed);
            Ok(())
        });

        comm.offer("music.stream", vec!["alice".into()]).unwrap();
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn unregister_handler() {
        let comm = Communicator::new();
        comm.register::<VoiceCall>(|_sid, _event, _data| Ok(()));
        assert!(comm.has_handler("voice.call"));

        comm.unregister("voice.call");
        assert!(!comm.has_handler("voice.call"));
    }

    #[test]
    fn registered_channel_ids() {
        let comm = Communicator::new();
        comm.register::<VoiceCall>(|_sid, _event, _data| Ok(()));
        comm.register::<MusicStream>(|_sid, _event, _data| Ok(()));

        let mut ids = comm.registered_channel_ids();
        ids.sort();
        assert_eq!(ids, vec!["music.stream", "voice.call"]);
    }

    #[test]
    fn deliver_to_active_session() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let comm = Communicator::new();
        comm.register_raw("voice.call", move |_sid, event, data| {
            if event == "data" {
                received_clone.lock().unwrap().extend_from_slice(data);
            }
            Ok(())
        });

        let session = comm
            .offer("voice.call", vec!["alice".into()])
            .unwrap();
        comm.accept(&session.session_id).unwrap();

        comm.deliver(&session.session_id, b"audio_frame_001")
            .unwrap();

        let data = received.lock().unwrap();
        assert_eq!(&*data, b"audio_frame_001");
    }

    #[test]
    fn deliver_to_offering_session_fails() {
        let comm = Communicator::new();
        let session = comm
            .offer("voice.call", vec!["alice".into()])
            .unwrap();

        let result = comm.deliver(&session.session_id, b"data");
        assert!(matches!(
            result,
            Err(CommunicatorError::SessionNotActive(_))
        ));
    }

    #[test]
    fn session_serde_round_trip() {
        let session = CommunicatorSession {
            session_id: "abc-123".into(),
            channel_id: "voice.call".into(),
            participants: vec!["alice".into(), "bob".into()],
            created_at: Utc::now(),
            status: SessionStatus::Active,
            privacy_route: None,
            confidentiality: Confidentiality::default(),
        };

        let json = serde_json::to_string(&session).unwrap();
        let loaded: CommunicatorSession = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.session_id, session.session_id);
        assert_eq!(loaded.status, SessionStatus::Active);
    }

    #[test]
    fn session_status_serde() {
        let json = serde_json::to_string(&SessionStatus::Offering).unwrap();
        assert_eq!(json, "\"offering\"");
        let loaded: SessionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, SessionStatus::Offering);
    }

    #[test]
    fn prune_removes_old_ended() {
        let comm = Communicator::new();
        let s1 = comm
            .offer("voice.call", vec!["alice".into()])
            .unwrap();
        comm.end(&s1.session_id).unwrap();

        // Active session should survive prune.
        let s2 = comm
            .offer("voice.call", vec!["bob".into()])
            .unwrap();
        comm.accept(&s2.session_id).unwrap();

        // Prune with zero duration removes all ended/failed.
        comm.prune(chrono::Duration::zero());

        assert!(comm.session(&s1.session_id).is_none());
        assert!(comm.session(&s2.session_id).is_some());
    }

    #[test]
    fn session_lookup() {
        let comm = Communicator::new();
        assert!(comm.session("nonexistent").is_none());

        let s = comm
            .offer("voice.call", vec!["alice".into()])
            .unwrap();
        let found = comm.session(&s.session_id).unwrap();
        assert_eq!(found.channel_id, "voice.call");
    }

    // --- Privacy-aware session tests ---

    #[test]
    fn session_default_is_not_private() {
        let comm = Communicator::new();
        let session = comm
            .offer("voice.call", vec!["alice".into()])
            .unwrap();
        assert!(!session.is_private());
        assert_eq!(session.confidentiality, Confidentiality::Standard);
        assert!(session.privacy_route.is_none());
    }

    #[test]
    fn is_private_true_for_high_confidentiality() {
        let session = CommunicatorSession {
            session_id: "priv-001".into(),
            channel_id: "voice.call".into(),
            participants: vec!["alice".into()],
            created_at: Utc::now(),
            status: SessionStatus::Active,
            privacy_route: None,
            confidentiality: Confidentiality::High,
        };
        assert!(session.is_private());
    }

    #[test]
    fn is_private_true_when_privacy_route_set() {
        let session = CommunicatorSession {
            session_id: "priv-002".into(),
            channel_id: "voice.call".into(),
            participants: vec!["alice".into()],
            created_at: Utc::now(),
            status: SessionStatus::Active,
            privacy_route: Some(PrivacyRoute {
                relay_path: vec!["wss://relay1.example.com".into()],
                blinding_context: None,
            }),
            confidentiality: Confidentiality::Standard,
        };
        assert!(session.is_private());
    }

    #[test]
    fn with_privacy_builder_sets_both_fields() {
        let session = CommunicatorSession {
            session_id: "priv-003".into(),
            channel_id: "voice.call".into(),
            participants: vec!["alice".into(), "bob".into()],
            created_at: Utc::now(),
            status: SessionStatus::Offering,
            privacy_route: None,
            confidentiality: Confidentiality::Standard,
        };

        let route = PrivacyRoute {
            relay_path: vec![
                "wss://relay1.example.com".into(),
                "wss://relay2.example.com".into(),
            ],
            blinding_context: Some("blind-ctx".into()),
        };

        let private_session = session.with_privacy(route, Confidentiality::High);
        assert!(private_session.is_private());
        assert_eq!(private_session.confidentiality, Confidentiality::High);
        let pr = private_session.privacy_route.as_ref().unwrap();
        assert_eq!(pr.relay_path.len(), 2);
        assert_eq!(pr.blinding_context.as_deref(), Some("blind-ctx"));
    }

    #[test]
    fn session_with_privacy_serde_round_trip() {
        let session = CommunicatorSession {
            session_id: "priv-004".into(),
            channel_id: "voice.call".into(),
            participants: vec!["alice".into()],
            created_at: Utc::now(),
            status: SessionStatus::Active,
            privacy_route: Some(PrivacyRoute {
                relay_path: vec!["wss://relay1.example.com".into()],
                blinding_context: Some("ctx-001".into()),
            }),
            confidentiality: Confidentiality::High,
        };

        let json = serde_json::to_string(&session).unwrap();
        let loaded: CommunicatorSession = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.confidentiality, Confidentiality::High);
        assert!(loaded.privacy_route.is_some());
        let pr = loaded.privacy_route.unwrap();
        assert_eq!(pr.relay_path[0], "wss://relay1.example.com");
        assert_eq!(pr.blinding_context.as_deref(), Some("ctx-001"));
    }

    #[test]
    fn session_backward_compat_without_privacy_fields() {
        // JSON produced by older code without privacy_route or confidentiality.
        let json = r#"{
            "session_id": "old-session",
            "channel_id": "voice.call",
            "participants": ["alice"],
            "created_at": "2025-01-01T00:00:00Z",
            "status": "active"
        }"#;

        let loaded: CommunicatorSession = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.session_id, "old-session");
        assert_eq!(loaded.confidentiality, Confidentiality::Standard);
        assert!(loaded.privacy_route.is_none());
        assert!(!loaded.is_private());
    }
}
