use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A cognitive session — either the home monologue or a user conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub id: Uuid,
    /// Home (inner monologue, singleton) or User (conversation)
    pub session_type: SessionType,
    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
    /// Optional title (user sessions only)
    pub title: Option<String>,
    /// Auto-generated summary of the session
    pub summary: Option<String>,
    /// Key topics discussed
    pub key_topics: Vec<String>,
    /// Number of thoughts in this session
    pub thought_count: usize,
    /// Whether this session is archived
    pub is_archived: bool,
}

/// The two kinds of session.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SessionType {
    /// The advisor's inner monologue — singleton, never archived.
    /// This is the "brain stem" from Solas v3.
    Home,
    /// A conversation with the user — archivable.
    User,
}

impl Session {
    /// Create the home session (singleton inner monologue).
    pub fn home() -> Self {
        Self {
            id: Uuid::new_v4(),
            session_type: SessionType::Home,
            created_at: Utc::now(),
            last_active_at: Utc::now(),
            title: None,
            summary: None,
            key_topics: Vec::new(),
            thought_count: 0,
            is_archived: false,
        }
    }

    /// Create a new user session.
    pub fn user(title: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_type: SessionType::User,
            created_at: Utc::now(),
            last_active_at: Utc::now(),
            title: Some(title.into()),
            summary: None,
            key_topics: Vec::new(),
            thought_count: 0,
            is_archived: false,
        }
    }

    /// Touch — mark the session as active now.
    pub fn touch(&mut self) {
        self.last_active_at = Utc::now();
    }

    /// Increment thought count and touch.
    pub fn add_thought(&mut self) {
        self.thought_count += 1;
        self.touch();
    }

    /// Archive the session (user sessions only).
    pub fn archive(&mut self) -> Result<(), crate::error::AdvisorError> {
        if self.session_type == SessionType::Home {
            return Err(crate::error::AdvisorError::CannotModifyHomeSession);
        }
        self.is_archived = true;
        Ok(())
    }

    /// Unarchive the session.
    pub fn unarchive(&mut self) -> Result<(), crate::error::AdvisorError> {
        if self.session_type == SessionType::Home {
            return Err(crate::error::AdvisorError::CannotModifyHomeSession);
        }
        self.is_archived = false;
        Ok(())
    }

    /// Whether this session can accept new thoughts.
    pub fn is_active(&self) -> bool {
        !self.is_archived
    }

    /// Get a lightweight summary of this session.
    pub fn to_summary(&self) -> SessionSummary {
        SessionSummary {
            id: self.id,
            session_type: self.session_type,
            title: self.title.clone(),
            summary: self.summary.clone(),
            key_topics: self.key_topics.clone(),
            last_active_at: self.last_active_at,
            thought_count: self.thought_count,
            is_archived: self.is_archived,
        }
    }
}

/// A lightweight view of a session for listings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSummary {
    pub id: Uuid,
    pub session_type: SessionType,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub key_topics: Vec<String>,
    pub last_active_at: DateTime<Utc>,
    pub thought_count: usize,
    pub is_archived: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_session_creation() {
        let session = Session::home();
        assert_eq!(session.session_type, SessionType::Home);
        assert!(session.title.is_none());
        assert!(!session.is_archived);
        assert_eq!(session.thought_count, 0);
    }

    #[test]
    fn user_session_creation() {
        let session = Session::user("Design discussion");
        assert_eq!(session.session_type, SessionType::User);
        assert_eq!(session.title.as_deref(), Some("Design discussion"));
    }

    #[test]
    fn add_thought_increments_and_touches() {
        let mut session = Session::user("test");
        let old_time = session.last_active_at;
        // Sleep would be needed for a real time check, but we verify the count
        session.add_thought();
        assert_eq!(session.thought_count, 1);
        assert!(session.last_active_at >= old_time);
    }

    #[test]
    fn archive_user_session() {
        let mut session = Session::user("old chat");
        assert!(session.archive().is_ok());
        assert!(session.is_archived);
        assert!(!session.is_active());
    }

    #[test]
    fn cannot_archive_home_session() {
        let mut session = Session::home();
        assert!(session.archive().is_err());
    }

    #[test]
    fn unarchive_session() {
        let mut session = Session::user("paused");
        session.archive().unwrap();
        session.unarchive().unwrap();
        assert!(!session.is_archived);
        assert!(session.is_active());
    }

    #[test]
    fn session_summary_conversion() {
        let mut session = Session::user("topic");
        session.thought_count = 5;
        session.key_topics = vec!["rust".into(), "design".into()];
        let summary = session.to_summary();
        assert_eq!(summary.id, session.id);
        assert_eq!(summary.thought_count, 5);
        assert_eq!(summary.key_topics.len(), 2);
    }

    #[test]
    fn session_serialization_roundtrip() {
        let session = Session::user("test");
        let json = serde_json::to_string(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(session, deserialized);
    }
}
