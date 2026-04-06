use chrono::{DateTime, Utc};
use ideas::commerce::OrderStatus;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::FortuneError;

/// An order representing a purchase between buyer and seller.
///
/// From Consortium Art. 2 §2: "Fair compensation, consent-based agreements,
/// and transparency in value flows shall be required."
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    pub items: Vec<OrderItem>,
    pub total_cents: i64,
    pub status: OrderStatus,
    /// Fortune Ledger transaction reference.
    pub payment_ref: Option<Uuid>,
    /// Caravan delivery reference (physical goods).
    pub delivery_ref: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Order {
    /// Create a new order. Status starts at Placed, total computed from items.
    pub fn new(
        buyer: impl Into<String>,
        seller: impl Into<String>,
        items: Vec<OrderItem>,
    ) -> Self {
        let total_cents = items
            .iter()
            .map(|i| i.unit_price_cents * i.quantity as i64)
            .sum();
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            buyer_pubkey: buyer.into(),
            seller_pubkey: seller.into(),
            items,
            total_cents,
            status: OrderStatus::Placed,
            payment_ref: None,
            delivery_ref: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Advance the order through its lifecycle:
    /// Placed -> Paid -> Preparing -> Shipped -> Delivered -> Confirmed
    pub fn advance_status(&mut self) -> Result<(), FortuneError> {
        let next = match self.status {
            OrderStatus::Placed => OrderStatus::Paid,
            OrderStatus::Paid => OrderStatus::Preparing,
            OrderStatus::Preparing => OrderStatus::Shipped,
            OrderStatus::Shipped => OrderStatus::Delivered,
            OrderStatus::Delivered => OrderStatus::Confirmed,
            OrderStatus::Confirmed | OrderStatus::Disputed => {
                return Err(FortuneError::InvalidOrderTransition {
                    from: format!("{:?}", self.status),
                    to: "next".into(),
                });
            }
        };
        self.status = next;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Dispute an order (can only dispute from Delivered).
    pub fn dispute(&mut self) -> Result<(), FortuneError> {
        if self.status != OrderStatus::Delivered {
            return Err(FortuneError::InvalidOrderTransition {
                from: format!("{:?}", self.status),
                to: "Disputed".into(),
            });
        }
        self.status = OrderStatus::Disputed;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Set the payment reference (Ledger transaction ID).
    pub fn set_payment_ref(&mut self, payment_ref: Uuid) {
        self.payment_ref = Some(payment_ref);
        self.updated_at = Utc::now();
    }

    /// Set the delivery reference (Caravan delivery ID).
    pub fn set_delivery_ref(&mut self, delivery_ref: Uuid) {
        self.delivery_ref = Some(delivery_ref);
        self.updated_at = Utc::now();
    }

    /// Whether the order is in a terminal state (Confirmed or Disputed).
    pub fn is_complete(&self) -> bool {
        matches!(self.status, OrderStatus::Confirmed | OrderStatus::Disputed)
    }
}

/// A single item within an order.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderItem {
    /// Reference to the product .idea.
    pub product_ref: String,
    /// Optional variant selection.
    pub variant_id: Option<Uuid>,
    pub quantity: u32,
    /// Unit price in Cool cents at time of order.
    pub unit_price_cents: i64,
}

/// A receipt capturing a finalized order with signatures.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub order: Order,
    pub buyer_signature: Option<String>,
    pub seller_signature: Option<String>,
    pub finalized_at: DateTime<Utc>,
}

impl Receipt {
    /// Create a receipt from a completed order. Signatures are added separately.
    pub fn from_order(order: Order) -> Self {
        Self {
            order,
            buyer_signature: None,
            seller_signature: None,
            finalized_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_items() -> Vec<OrderItem> {
        vec![
            OrderItem {
                product_ref: "prod-1".into(),
                variant_id: None,
                quantity: 2,
                unit_price_cents: 1000,
            },
            OrderItem {
                product_ref: "prod-2".into(),
                variant_id: Some(Uuid::new_v4()),
                quantity: 1,
                unit_price_cents: 500,
            },
        ]
    }

    #[test]
    fn create_order() {
        let items = test_items();
        let order = Order::new("cpub1buyer", "cpub1seller", items);

        assert_eq!(order.buyer_pubkey, "cpub1buyer");
        assert_eq!(order.seller_pubkey, "cpub1seller");
        assert_eq!(order.total_cents, 2500); // 1000*2 + 500*1
        assert_eq!(order.status, OrderStatus::Placed);
        assert!(!order.is_complete());
        assert!(order.payment_ref.is_none());
        assert!(order.delivery_ref.is_none());
    }

    #[test]
    fn order_full_lifecycle() {
        let mut order = Order::new("cpub1buyer", "cpub1seller", test_items());

        // Placed -> Paid -> Preparing -> Shipped -> Delivered -> Confirmed
        assert_eq!(order.status, OrderStatus::Placed);

        order.advance_status().unwrap();
        assert_eq!(order.status, OrderStatus::Paid);

        order.advance_status().unwrap();
        assert_eq!(order.status, OrderStatus::Preparing);

        order.advance_status().unwrap();
        assert_eq!(order.status, OrderStatus::Shipped);

        order.advance_status().unwrap();
        assert_eq!(order.status, OrderStatus::Delivered);

        order.advance_status().unwrap();
        assert_eq!(order.status, OrderStatus::Confirmed);
        assert!(order.is_complete());

        // Can't advance past Confirmed
        let err = order.advance_status().unwrap_err();
        assert!(matches!(err, FortuneError::InvalidOrderTransition { .. }));
    }

    #[test]
    fn dispute_from_delivered() {
        let mut order = Order::new("cpub1buyer", "cpub1seller", test_items());

        // Advance to Delivered
        order.advance_status().unwrap(); // Paid
        order.advance_status().unwrap(); // Preparing
        order.advance_status().unwrap(); // Shipped
        order.advance_status().unwrap(); // Delivered

        order.dispute().unwrap();
        assert_eq!(order.status, OrderStatus::Disputed);
        assert!(order.is_complete());
    }

    #[test]
    fn dispute_only_from_delivered() {
        let mut order = Order::new("cpub1buyer", "cpub1seller", test_items());
        // Can't dispute from Placed
        let err = order.dispute().unwrap_err();
        assert!(matches!(err, FortuneError::InvalidOrderTransition { .. }));

        order.advance_status().unwrap(); // Paid
        let err = order.dispute().unwrap_err();
        assert!(matches!(err, FortuneError::InvalidOrderTransition { .. }));
    }

    #[test]
    fn cannot_advance_disputed() {
        let mut order = Order::new("cpub1buyer", "cpub1seller", test_items());
        // Advance to Delivered, then dispute
        for _ in 0..4 {
            order.advance_status().unwrap();
        }
        order.dispute().unwrap();

        let err = order.advance_status().unwrap_err();
        assert!(matches!(err, FortuneError::InvalidOrderTransition { .. }));
    }

    #[test]
    fn set_references() {
        let mut order = Order::new("cpub1buyer", "cpub1seller", test_items());
        let pay_id = Uuid::new_v4();
        let delivery_id = Uuid::new_v4();

        order.set_payment_ref(pay_id);
        assert_eq!(order.payment_ref, Some(pay_id));

        order.set_delivery_ref(delivery_id);
        assert_eq!(order.delivery_ref, Some(delivery_id));
    }

    #[test]
    fn receipt_from_order() {
        let mut order = Order::new("cpub1buyer", "cpub1seller", test_items());
        // Advance to Confirmed
        for _ in 0..5 {
            order.advance_status().unwrap();
        }

        let receipt = Receipt::from_order(order.clone());
        assert_eq!(receipt.order.id, order.id);
        assert_eq!(receipt.order.status, OrderStatus::Confirmed);
        assert!(receipt.buyer_signature.is_none());
        assert!(receipt.seller_signature.is_none());
    }

    #[test]
    fn order_serde_round_trip() {
        let order = Order::new("cpub1buyer", "cpub1seller", test_items());
        let json = serde_json::to_string(&order).unwrap();
        let deserialized: Order = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, order.id);
        assert_eq!(deserialized.total_cents, 2500);
        assert_eq!(deserialized.status, OrderStatus::Placed);
        assert_eq!(deserialized.items.len(), 2);
    }

    #[test]
    fn receipt_serde_round_trip() {
        let order = Order::new("cpub1buyer", "cpub1seller", test_items());
        let receipt = Receipt::from_order(order);

        let json = serde_json::to_string(&receipt).unwrap();
        let deserialized: Receipt = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.order.total_cents, 2500);
    }

    #[test]
    fn empty_order() {
        let order = Order::new("cpub1buyer", "cpub1seller", Vec::new());
        assert_eq!(order.total_cents, 0);
        assert!(order.items.is_empty());
    }
}
