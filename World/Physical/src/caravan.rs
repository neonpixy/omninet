//! Delivery orchestration.
//!
//! Caravan choreographs Place, Rendezvous, Handoff, Lantern, and OmniTag into
//! a coherent delivery workflow. A Delivery tracks a package from sender to
//! recipient, with optional courier assignment, tracker attachment, and
//! cash-on-delivery support.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::PhysicalError;

// TrackerType is defined in omnitag; re-export here so lib.rs can
// `pub use caravan::{..., TrackerType}`.
pub use crate::omnitag::TrackerType;

// ---------------------------------------------------------------------------
// MARK: - DeliveryItem
// ---------------------------------------------------------------------------

/// A single item in a delivery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeliveryItem {
    pub description: String,
    pub quantity: u32,
    /// Optional Idea or Redemption ID linking to a Fortune/Ideas entity.
    pub reference_id: Option<String>,
}

impl DeliveryItem {
    pub fn new(description: impl Into<String>, quantity: u32) -> Self {
        Self {
            description: description.into(),
            quantity,
            reference_id: None,
        }
    }

    pub fn with_reference(mut self, id: impl Into<String>) -> Self {
        self.reference_id = Some(id.into());
        self
    }
}

// ---------------------------------------------------------------------------
// MARK: - DeliveryStatus
// ---------------------------------------------------------------------------

/// Lifecycle state of a delivery.
///
/// ```text
/// Created → CourierAssigned → PickedUp → InTransit → NearDestination → Delivered → Confirmed
///                                                                       ↓            ↓
///                                                                    Disputed     Disputed
/// (any non-Confirmed) → Cancelled
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeliveryStatus {
    Created,
    CourierAssigned,
    PickedUp,
    InTransit,
    NearDestination,
    Delivered,
    Confirmed,
    Disputed,
    Cancelled,
}

impl std::fmt::Display for DeliveryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Created => "Created",
            Self::CourierAssigned => "CourierAssigned",
            Self::PickedUp => "PickedUp",
            Self::InTransit => "InTransit",
            Self::NearDestination => "NearDestination",
            Self::Delivered => "Delivered",
            Self::Confirmed => "Confirmed",
            Self::Disputed => "Disputed",
            Self::Cancelled => "Cancelled",
        };
        write!(f, "{}", s)
    }
}

// ---------------------------------------------------------------------------
// MARK: - DeliveryLeg
// ---------------------------------------------------------------------------

/// A leg of a multi-hop delivery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeliveryLeg {
    pub from_place: Option<Uuid>,
    pub to_place: Option<Uuid>,
    pub courier: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// MARK: - TrackerAttachment
// ---------------------------------------------------------------------------

/// A physical tracker attached to a delivery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackerAttachment {
    pub tracker_id: String,
    pub tracker_type: TrackerType,
    pub attached_at: DateTime<Utc>,
    pub detached_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// MARK: - Delivery
// ---------------------------------------------------------------------------

/// A delivery orchestrating the movement of items from sender to recipient.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Delivery {
    pub id: Uuid,
    /// Sender's crown_id.
    pub sender: String,
    /// Courier's crown_id (assigned after creation).
    pub courier: Option<String>,
    /// Recipient's crown_id.
    pub recipient: String,
    pub description: String,
    pub items: Vec<DeliveryItem>,
    pub pickup_place_id: Option<Uuid>,
    pub dropoff_place_id: Option<Uuid>,
    pub pickup_rendezvous_id: Option<Uuid>,
    pub dropoff_rendezvous_id: Option<Uuid>,
    pub pickup_handoff_id: Option<Uuid>,
    pub delivery_handoff_id: Option<Uuid>,
    pub tracker: Option<TrackerAttachment>,
    pub status: DeliveryStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    /// Cash serial number for cash-on-delivery payments.
    pub cash_serial: Option<String>,
}

impl Delivery {
    /// Create a new delivery in `Created` status.
    pub fn new(
        sender: impl Into<String>,
        recipient: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            sender: sender.into(),
            courier: None,
            recipient: recipient.into(),
            description: description.into(),
            items: Vec::new(),
            pickup_place_id: None,
            dropoff_place_id: None,
            pickup_rendezvous_id: None,
            dropoff_rendezvous_id: None,
            pickup_handoff_id: None,
            delivery_handoff_id: None,
            tracker: None,
            status: DeliveryStatus::Created,
            created_at: now,
            updated_at: now,
            completed_at: None,
            notes: None,
            cash_serial: None,
        }
    }

    // -- Builder methods ----------------------------------------------------

    /// Attach items to the delivery.
    pub fn with_items(mut self, items: Vec<DeliveryItem>) -> Self {
        self.items = items;
        self
    }

    /// Set the pickup place.
    pub fn with_pickup(mut self, place_id: Uuid) -> Self {
        self.pickup_place_id = Some(place_id);
        self
    }

    /// Set the dropoff place.
    pub fn with_dropoff(mut self, place_id: Uuid) -> Self {
        self.dropoff_place_id = Some(place_id);
        self
    }

    /// Add delivery notes.
    pub fn with_notes(mut self, text: impl Into<String>) -> Self {
        self.notes = Some(text.into());
        self
    }

    /// Set up cash-on-delivery by attaching a Cash serial number.
    pub fn with_cash_on_delivery(mut self, serial: impl Into<String>) -> Self {
        self.cash_serial = Some(serial.into());
        self
    }

    // -- Lifecycle transitions ----------------------------------------------

    /// Assign a courier. Only valid from `Created`.
    pub fn assign_courier(&mut self, courier_crown_id: impl Into<String>) -> Result<(), PhysicalError> {
        self.require_status(&DeliveryStatus::Created)?;
        self.courier = Some(courier_crown_id.into());
        self.status = DeliveryStatus::CourierAssigned;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Mark the package as picked up. Only valid from `CourierAssigned`.
    pub fn mark_picked_up(&mut self, handoff_id: Uuid) -> Result<(), PhysicalError> {
        self.require_status(&DeliveryStatus::CourierAssigned)?;
        self.pickup_handoff_id = Some(handoff_id);
        self.status = DeliveryStatus::PickedUp;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Mark the delivery as in transit. Only valid from `PickedUp`.
    pub fn mark_in_transit(&mut self) -> Result<(), PhysicalError> {
        self.require_status(&DeliveryStatus::PickedUp)?;
        self.status = DeliveryStatus::InTransit;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Mark the delivery as near destination. Only valid from `InTransit`.
    pub fn mark_near_destination(&mut self) -> Result<(), PhysicalError> {
        self.require_status(&DeliveryStatus::InTransit)?;
        self.status = DeliveryStatus::NearDestination;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Mark the delivery as delivered. Valid from `NearDestination` or `InTransit`.
    pub fn mark_delivered(&mut self, handoff_id: Uuid) -> Result<(), PhysicalError> {
        if self.status != DeliveryStatus::NearDestination
            && self.status != DeliveryStatus::InTransit
        {
            return Err(PhysicalError::InvalidDeliveryState {
                expected: "NearDestination or InTransit".into(),
                actual: self.status.to_string(),
            });
        }
        self.delivery_handoff_id = Some(handoff_id);
        self.status = DeliveryStatus::Delivered;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Confirm delivery. Only the recipient can confirm, and only from `Delivered`.
    pub fn confirm(&mut self, confirmer: &str) -> Result<(), PhysicalError> {
        if confirmer != self.recipient {
            return Err(PhysicalError::Unauthorized(format!(
                "{} is not the recipient",
                confirmer
            )));
        }
        self.require_status(&DeliveryStatus::Delivered)?;
        self.status = DeliveryStatus::Confirmed;
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Dispute a delivery. Valid from `Delivered` or `Confirmed`.
    pub fn dispute(&mut self, _reason: &str) -> Result<(), PhysicalError> {
        if self.status != DeliveryStatus::Delivered && self.status != DeliveryStatus::Confirmed {
            return Err(PhysicalError::InvalidDeliveryState {
                expected: "Delivered or Confirmed".into(),
                actual: self.status.to_string(),
            });
        }
        self.status = DeliveryStatus::Disputed;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Cancel the delivery. Valid from any state except `Confirmed`.
    pub fn cancel(&mut self, _canceller: &str) -> Result<(), PhysicalError> {
        if self.status == DeliveryStatus::Confirmed {
            return Err(PhysicalError::InvalidDeliveryState {
                expected: "any state except Confirmed".into(),
                actual: "Confirmed".into(),
            });
        }
        self.status = DeliveryStatus::Cancelled;
        self.updated_at = Utc::now();
        Ok(())
    }

    // -- Tracker attachment -------------------------------------------------

    /// Attach a physical tracker to this delivery.
    pub fn attach_tracker(
        &mut self,
        tracker_id: impl Into<String>,
        tracker_type: TrackerType,
    ) {
        self.tracker = Some(TrackerAttachment {
            tracker_id: tracker_id.into(),
            tracker_type,
            attached_at: Utc::now(),
            detached_at: None,
        });
        self.updated_at = Utc::now();
    }

    /// Detach the tracker from this delivery.
    pub fn detach_tracker(&mut self) {
        if let Some(ref mut t) = self.tracker {
            t.detached_at = Some(Utc::now());
        }
        self.updated_at = Utc::now();
    }

    // -- Queries ------------------------------------------------------------

    /// Whether the delivery has reached a terminal state (Confirmed or Cancelled).
    pub fn is_complete(&self) -> bool {
        matches!(
            self.status,
            DeliveryStatus::Confirmed | DeliveryStatus::Cancelled
        )
    }

    // -- Helpers ------------------------------------------------------------

    fn require_status(&self, expected: &DeliveryStatus) -> Result<(), PhysicalError> {
        if self.status != *expected {
            return Err(PhysicalError::InvalidDeliveryState {
                expected: expected.to_string(),
                actual: self.status.to_string(),
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MARK: - Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_delivery() -> Delivery {
        Delivery::new("cpub1sender", "cpub1recipient", "A box of books")
    }

    // -- Construction -------------------------------------------------------

    #[test]
    fn new_delivery_defaults() {
        let d = make_delivery();
        assert_eq!(d.sender, "cpub1sender");
        assert_eq!(d.recipient, "cpub1recipient");
        assert_eq!(d.description, "A box of books");
        assert_eq!(d.status, DeliveryStatus::Created);
        assert!(d.courier.is_none());
        assert!(d.items.is_empty());
        assert!(d.pickup_place_id.is_none());
        assert!(d.dropoff_place_id.is_none());
        assert!(d.tracker.is_none());
        assert!(d.notes.is_none());
        assert!(d.cash_serial.is_none());
        assert!(d.completed_at.is_none());
        assert!(!d.is_complete());
    }

    #[test]
    fn delivery_uuid_is_v4() {
        let d = make_delivery();
        assert_eq!(d.id.get_version_num(), 4);
    }

    // -- Builder methods ----------------------------------------------------

    #[test]
    fn builder_methods() {
        let pickup = Uuid::new_v4();
        let dropoff = Uuid::new_v4();
        let items = vec![
            DeliveryItem::new("Widget", 3),
            DeliveryItem::new("Gadget", 1).with_reference("idea-123"),
        ];

        let d = make_delivery()
            .with_items(items.clone())
            .with_pickup(pickup)
            .with_dropoff(dropoff)
            .with_notes("Handle with care")
            .with_cash_on_delivery("CASH-001");

        assert_eq!(d.items.len(), 2);
        assert_eq!(d.items[0].description, "Widget");
        assert_eq!(d.items[0].quantity, 3);
        assert!(d.items[0].reference_id.is_none());
        assert_eq!(d.items[1].reference_id.as_deref(), Some("idea-123"));
        assert_eq!(d.pickup_place_id, Some(pickup));
        assert_eq!(d.dropoff_place_id, Some(dropoff));
        assert_eq!(d.notes.as_deref(), Some("Handle with care"));
        assert_eq!(d.cash_serial.as_deref(), Some("CASH-001"));
    }

    // -- Happy path lifecycle -----------------------------------------------

    #[test]
    fn full_happy_path() {
        let mut d = make_delivery();

        // Assign courier
        assert!(d.assign_courier("cpub1courier").is_ok());
        assert_eq!(d.status, DeliveryStatus::CourierAssigned);
        assert_eq!(d.courier.as_deref(), Some("cpub1courier"));

        // Pickup
        let pickup_handoff = Uuid::new_v4();
        assert!(d.mark_picked_up(pickup_handoff).is_ok());
        assert_eq!(d.status, DeliveryStatus::PickedUp);
        assert_eq!(d.pickup_handoff_id, Some(pickup_handoff));

        // In transit
        assert!(d.mark_in_transit().is_ok());
        assert_eq!(d.status, DeliveryStatus::InTransit);

        // Near destination
        assert!(d.mark_near_destination().is_ok());
        assert_eq!(d.status, DeliveryStatus::NearDestination);

        // Delivered
        let delivery_handoff = Uuid::new_v4();
        assert!(d.mark_delivered(delivery_handoff).is_ok());
        assert_eq!(d.status, DeliveryStatus::Delivered);
        assert_eq!(d.delivery_handoff_id, Some(delivery_handoff));

        // Confirmed
        assert!(d.confirm("cpub1recipient").is_ok());
        assert_eq!(d.status, DeliveryStatus::Confirmed);
        assert!(d.completed_at.is_some());
        assert!(d.is_complete());
    }

    #[test]
    fn deliver_from_in_transit_skipping_near() {
        let mut d = make_delivery();
        d.assign_courier("cpub1courier").unwrap();
        d.mark_picked_up(Uuid::new_v4()).unwrap();
        d.mark_in_transit().unwrap();

        // Can deliver directly from InTransit (skip NearDestination)
        assert!(d.mark_delivered(Uuid::new_v4()).is_ok());
        assert_eq!(d.status, DeliveryStatus::Delivered);
    }

    // -- Invalid transitions ------------------------------------------------

    #[test]
    fn cannot_assign_courier_twice() {
        let mut d = make_delivery();
        d.assign_courier("cpub1first").unwrap();
        let err = d.assign_courier("cpub1second").unwrap_err();
        assert!(matches!(err, PhysicalError::InvalidDeliveryState { .. }));
    }

    #[test]
    fn cannot_pickup_from_created() {
        let mut d = make_delivery();
        let err = d.mark_picked_up(Uuid::new_v4()).unwrap_err();
        assert!(matches!(err, PhysicalError::InvalidDeliveryState { .. }));
    }

    #[test]
    fn cannot_transit_from_created() {
        let mut d = make_delivery();
        let err = d.mark_in_transit().unwrap_err();
        assert!(matches!(err, PhysicalError::InvalidDeliveryState { .. }));
    }

    #[test]
    fn cannot_deliver_from_created() {
        let mut d = make_delivery();
        let err = d.mark_delivered(Uuid::new_v4()).unwrap_err();
        assert!(matches!(err, PhysicalError::InvalidDeliveryState { .. }));
    }

    #[test]
    fn cannot_near_destination_from_picked_up() {
        let mut d = make_delivery();
        d.assign_courier("cpub1c").unwrap();
        d.mark_picked_up(Uuid::new_v4()).unwrap();
        let err = d.mark_near_destination().unwrap_err();
        assert!(matches!(err, PhysicalError::InvalidDeliveryState { .. }));
    }

    // -- Confirm / Dispute / Cancel -----------------------------------------

    #[test]
    fn only_recipient_can_confirm() {
        let mut d = make_delivery();
        d.assign_courier("cpub1courier").unwrap();
        d.mark_picked_up(Uuid::new_v4()).unwrap();
        d.mark_in_transit().unwrap();
        d.mark_delivered(Uuid::new_v4()).unwrap();

        let err = d.confirm("cpub1sender").unwrap_err();
        assert!(matches!(err, PhysicalError::Unauthorized(_)));

        assert!(d.confirm("cpub1recipient").is_ok());
    }

    #[test]
    fn dispute_from_delivered() {
        let mut d = make_delivery();
        d.assign_courier("cpub1c").unwrap();
        d.mark_picked_up(Uuid::new_v4()).unwrap();
        d.mark_in_transit().unwrap();
        d.mark_delivered(Uuid::new_v4()).unwrap();

        assert!(d.dispute("Wrong items").is_ok());
        assert_eq!(d.status, DeliveryStatus::Disputed);
    }

    #[test]
    fn dispute_from_confirmed() {
        let mut d = make_delivery();
        d.assign_courier("cpub1c").unwrap();
        d.mark_picked_up(Uuid::new_v4()).unwrap();
        d.mark_in_transit().unwrap();
        d.mark_delivered(Uuid::new_v4()).unwrap();
        d.confirm("cpub1recipient").unwrap();

        assert!(d.dispute("Damaged").is_ok());
        assert_eq!(d.status, DeliveryStatus::Disputed);
    }

    #[test]
    fn cannot_dispute_from_created() {
        let mut d = make_delivery();
        let err = d.dispute("No reason").unwrap_err();
        assert!(matches!(err, PhysicalError::InvalidDeliveryState { .. }));
    }

    #[test]
    fn cancel_from_created() {
        let mut d = make_delivery();
        assert!(d.cancel("cpub1sender").is_ok());
        assert_eq!(d.status, DeliveryStatus::Cancelled);
        assert!(d.is_complete());
    }

    #[test]
    fn cancel_from_in_transit() {
        let mut d = make_delivery();
        d.assign_courier("cpub1c").unwrap();
        d.mark_picked_up(Uuid::new_v4()).unwrap();
        d.mark_in_transit().unwrap();

        assert!(d.cancel("cpub1sender").is_ok());
        assert_eq!(d.status, DeliveryStatus::Cancelled);
    }

    #[test]
    fn cannot_cancel_confirmed() {
        let mut d = make_delivery();
        d.assign_courier("cpub1c").unwrap();
        d.mark_picked_up(Uuid::new_v4()).unwrap();
        d.mark_in_transit().unwrap();
        d.mark_delivered(Uuid::new_v4()).unwrap();
        d.confirm("cpub1recipient").unwrap();

        let err = d.cancel("cpub1sender").unwrap_err();
        assert!(matches!(err, PhysicalError::InvalidDeliveryState { .. }));
    }

    // -- Tracker attachment -------------------------------------------------

    #[test]
    fn attach_and_detach_tracker() {
        let mut d = make_delivery();
        d.attach_tracker("tag-001", TrackerType::OmniTag);

        let t = d.tracker.as_ref().unwrap();
        assert_eq!(t.tracker_id, "tag-001");
        assert_eq!(t.tracker_type, TrackerType::OmniTag);
        assert!(t.detached_at.is_none());

        d.detach_tracker();
        let t = d.tracker.as_ref().unwrap();
        assert!(t.detached_at.is_some());
    }

    #[test]
    fn detach_without_tracker_is_harmless() {
        let mut d = make_delivery();
        d.detach_tracker(); // no panic
        assert!(d.tracker.is_none());
    }

    // -- Serde round-trip ---------------------------------------------------

    #[test]
    fn serde_round_trip() {
        let mut d = make_delivery()
            .with_items(vec![DeliveryItem::new("Book", 2)])
            .with_pickup(Uuid::new_v4())
            .with_dropoff(Uuid::new_v4())
            .with_notes("Fragile")
            .with_cash_on_delivery("CASH-42");

        d.assign_courier("cpub1courier").unwrap();
        d.attach_tracker("tag-99", TrackerType::AppleAirTag);

        let json = serde_json::to_string(&d).unwrap();
        let parsed: Delivery = serde_json::from_str(&json).unwrap();
        assert_eq!(d, parsed);
    }
}
