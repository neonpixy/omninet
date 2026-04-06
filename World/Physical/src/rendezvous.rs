//! Meetup coordination for physical-world gatherings.
//!
//! Rendezvous enables sovereign participants to organize in-person meetings
//! for socializing, cash exchanges, identity verification, community events,
//! and commerce. All participation is voluntary and opt-in.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use x::GeoCoordinate;

use crate::error::PhysicalError;

// ---------------------------------------------------------------------------
// MARK: - Types
// ---------------------------------------------------------------------------

/// A scheduled physical meetup between participants.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rendezvous {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    /// Organizer's crown_id.
    pub organizer: String,
    /// Optional link to a `Place`.
    pub place_id: Option<Uuid>,
    /// Explicit geographic coordinate.
    pub location: Option<GeoCoordinate>,
    /// Human-readable location name (e.g. "Cheesman Park pavilion").
    pub location_name: Option<String>,
    pub scheduled_at: DateTime<Utc>,
    pub duration_minutes: Option<u32>,
    pub purpose: RendezvousPurpose,
    /// Invited crown IDs.
    pub invitees: Vec<String>,
    pub rsvps: Vec<Rsvp>,
    pub status: RendezvousStatus,
    pub max_participants: Option<u32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Why the rendezvous is being organized.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RendezvousPurpose {
    Social,
    CashExchange,
    Verification,
    CommunityEvent,
    Commerce,
    Custom(String),
}

/// A single participant's RSVP to a rendezvous.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rsvp {
    /// Participant crown_id.
    pub person: String,
    pub response: RsvpResponse,
    pub message: Option<String>,
    pub responded_at: DateTime<Utc>,
}

/// RSVP response options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RsvpResponse {
    Attending,
    Maybe,
    Declined,
}

/// Lifecycle state of a rendezvous.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RendezvousStatus {
    Proposed,
    Confirmed,
    InProgress,
    Completed,
    Cancelled,
}

// ---------------------------------------------------------------------------
// MARK: - Construction
// ---------------------------------------------------------------------------

impl Rendezvous {
    /// Create a new rendezvous in `Proposed` status.
    pub fn new(
        title: impl Into<String>,
        organizer: impl Into<String>,
        scheduled_at: DateTime<Utc>,
        purpose: RendezvousPurpose,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            description: None,
            organizer: organizer.into(),
            place_id: None,
            location: None,
            location_name: None,
            scheduled_at,
            duration_minutes: None,
            purpose,
            invitees: Vec::new(),
            rsvps: Vec::new(),
            status: RendezvousStatus::Proposed,
            max_participants: None,
            created_at: now,
            updated_at: now,
        }
    }

    // -- Builder methods --

    /// Attach to an existing `Place`.
    pub fn with_place(mut self, place_id: Uuid) -> Self {
        self.place_id = Some(place_id);
        self
    }

    /// Set explicit coordinates and a human-readable location name.
    pub fn with_location(mut self, coords: GeoCoordinate, name: impl Into<String>) -> Self {
        self.location = Some(coords);
        self.location_name = Some(name.into());
        self
    }

    /// Pre-populate the invite list.
    pub fn with_invitees(mut self, crown_ids: Vec<String>) -> Self {
        self.invitees = crown_ids;
        self
    }

    /// Set expected duration in minutes.
    pub fn with_duration(mut self, minutes: u32) -> Self {
        self.duration_minutes = Some(minutes);
        self
    }

    /// Set maximum participant count.
    pub fn with_max_participants(mut self, n: u32) -> Self {
        self.max_participants = Some(n);
        self
    }

    /// Set a description.
    pub fn with_description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    // -----------------------------------------------------------------------
    // MARK: - Invite management
    // -----------------------------------------------------------------------

    /// Invite a participant (no-op if already invited).
    pub fn invite(&mut self, crown_id: impl Into<String>) {
        let crown_id = crown_id.into();
        if !self.invitees.contains(&crown_id) {
            self.invitees.push(crown_id);
            self.updated_at = Utc::now();
        }
    }

    /// Remove an invite (no-op if not present).
    pub fn uninvite(&mut self, crown_id: &str) {
        let before = self.invitees.len();
        self.invitees.retain(|i| i != crown_id);
        if self.invitees.len() != before {
            self.updated_at = Utc::now();
        }
    }

    /// Whether the given crown_id is on the invite list.
    pub fn is_invited(&self, crown_id: &str) -> bool {
        self.invitees.iter().any(|i| i == crown_id)
    }

    // -----------------------------------------------------------------------
    // MARK: - RSVP
    // -----------------------------------------------------------------------

    /// Record or update an RSVP. Replaces any previous response from the same person.
    pub fn rsvp(
        &mut self,
        person: impl Into<String>,
        response: RsvpResponse,
        message: Option<String>,
    ) {
        let person = person.into();
        self.rsvps.retain(|r| r.person != person);
        self.rsvps.push(Rsvp {
            person,
            response,
            message,
            responded_at: Utc::now(),
        });
        self.updated_at = Utc::now();
    }

    /// Number of participants who responded `Attending`.
    pub fn attending_count(&self) -> usize {
        self.rsvps
            .iter()
            .filter(|r| r.response == RsvpResponse::Attending)
            .count()
    }

    // -----------------------------------------------------------------------
    // MARK: - Lifecycle transitions
    // -----------------------------------------------------------------------

    /// Reschedule to a new time. Only the organizer may do this, and only
    /// if the rendezvous has not been completed or cancelled.
    pub fn reschedule(
        &mut self,
        new_time: DateTime<Utc>,
        updater: &str,
    ) -> Result<(), PhysicalError> {
        self.require_organizer(updater)?;
        self.require_not_finalized()?;
        self.scheduled_at = new_time;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Cancel the rendezvous. Only the organizer may do this.
    pub fn cancel(
        &mut self,
        _reason: Option<String>,
        canceller: &str,
    ) -> Result<(), PhysicalError> {
        self.require_organizer(canceller)?;
        self.require_not_finalized()?;
        self.status = RendezvousStatus::Cancelled;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Move from `Proposed` to `Confirmed`. Only the organizer.
    pub fn confirm(&mut self, updater: &str) -> Result<(), PhysicalError> {
        self.require_organizer(updater)?;
        if self.status != RendezvousStatus::Proposed {
            return Err(PhysicalError::RendezvousAlreadyFinalized(format!(
                "{:?}",
                self.status
            )));
        }
        self.status = RendezvousStatus::Confirmed;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Mark the rendezvous as `Completed`. Only the organizer, and only
    /// if not already cancelled.
    pub fn complete(&mut self, updater: &str) -> Result<(), PhysicalError> {
        self.require_organizer(updater)?;
        if self.status == RendezvousStatus::Completed
            || self.status == RendezvousStatus::Cancelled
        {
            return Err(PhysicalError::RendezvousAlreadyFinalized(format!(
                "{:?}",
                self.status
            )));
        }
        self.status = RendezvousStatus::Completed;
        self.updated_at = Utc::now();
        Ok(())
    }

    // -----------------------------------------------------------------------
    // MARK: - Helpers
    // -----------------------------------------------------------------------

    fn require_organizer(&self, who: &str) -> Result<(), PhysicalError> {
        if self.organizer != who {
            return Err(PhysicalError::NotOrganizer(who.to_string()));
        }
        Ok(())
    }

    fn require_not_finalized(&self) -> Result<(), PhysicalError> {
        match self.status {
            RendezvousStatus::Completed | RendezvousStatus::Cancelled => {
                Err(PhysicalError::RendezvousAlreadyFinalized(format!(
                    "{:?}",
                    self.status
                )))
            }
            _ => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------
// MARK: - Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn future_time() -> DateTime<Utc> {
        Utc::now() + Duration::hours(24)
    }

    fn make_rendezvous() -> Rendezvous {
        Rendezvous::new(
            "Coffee meetup",
            "cpub_organizer",
            future_time(),
            RendezvousPurpose::Social,
        )
    }

    #[test]
    fn new_rendezvous_defaults() {
        let r = make_rendezvous();
        assert_eq!(r.title, "Coffee meetup");
        assert_eq!(r.organizer, "cpub_organizer");
        assert_eq!(r.status, RendezvousStatus::Proposed);
        assert!(r.description.is_none());
        assert!(r.place_id.is_none());
        assert!(r.location.is_none());
        assert!(r.location_name.is_none());
        assert!(r.duration_minutes.is_none());
        assert!(r.max_participants.is_none());
        assert!(r.invitees.is_empty());
        assert!(r.rsvps.is_empty());
    }

    #[test]
    fn builder_methods() {
        let place = Uuid::new_v4();
        let coords = GeoCoordinate::new(39.7392, -104.9903).unwrap();
        let r = make_rendezvous()
            .with_place(place)
            .with_location(coords, "Cheesman Park")
            .with_invitees(vec!["cpub_alice".into(), "cpub_bob".into()])
            .with_duration(60)
            .with_max_participants(10)
            .with_description("Bring your own mug");

        assert_eq!(r.place_id, Some(place));
        assert_eq!(r.location, Some(coords));
        assert_eq!(r.location_name.as_deref(), Some("Cheesman Park"));
        assert_eq!(r.invitees.len(), 2);
        assert_eq!(r.duration_minutes, Some(60));
        assert_eq!(r.max_participants, Some(10));
        assert_eq!(r.description.as_deref(), Some("Bring your own mug"));
    }

    #[test]
    fn invite_and_uninvite() {
        let mut r = make_rendezvous();
        r.invite("cpub_alice");
        r.invite("cpub_bob");
        assert!(r.is_invited("cpub_alice"));
        assert!(r.is_invited("cpub_bob"));
        assert!(!r.is_invited("cpub_charlie"));

        // duplicate invite is a no-op
        r.invite("cpub_alice");
        assert_eq!(r.invitees.len(), 2);

        r.uninvite("cpub_alice");
        assert!(!r.is_invited("cpub_alice"));
        assert_eq!(r.invitees.len(), 1);
    }

    #[test]
    fn uninvite_unknown_is_noop() {
        let mut r = make_rendezvous();
        let before = r.updated_at;
        r.uninvite("cpub_nobody");
        // updated_at should not change
        assert_eq!(r.updated_at, before);
    }

    #[test]
    fn rsvp_attending() {
        let mut r = make_rendezvous();
        r.rsvp("cpub_alice", RsvpResponse::Attending, None);
        assert_eq!(r.attending_count(), 1);
        assert_eq!(r.rsvps.len(), 1);
        assert_eq!(r.rsvps[0].person, "cpub_alice");
        assert_eq!(r.rsvps[0].response, RsvpResponse::Attending);
    }

    #[test]
    fn rsvp_replaces_previous() {
        let mut r = make_rendezvous();
        r.rsvp("cpub_alice", RsvpResponse::Attending, None);
        assert_eq!(r.attending_count(), 1);

        r.rsvp(
            "cpub_alice",
            RsvpResponse::Declined,
            Some("Can't make it".into()),
        );
        assert_eq!(r.attending_count(), 0);
        assert_eq!(r.rsvps.len(), 1);
        assert_eq!(r.rsvps[0].response, RsvpResponse::Declined);
        assert_eq!(r.rsvps[0].message.as_deref(), Some("Can't make it"));
    }

    #[test]
    fn rsvp_multiple_people() {
        let mut r = make_rendezvous();
        r.rsvp("cpub_alice", RsvpResponse::Attending, None);
        r.rsvp("cpub_bob", RsvpResponse::Maybe, None);
        r.rsvp("cpub_charlie", RsvpResponse::Attending, None);
        assert_eq!(r.attending_count(), 2);
        assert_eq!(r.rsvps.len(), 3);
    }

    #[test]
    fn confirm_from_proposed() {
        let mut r = make_rendezvous();
        assert!(r.confirm("cpub_organizer").is_ok());
        assert_eq!(r.status, RendezvousStatus::Confirmed);
    }

    #[test]
    fn confirm_not_organizer() {
        let mut r = make_rendezvous();
        let err = r.confirm("cpub_intruder").unwrap_err();
        assert!(matches!(err, PhysicalError::NotOrganizer(_)));
    }

    #[test]
    fn confirm_only_from_proposed() {
        let mut r = make_rendezvous();
        r.confirm("cpub_organizer").unwrap();
        // Already confirmed -- cannot confirm again
        let err = r.confirm("cpub_organizer").unwrap_err();
        assert!(matches!(err, PhysicalError::RendezvousAlreadyFinalized(_)));
    }

    #[test]
    fn complete_from_confirmed() {
        let mut r = make_rendezvous();
        r.confirm("cpub_organizer").unwrap();
        assert!(r.complete("cpub_organizer").is_ok());
        assert_eq!(r.status, RendezvousStatus::Completed);
    }

    #[test]
    fn complete_from_proposed() {
        let mut r = make_rendezvous();
        // Completing from Proposed is allowed (small meetup, skip confirm)
        assert!(r.complete("cpub_organizer").is_ok());
        assert_eq!(r.status, RendezvousStatus::Completed);
    }

    #[test]
    fn cannot_complete_cancelled() {
        let mut r = make_rendezvous();
        r.cancel(None, "cpub_organizer").unwrap();
        let err = r.complete("cpub_organizer").unwrap_err();
        assert!(matches!(err, PhysicalError::RendezvousAlreadyFinalized(_)));
    }

    #[test]
    fn cancel() {
        let mut r = make_rendezvous();
        assert!(r.cancel(Some("Rain".into()), "cpub_organizer").is_ok());
        assert_eq!(r.status, RendezvousStatus::Cancelled);
    }

    #[test]
    fn cancel_not_organizer() {
        let mut r = make_rendezvous();
        let err = r.cancel(None, "cpub_intruder").unwrap_err();
        assert!(matches!(err, PhysicalError::NotOrganizer(_)));
    }

    #[test]
    fn cannot_cancel_completed() {
        let mut r = make_rendezvous();
        r.complete("cpub_organizer").unwrap();
        let err = r.cancel(None, "cpub_organizer").unwrap_err();
        assert!(matches!(err, PhysicalError::RendezvousAlreadyFinalized(_)));
    }

    #[test]
    fn reschedule() {
        let mut r = make_rendezvous();
        let new_time = Utc::now() + Duration::hours(48);
        assert!(r.reschedule(new_time, "cpub_organizer").is_ok());
        assert_eq!(r.scheduled_at, new_time);
    }

    #[test]
    fn reschedule_not_organizer() {
        let mut r = make_rendezvous();
        let new_time = Utc::now() + Duration::hours(48);
        let err = r.reschedule(new_time, "cpub_intruder").unwrap_err();
        assert!(matches!(err, PhysicalError::NotOrganizer(_)));
    }

    #[test]
    fn cannot_reschedule_cancelled() {
        let mut r = make_rendezvous();
        r.cancel(None, "cpub_organizer").unwrap();
        let new_time = Utc::now() + Duration::hours(48);
        let err = r.reschedule(new_time, "cpub_organizer").unwrap_err();
        assert!(matches!(err, PhysicalError::RendezvousAlreadyFinalized(_)));
    }

    #[test]
    fn serde_round_trip() {
        let coords = GeoCoordinate::new(39.7392, -104.9903).unwrap();
        let mut r = make_rendezvous()
            .with_location(coords, "Downtown")
            .with_duration(90)
            .with_description("Planning session");
        r.rsvp("cpub_alice", RsvpResponse::Attending, Some("Excited!".into()));

        let json = serde_json::to_string(&r).unwrap();
        let parsed: Rendezvous = serde_json::from_str(&json).unwrap();
        assert_eq!(r, parsed);
    }

    #[test]
    fn purpose_variants() {
        // Ensure all purpose variants serialize correctly
        let purposes = vec![
            RendezvousPurpose::Social,
            RendezvousPurpose::CashExchange,
            RendezvousPurpose::Verification,
            RendezvousPurpose::CommunityEvent,
            RendezvousPurpose::Commerce,
            RendezvousPurpose::Custom("Hackathon".into()),
        ];
        for p in &purposes {
            let json = serde_json::to_string(p).unwrap();
            let parsed: RendezvousPurpose = serde_json::from_str(&json).unwrap();
            assert_eq!(*p, parsed);
        }
    }
}
