use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Cart item with price snapshot at time of addition.
///
/// The price snapshot captures what the buyer saw when they added the item.
/// The actual checkout price is verified against the current listing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CartItem {
    /// Reference to the product .idea.
    pub product_ref: String,
    /// Optional variant selection.
    pub variant_id: Option<Uuid>,
    pub quantity: u32,
    pub seller_pubkey: String,
    /// Price in Cool cents at time of adding.
    pub price_snapshot_cents: i64,
    pub added_at: DateTime<Utc>,
}

/// Local shopping cart. Lives in Vault, never on Globe.
///
/// From Consortium Art. 2 §2: Cart is consent-gated — sellers never see it
/// until the buyer initiates checkout.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Cart {
    items: Vec<CartItem>,
}

/// Actions that can be applied to a cart.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CartAction {
    /// Add an item to the cart.
    Add(CartItem),
    /// Remove an item by product reference.
    Remove { product_ref: String },
    /// Update quantity for an item.
    UpdateQuantity { product_ref: String, quantity: u32 },
    /// Clear the entire cart.
    Clear,
}

/// Push-to-cart suggestion from a storefront.
///
/// Requires Polity consent before being acted upon.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CartSuggestion {
    pub storefront_id: String,
    pub product_ref: String,
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl Cart {
    /// Create an empty cart.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Apply an action to the cart.
    pub fn apply(&mut self, action: CartAction) {
        match action {
            CartAction::Add(item) => {
                // If the same product+variant is already in the cart, update quantity
                if let Some(existing) = self.items.iter_mut().find(|i| {
                    i.product_ref == item.product_ref && i.variant_id == item.variant_id
                }) {
                    existing.quantity += item.quantity;
                    existing.price_snapshot_cents = item.price_snapshot_cents;
                } else {
                    self.items.push(item);
                }
            }
            CartAction::Remove { product_ref } => {
                self.items.retain(|i| i.product_ref != product_ref);
            }
            CartAction::UpdateQuantity {
                product_ref,
                quantity,
            } => {
                if quantity == 0 {
                    self.items.retain(|i| i.product_ref != product_ref);
                } else if let Some(item) =
                    self.items.iter_mut().find(|i| i.product_ref == product_ref)
                {
                    item.quantity = quantity;
                }
            }
            CartAction::Clear => {
                self.items.clear();
            }
        }
    }

    /// All items in the cart.
    pub fn items(&self) -> &[CartItem] {
        &self.items
    }

    /// Total price in Cool cents.
    pub fn total_cents(&self) -> i64 {
        self.items
            .iter()
            .map(|i| i.price_snapshot_cents * i.quantity as i64)
            .sum()
    }

    /// Total number of items (sum of quantities).
    pub fn item_count(&self) -> usize {
        self.items.iter().map(|i| i.quantity as usize).sum()
    }

    /// Whether the cart is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Find an item by product reference.
    pub fn find_item(&self, product_ref: &str) -> Option<&CartItem> {
        self.items.iter().find(|i| i.product_ref == product_ref)
    }

    /// Unique seller pubkeys in the cart.
    pub fn sellers(&self) -> Vec<&str> {
        let mut sellers: Vec<&str> = self
            .items
            .iter()
            .map(|i| i.seller_pubkey.as_str())
            .collect();
        sellers.sort_unstable();
        sellers.dedup();
        sellers
    }

    /// Items from a specific seller.
    pub fn items_for_seller(&self, seller: &str) -> Vec<&CartItem> {
        self.items
            .iter()
            .filter(|i| i.seller_pubkey == seller)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_item(product_ref: &str, seller: &str, price: i64, quantity: u32) -> CartItem {
        CartItem {
            product_ref: product_ref.into(),
            variant_id: None,
            quantity,
            seller_pubkey: seller.into(),
            price_snapshot_cents: price,
            added_at: Utc::now(),
        }
    }

    #[test]
    fn empty_cart() {
        let cart = Cart::new();
        assert!(cart.is_empty());
        assert_eq!(cart.total_cents(), 0);
        assert_eq!(cart.item_count(), 0);
    }

    #[test]
    fn add_items() {
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 2)));
        cart.apply(CartAction::Add(test_item("prod-2", "seller-b", 500, 1)));

        assert!(!cart.is_empty());
        assert_eq!(cart.items().len(), 2);
        assert_eq!(cart.total_cents(), 2500); // 1000*2 + 500*1
        assert_eq!(cart.item_count(), 3); // 2 + 1
    }

    #[test]
    fn add_duplicate_merges_quantity() {
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 2)));
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 3)));

        assert_eq!(cart.items().len(), 1);
        assert_eq!(cart.items()[0].quantity, 5);
    }

    #[test]
    fn remove_item() {
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 1)));
        cart.apply(CartAction::Add(test_item("prod-2", "seller-b", 500, 1)));

        cart.apply(CartAction::Remove {
            product_ref: "prod-1".into(),
        });

        assert_eq!(cart.items().len(), 1);
        assert_eq!(cart.items()[0].product_ref, "prod-2");
    }

    #[test]
    fn update_quantity() {
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 1)));

        cart.apply(CartAction::UpdateQuantity {
            product_ref: "prod-1".into(),
            quantity: 5,
        });

        assert_eq!(cart.items()[0].quantity, 5);
        assert_eq!(cart.total_cents(), 5000);
    }

    #[test]
    fn update_quantity_to_zero_removes() {
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 3)));

        cart.apply(CartAction::UpdateQuantity {
            product_ref: "prod-1".into(),
            quantity: 0,
        });

        assert!(cart.is_empty());
    }

    #[test]
    fn clear_cart() {
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 1)));
        cart.apply(CartAction::Add(test_item("prod-2", "seller-b", 500, 1)));

        cart.apply(CartAction::Clear);
        assert!(cart.is_empty());
        assert_eq!(cart.total_cents(), 0);
    }

    #[test]
    fn find_item() {
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 2)));

        let found = cart.find_item("prod-1");
        assert!(found.is_some());
        assert_eq!(found.unwrap().quantity, 2);

        assert!(cart.find_item("prod-nonexistent").is_none());
    }

    #[test]
    fn seller_grouping() {
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 1)));
        cart.apply(CartAction::Add(test_item("prod-2", "seller-a", 500, 1)));
        cart.apply(CartAction::Add(test_item("prod-3", "seller-b", 800, 1)));

        let sellers = cart.sellers();
        assert_eq!(sellers.len(), 2);
        assert!(sellers.contains(&"seller-a"));
        assert!(sellers.contains(&"seller-b"));

        let seller_a_items = cart.items_for_seller("seller-a");
        assert_eq!(seller_a_items.len(), 2);

        let seller_b_items = cart.items_for_seller("seller-b");
        assert_eq!(seller_b_items.len(), 1);
    }

    #[test]
    fn cart_serde_round_trip() {
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 2)));
        cart.apply(CartAction::Add(test_item("prod-2", "seller-b", 500, 1)));

        let json = serde_json::to_string(&cart).unwrap();
        let deserialized: Cart = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.items().len(), 2);
        assert_eq!(deserialized.total_cents(), 2500);
    }

    #[test]
    fn cart_suggestion_serde() {
        let suggestion = CartSuggestion {
            storefront_id: "store-001".into(),
            product_ref: "prod-xyz".into(),
            message: Some("You might like this!".into()),
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&suggestion).unwrap();
        let deserialized: CartSuggestion = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.storefront_id, "store-001");
        assert_eq!(deserialized.product_ref, "prod-xyz");
        assert_eq!(
            deserialized.message.as_deref(),
            Some("You might like this!")
        );
    }
}
