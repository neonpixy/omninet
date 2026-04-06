use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A hearing in a dispute process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HearingRecord {
    pub id: Uuid,
    pub dispute_id: Uuid,
    pub scheduled_at: DateTime<Utc>,
    pub format: HearingFormat,
    pub participants: Vec<HearingParticipant>,
    pub notes: Option<String>,
    pub occurred_at: Option<DateTime<Utc>>,
    pub duration_minutes: Option<u32>,
}

impl HearingRecord {
    pub fn new(dispute_id: Uuid, scheduled_at: DateTime<Utc>, format: HearingFormat) -> Self {
        Self {
            id: Uuid::new_v4(),
            dispute_id,
            scheduled_at,
            format,
            participants: Vec::new(),
            notes: None,
            occurred_at: None,
            duration_minutes: None,
        }
    }

    /// Add a participant to the hearing.
    pub fn add_participant(&mut self, participant: HearingParticipant) {
        self.participants.push(participant);
    }

    /// Mark the hearing as completed with notes and duration.
    pub fn complete(&mut self, notes: impl Into<String>, duration_minutes: u32) {
        self.notes = Some(notes.into());
        self.occurred_at = Some(Utc::now());
        self.duration_minutes = Some(duration_minutes);
    }

    /// Whether the hearing has been completed.
    pub fn is_completed(&self) -> bool {
        self.occurred_at.is_some()
    }
}

/// Format of the hearing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum HearingFormat {
    /// Parties submit written briefs only.
    Written,
    /// Real-time text-based hearing (chat).
    TextBased,
    /// Voice-only hearing.
    Audio,
    /// Video conference hearing.
    Video,
    /// Physical, face-to-face hearing.
    InPerson,
}

/// A participant in a hearing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HearingParticipant {
    pub pubkey: String,
    pub role: ParticipantRole,
    pub attended: bool,
    pub notes: Option<String>,
}

/// Role in a hearing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ParticipantRole {
    /// The party who filed the dispute.
    Complainant,
    /// The party the dispute was filed against.
    Respondent,
    /// The person(s) hearing and deciding the case.
    Adjudicator,
    /// A third party providing testimony.
    Witness,
    /// A representative speaking on behalf of a party.
    Advocate,
    /// Present but not participating in the proceedings.
    Observer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hearing_lifecycle() {
        let mut hearing = HearingRecord::new(
            Uuid::new_v4(),
            Utc::now() + chrono::Duration::days(7),
            HearingFormat::Video,
        );
        assert!(!hearing.is_completed());

        hearing.add_participant(HearingParticipant {
            pubkey: "alice".into(),
            role: ParticipantRole::Complainant,
            attended: true,
            notes: None,
        });
        hearing.add_participant(HearingParticipant {
            pubkey: "bob".into(),
            role: ParticipantRole::Respondent,
            attended: true,
            notes: None,
        });

        hearing.complete("Both parties presented their case", 90);
        assert!(hearing.is_completed());
        assert_eq!(hearing.duration_minutes, Some(90));
        assert_eq!(hearing.participants.len(), 2);
    }
}
