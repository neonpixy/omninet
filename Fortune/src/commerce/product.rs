use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::FortuneError;

/// Business logic wrapper around an Ideas product digit.
///
/// Does NOT duplicate content fields (title, description, etc.) — those live
/// in the .idea digit. This struct handles inventory, variants, and listing state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    /// Reference to the .idea digit containing ProductMeta.
    pub idea_id: String,
    pub seller_pubkey: String,
    pub variants: Vec<ProductVariant>,
    pub inventory: Inventory,
    pub listed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub active: bool,
}

impl Product {
    /// Create a new product linked to an .idea digit.
    pub fn new(idea_id: impl Into<String>, seller_pubkey: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            idea_id: idea_id.into(),
            seller_pubkey: seller_pubkey.into(),
            variants: Vec::new(),
            inventory: Inventory::new(),
            listed_at: now,
            updated_at: now,
            active: true,
        }
    }

    /// Add a variant to this product.
    pub fn add_variant(&mut self, variant: ProductVariant) {
        self.variants.push(variant);
        self.updated_at = Utc::now();
    }

    /// Remove a variant by ID. Returns true if found and removed.
    pub fn remove_variant(&mut self, id: &Uuid) -> bool {
        let before = self.variants.len();
        self.variants.retain(|v| v.id != *id);
        let removed = self.variants.len() < before;
        if removed {
            self.updated_at = Utc::now();
        }
        removed
    }

    /// Available stock (stock minus reserved). None means unlimited.
    pub fn available_stock(&self) -> Option<u32> {
        self.inventory
            .stock
            .map(|s| s.saturating_sub(self.inventory.reserved))
    }

    /// Whether this product is currently in stock.
    pub fn is_in_stock(&self) -> bool {
        self.inventory.is_available(1)
    }

    /// Reserve stock for a pending order.
    pub fn reserve(&mut self, quantity: u32) -> Result<(), FortuneError> {
        if !self.inventory.is_available(quantity) {
            let available = self.available_stock().unwrap_or(0);
            return Err(FortuneError::InsufficientInventory {
                product: self.idea_id.clone(),
                requested: quantity,
                available,
            });
        }
        self.inventory.reserved += quantity;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Release a reservation (e.g., cancelled order).
    pub fn release_reservation(&mut self, quantity: u32) {
        self.inventory.reserved = self.inventory.reserved.saturating_sub(quantity);
        self.updated_at = Utc::now();
    }

    /// Deactivate this product listing.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.updated_at = Utc::now();
    }

    /// Activate this product listing.
    pub fn activate(&mut self) {
        self.active = true;
        self.updated_at = Utc::now();
    }
}

/// A product variant with a price modifier and attributes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductVariant {
    pub id: Uuid,
    pub name: String,
    /// Added to base price (can be negative for discounts).
    pub price_modifier_cents: i64,
    /// e.g., "color" -> "red", "size" -> "L"
    pub attributes: HashMap<String, String>,
    /// Variant-specific inventory, if different from product-level.
    pub inventory: Option<Inventory>,
}

impl ProductVariant {
    /// Create a new variant.
    pub fn new(name: impl Into<String>, price_modifier_cents: i64) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            price_modifier_cents,
            attributes: HashMap::new(),
            inventory: None,
        }
    }

    /// Add an attribute to this variant.
    pub fn with_attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Set variant-specific inventory.
    pub fn with_inventory(mut self, inventory: Inventory) -> Self {
        self.inventory = Some(inventory);
        self
    }
}

/// Inventory tracking for a product or variant.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inventory {
    /// None = unlimited stock.
    pub stock: Option<u32>,
    /// Alert when stock falls below this threshold.
    pub low_stock_threshold: u32,
    /// Units reserved for pending orders.
    pub reserved: u32,
}

impl Inventory {
    /// Unlimited stock inventory.
    pub fn new() -> Self {
        Self {
            stock: None,
            low_stock_threshold: 5,
            reserved: 0,
        }
    }

    /// Inventory with a specific stock count.
    pub fn with_stock(count: u32) -> Self {
        Self {
            stock: Some(count),
            low_stock_threshold: 5,
            reserved: 0,
        }
    }

    /// Whether the requested quantity is available.
    pub fn is_available(&self, quantity: u32) -> bool {
        match self.stock {
            None => true, // unlimited
            Some(stock) => stock.saturating_sub(self.reserved) >= quantity,
        }
    }

    /// Whether current stock is below the low-stock threshold.
    pub fn is_low_stock(&self) -> bool {
        match self.stock {
            None => false, // unlimited is never low
            Some(stock) => stock.saturating_sub(self.reserved) <= self.low_stock_threshold,
        }
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_product() {
        let product = Product::new("idea-001", "cpub1seller");
        assert_eq!(product.idea_id, "idea-001");
        assert_eq!(product.seller_pubkey, "cpub1seller");
        assert!(product.active);
        assert!(product.variants.is_empty());
        assert!(product.is_in_stock()); // unlimited by default
    }

    #[test]
    fn add_and_remove_variants() {
        let mut product = Product::new("idea-001", "cpub1seller");

        let variant = ProductVariant::new("Large Red", 200)
            .with_attribute("color", "red")
            .with_attribute("size", "L");

        let variant_id = variant.id;
        product.add_variant(variant);
        assert_eq!(product.variants.len(), 1);
        assert_eq!(product.variants[0].name, "Large Red");
        assert_eq!(product.variants[0].price_modifier_cents, 200);
        assert_eq!(
            product.variants[0].attributes.get("color"),
            Some(&"red".to_string())
        );

        assert!(product.remove_variant(&variant_id));
        assert!(product.variants.is_empty());

        // Removing nonexistent variant returns false
        assert!(!product.remove_variant(&Uuid::new_v4()));
    }

    #[test]
    fn reserve_and_release_stock() {
        let mut product = Product::new("idea-001", "cpub1seller");
        product.inventory = Inventory::with_stock(10);

        product.reserve(3).unwrap();
        assert_eq!(product.available_stock(), Some(7));
        assert!(product.is_in_stock());

        product.reserve(5).unwrap();
        assert_eq!(product.available_stock(), Some(2));

        // Can't reserve more than available
        let err = product.reserve(5).unwrap_err();
        assert!(matches!(
            err,
            FortuneError::InsufficientInventory {
                requested: 5,
                available: 2,
                ..
            }
        ));

        product.release_reservation(4);
        assert_eq!(product.available_stock(), Some(6));
    }

    #[test]
    fn unlimited_stock() {
        let mut product = Product::new("idea-digital", "cpub1seller");
        // Default is unlimited
        assert!(product.is_in_stock());
        assert_eq!(product.available_stock(), None);

        // Can always reserve unlimited
        product.reserve(1_000_000).unwrap();
        assert!(product.is_in_stock());
    }

    #[test]
    fn deactivate_and_activate() {
        let mut product = Product::new("idea-001", "cpub1seller");
        assert!(product.active);

        product.deactivate();
        assert!(!product.active);

        product.activate();
        assert!(product.active);
    }

    #[test]
    fn inventory_stock_checks() {
        let inv = Inventory::with_stock(10);
        assert!(inv.is_available(10));
        assert!(!inv.is_available(11));
        assert!(!inv.is_low_stock()); // 10 > threshold of 5

        let low_inv = Inventory::with_stock(3);
        assert!(low_inv.is_low_stock()); // 3 <= threshold of 5
    }

    #[test]
    fn inventory_unlimited_never_low() {
        let inv = Inventory::new();
        assert!(!inv.is_low_stock());
        assert!(inv.is_available(u32::MAX));
    }

    #[test]
    fn inventory_reserved_affects_availability() {
        let mut inv = Inventory::with_stock(10);
        inv.reserved = 8;
        assert!(!inv.is_available(5)); // only 2 available
        assert!(inv.is_available(2));
        assert!(inv.is_low_stock()); // 2 <= 5
    }

    #[test]
    fn variant_with_inventory() {
        let variant = ProductVariant::new("Small", -100)
            .with_inventory(Inventory::with_stock(5));

        assert!(variant.inventory.is_some());
        assert!(variant.inventory.as_ref().unwrap().is_available(5));
    }

    #[test]
    fn product_serde_round_trip() {
        let mut product = Product::new("idea-001", "cpub1seller");
        product.add_variant(
            ProductVariant::new("Red", 100).with_attribute("color", "red"),
        );
        product.inventory = Inventory::with_stock(50);

        let json = serde_json::to_string(&product).unwrap();
        let deserialized: Product = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, product.id);
        assert_eq!(deserialized.idea_id, "idea-001");
        assert_eq!(deserialized.variants.len(), 1);
        assert_eq!(deserialized.inventory.stock, Some(50));
    }
}
