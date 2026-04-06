//! Commerce digit helpers — typed constructors and parsers for marketplace content.
//!
//! Commerce metadata is stored in Digit properties as `Value` types.
//! Used by Cart in Scry and storefronts built in Throne.

use serde::{Deserialize, Serialize};

use crate::digit::Digit;
use crate::error::IdeasError;
use crate::helpers::{check_type, prop_int, prop_int_opt, prop_str, prop_str_array, prop_str_opt};
use crate::schema::{DigitSchema, PropertyType};
use x::Value;

const DOMAIN: &str = "commerce";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Metadata for a product digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductMeta {
    pub title: String,
    pub description: String,
    /// Price in Cool cents.
    pub price_cents: i64,
    pub seller_pubkey: String,
    /// Asset references for product images.
    pub images: Vec<String>,
    pub categories: Vec<String>,
    /// Optional inventory count. None = unlimited.
    pub inventory: Option<u32>,
}

/// Metadata for a storefront digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorefrontMeta {
    pub owner_pubkey: String,
    pub name: String,
    pub description: String,
    /// Optional theme .idea reference.
    pub theme_ref: Option<String>,
}

/// Metadata for a cart item digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CartItemMeta {
    /// Reference to the product .idea.
    pub product_ref: String,
    pub quantity: u32,
    pub seller_pubkey: String,
    /// Snapshot of price at time of adding to cart.
    pub price_snapshot: i64,
}

/// Status of an order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    /// Order submitted but not yet paid.
    Placed,
    /// Payment received.
    Paid,
    /// Seller is preparing the order.
    Preparing,
    /// Order has been shipped.
    Shipped,
    /// Order arrived at the buyer.
    Delivered,
    /// Buyer confirmed receipt and satisfaction.
    Confirmed,
    /// A dispute has been opened.
    Disputed,
}

impl OrderStatus {
    fn to_str(&self) -> &'static str {
        match self {
            OrderStatus::Placed => "placed",
            OrderStatus::Paid => "paid",
            OrderStatus::Preparing => "preparing",
            OrderStatus::Shipped => "shipped",
            OrderStatus::Delivered => "delivered",
            OrderStatus::Confirmed => "confirmed",
            OrderStatus::Disputed => "disputed",
        }
    }

    fn from_str_value(s: &str) -> Result<Self, IdeasError> {
        match s {
            "placed" => Ok(OrderStatus::Placed),
            "paid" => Ok(OrderStatus::Paid),
            "preparing" => Ok(OrderStatus::Preparing),
            "shipped" => Ok(OrderStatus::Shipped),
            "delivered" => Ok(OrderStatus::Delivered),
            "confirmed" => Ok(OrderStatus::Confirmed),
            "disputed" => Ok(OrderStatus::Disputed),
            other => Err(IdeasError::CommerceError(format!(
                "unknown order status: {other}"
            ))),
        }
    }
}

/// Metadata for an order digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderMeta {
    pub buyer_pubkey: String,
    pub seller_pubkey: String,
    /// Product references.
    pub items: Vec<String>,
    /// Total in Cool cents.
    pub total_cents: i64,
    pub status: OrderStatus,
    /// Optional payment reference.
    pub payment_ref: Option<String>,
}

/// Metadata for a review digit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewMeta {
    /// Rating 1-5.
    pub rating: u8,
    pub text: Option<String>,
    pub author_pubkey: String,
    /// Reference to the reviewed product.
    pub product_ref: String,
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

/// Create a product digit.
pub fn product_digit(meta: &ProductMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("commerce.product".into(), Value::Null, author.into())?;
    digit = digit.with_property("title".into(), Value::String(meta.title.clone()), author);
    digit = digit.with_property(
        "description".into(),
        Value::String(meta.description.clone()),
        author,
    );
    digit = digit.with_property("price_cents".into(), Value::Int(meta.price_cents), author);
    digit = digit.with_property(
        "seller_pubkey".into(),
        Value::String(meta.seller_pubkey.clone()),
        author,
    );
    let images_value = Value::Array(
        meta.images
            .iter()
            .map(|i| Value::String(i.clone()))
            .collect(),
    );
    digit = digit.with_property("images".into(), images_value, author);
    let categories_value = Value::Array(
        meta.categories
            .iter()
            .map(|c| Value::String(c.clone()))
            .collect(),
    );
    digit = digit.with_property("categories".into(), categories_value, author);
    if let Some(inv) = meta.inventory {
        digit = digit.with_property("inventory".into(), Value::Int(inv as i64), author);
    }
    Ok(digit)
}

/// Create a storefront digit.
pub fn storefront_digit(meta: &StorefrontMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("commerce.storefront".into(), Value::Null, author.into())?;
    digit = digit.with_property(
        "owner_pubkey".into(),
        Value::String(meta.owner_pubkey.clone()),
        author,
    );
    digit = digit.with_property("name".into(), Value::String(meta.name.clone()), author);
    digit = digit.with_property(
        "description".into(),
        Value::String(meta.description.clone()),
        author,
    );
    if let Some(ref theme) = meta.theme_ref {
        digit = digit.with_property("theme_ref".into(), Value::String(theme.clone()), author);
    }
    Ok(digit)
}

/// Create a cart item digit.
pub fn cart_item_digit(meta: &CartItemMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("commerce.cart-item".into(), Value::Null, author.into())?;
    digit = digit.with_property(
        "product_ref".into(),
        Value::String(meta.product_ref.clone()),
        author,
    );
    digit = digit.with_property("quantity".into(), Value::Int(meta.quantity as i64), author);
    digit = digit.with_property(
        "seller_pubkey".into(),
        Value::String(meta.seller_pubkey.clone()),
        author,
    );
    digit = digit.with_property(
        "price_snapshot".into(),
        Value::Int(meta.price_snapshot),
        author,
    );
    Ok(digit)
}

/// Create an order digit.
pub fn order_digit(meta: &OrderMeta, author: &str) -> Result<Digit, IdeasError> {
    let mut digit = Digit::new("commerce.order".into(), Value::Null, author.into())?;
    digit = digit.with_property(
        "buyer_pubkey".into(),
        Value::String(meta.buyer_pubkey.clone()),
        author,
    );
    digit = digit.with_property(
        "seller_pubkey".into(),
        Value::String(meta.seller_pubkey.clone()),
        author,
    );
    let items_value = Value::Array(
        meta.items
            .iter()
            .map(|i| Value::String(i.clone()))
            .collect(),
    );
    digit = digit.with_property("items".into(), items_value, author);
    digit = digit.with_property("total_cents".into(), Value::Int(meta.total_cents), author);
    digit = digit.with_property(
        "status".into(),
        Value::String(meta.status.to_str().into()),
        author,
    );
    if let Some(ref pr) = meta.payment_ref {
        digit = digit.with_property("payment_ref".into(), Value::String(pr.clone()), author);
    }
    Ok(digit)
}

/// Create a review digit.
pub fn review_digit(meta: &ReviewMeta, author: &str) -> Result<Digit, IdeasError> {
    if meta.rating == 0 || meta.rating > 5 {
        return Err(IdeasError::CommerceError(format!(
            "rating must be 1-5, got {}",
            meta.rating
        )));
    }
    let mut digit = Digit::new("commerce.review".into(), Value::Null, author.into())?;
    digit = digit.with_property("rating".into(), Value::Int(meta.rating as i64), author);
    if let Some(ref text) = meta.text {
        digit = digit.with_property("text".into(), Value::String(text.clone()), author);
    }
    digit = digit.with_property(
        "author_pubkey".into(),
        Value::String(meta.author_pubkey.clone()),
        author,
    );
    digit = digit.with_property(
        "product_ref".into(),
        Value::String(meta.product_ref.clone()),
        author,
    );
    Ok(digit)
}

// ---------------------------------------------------------------------------
// Parsers
// ---------------------------------------------------------------------------

/// Parse product metadata from a digit.
pub fn parse_product_meta(digit: &Digit) -> Result<ProductMeta, IdeasError> {
    check_type(digit, "commerce.product", DOMAIN)?;
    Ok(ProductMeta {
        title: prop_str(digit, "title", DOMAIN)?,
        description: prop_str(digit, "description", DOMAIN)?,
        price_cents: prop_int(digit, "price_cents", DOMAIN)?,
        seller_pubkey: prop_str(digit, "seller_pubkey", DOMAIN)?,
        images: prop_str_array(digit, "images", DOMAIN)?,
        categories: prop_str_array(digit, "categories", DOMAIN)?,
        inventory: prop_int_opt(digit, "inventory").map(|v| v as u32),
    })
}

/// Parse storefront metadata from a digit.
pub fn parse_storefront_meta(digit: &Digit) -> Result<StorefrontMeta, IdeasError> {
    check_type(digit, "commerce.storefront", DOMAIN)?;
    Ok(StorefrontMeta {
        owner_pubkey: prop_str(digit, "owner_pubkey", DOMAIN)?,
        name: prop_str(digit, "name", DOMAIN)?,
        description: prop_str(digit, "description", DOMAIN)?,
        theme_ref: prop_str_opt(digit, "theme_ref"),
    })
}

/// Parse cart item metadata from a digit.
pub fn parse_cart_item_meta(digit: &Digit) -> Result<CartItemMeta, IdeasError> {
    check_type(digit, "commerce.cart-item", DOMAIN)?;
    Ok(CartItemMeta {
        product_ref: prop_str(digit, "product_ref", DOMAIN)?,
        quantity: prop_int(digit, "quantity", DOMAIN)? as u32,
        seller_pubkey: prop_str(digit, "seller_pubkey", DOMAIN)?,
        price_snapshot: prop_int(digit, "price_snapshot", DOMAIN)?,
    })
}

/// Parse order metadata from a digit.
pub fn parse_order_meta(digit: &Digit) -> Result<OrderMeta, IdeasError> {
    check_type(digit, "commerce.order", DOMAIN)?;
    let status_str = prop_str(digit, "status", DOMAIN)?;
    Ok(OrderMeta {
        buyer_pubkey: prop_str(digit, "buyer_pubkey", DOMAIN)?,
        seller_pubkey: prop_str(digit, "seller_pubkey", DOMAIN)?,
        items: prop_str_array(digit, "items", DOMAIN)?,
        total_cents: prop_int(digit, "total_cents", DOMAIN)?,
        status: OrderStatus::from_str_value(&status_str)?,
        payment_ref: prop_str_opt(digit, "payment_ref"),
    })
}

/// Parse review metadata from a digit.
pub fn parse_review_meta(digit: &Digit) -> Result<ReviewMeta, IdeasError> {
    check_type(digit, "commerce.review", DOMAIN)?;
    let rating = prop_int(digit, "rating", DOMAIN)? as u8;
    if rating == 0 || rating > 5 {
        return Err(IdeasError::CommerceError(format!(
            "rating must be 1-5, got {rating}"
        )));
    }
    Ok(ReviewMeta {
        rating,
        text: prop_str_opt(digit, "text"),
        author_pubkey: prop_str(digit, "author_pubkey", DOMAIN)?,
        product_ref: prop_str(digit, "product_ref", DOMAIN)?,
    })
}

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

/// Schema for `commerce.product` digits.
pub fn product_schema() -> DigitSchema {
    DigitSchema::new("commerce.product".into())
        .with_required("title", PropertyType::String)
        .with_required("description", PropertyType::String)
        .with_required("price_cents", PropertyType::Int)
        .with_required("seller_pubkey", PropertyType::String)
        .with_required("images", PropertyType::Array)
        .with_required("categories", PropertyType::Array)
        .with_optional("inventory", PropertyType::Int)
        .with_description("Marketplace product listing")
}

/// Schema for `commerce.storefront` digits.
pub fn storefront_schema() -> DigitSchema {
    DigitSchema::new("commerce.storefront".into())
        .with_required("owner_pubkey", PropertyType::String)
        .with_required("name", PropertyType::String)
        .with_required("description", PropertyType::String)
        .with_optional("theme_ref", PropertyType::String)
        .with_description("Marketplace storefront")
}

/// Schema for `commerce.cart-item` digits.
pub fn cart_item_schema() -> DigitSchema {
    DigitSchema::new("commerce.cart-item".into())
        .with_required("product_ref", PropertyType::String)
        .with_required("quantity", PropertyType::Int)
        .with_required("seller_pubkey", PropertyType::String)
        .with_required("price_snapshot", PropertyType::Int)
        .with_description("Shopping cart item with price snapshot")
}

/// Schema for `commerce.order` digits.
pub fn order_schema() -> DigitSchema {
    DigitSchema::new("commerce.order".into())
        .with_required("buyer_pubkey", PropertyType::String)
        .with_required("seller_pubkey", PropertyType::String)
        .with_required("items", PropertyType::Array)
        .with_required("total_cents", PropertyType::Int)
        .with_required("status", PropertyType::String)
        .with_optional("payment_ref", PropertyType::String)
        .with_description("Purchase order")
}

/// Schema for `commerce.review` digits.
pub fn review_schema() -> DigitSchema {
    DigitSchema::new("commerce.review".into())
        .with_required("rating", PropertyType::Int)
        .with_required("author_pubkey", PropertyType::String)
        .with_required("product_ref", PropertyType::String)
        .with_optional("text", PropertyType::String)
        .with_description("Product review with 1-5 rating")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_product_meta() -> ProductMeta {
        ProductMeta {
            title: "Handmade Mug".into(),
            description: "A beautiful ceramic mug".into(),
            price_cents: 2500,
            seller_pubkey: "cpub1seller".into(),
            images: vec!["img001".into(), "img002".into()],
            categories: vec!["home".into(), "kitchen".into()],
            inventory: Some(50),
        }
    }

    #[test]
    fn product_round_trip() {
        let meta = test_product_meta();
        let digit = product_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "commerce.product");

        let parsed = parse_product_meta(&digit).unwrap();
        assert_eq!(parsed.title, "Handmade Mug");
        assert_eq!(parsed.description, "A beautiful ceramic mug");
        assert_eq!(parsed.price_cents, 2500);
        assert_eq!(parsed.seller_pubkey, "cpub1seller");
        assert_eq!(parsed.images, vec!["img001", "img002"]);
        assert_eq!(parsed.categories, vec!["home", "kitchen"]);
        assert_eq!(parsed.inventory, Some(50));
    }

    #[test]
    fn product_no_inventory() {
        let meta = ProductMeta {
            title: "Digital Art".into(),
            description: "An NFT".into(),
            price_cents: 10000,
            seller_pubkey: "cpub1artist".into(),
            images: vec!["art001".into()],
            categories: vec!["art".into()],
            inventory: None,
        };
        let digit = product_digit(&meta, "alice").unwrap();
        let parsed = parse_product_meta(&digit).unwrap();
        assert!(parsed.inventory.is_none());
    }

    #[test]
    fn storefront_round_trip() {
        let meta = StorefrontMeta {
            owner_pubkey: "cpub1owner".into(),
            name: "Artisan Goods".into(),
            description: "Handcrafted items from local artisans".into(),
            theme_ref: Some("theme-ref-123".into()),
        };
        let digit = storefront_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "commerce.storefront");

        let parsed = parse_storefront_meta(&digit).unwrap();
        assert_eq!(parsed.owner_pubkey, "cpub1owner");
        assert_eq!(parsed.name, "Artisan Goods");
        assert_eq!(parsed.theme_ref.as_deref(), Some("theme-ref-123"));
    }

    #[test]
    fn storefront_no_theme() {
        let meta = StorefrontMeta {
            owner_pubkey: "cpub1owner".into(),
            name: "Simple Shop".into(),
            description: "No theme".into(),
            theme_ref: None,
        };
        let digit = storefront_digit(&meta, "alice").unwrap();
        let parsed = parse_storefront_meta(&digit).unwrap();
        assert!(parsed.theme_ref.is_none());
    }

    #[test]
    fn cart_item_round_trip() {
        let meta = CartItemMeta {
            product_ref: "product-abc".into(),
            quantity: 3,
            seller_pubkey: "cpub1seller".into(),
            price_snapshot: 2500,
        };
        let digit = cart_item_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "commerce.cart-item");

        let parsed = parse_cart_item_meta(&digit).unwrap();
        assert_eq!(parsed.product_ref, "product-abc");
        assert_eq!(parsed.quantity, 3);
        assert_eq!(parsed.seller_pubkey, "cpub1seller");
        assert_eq!(parsed.price_snapshot, 2500);
    }

    #[test]
    fn order_round_trip() {
        let meta = OrderMeta {
            buyer_pubkey: "cpub1buyer".into(),
            seller_pubkey: "cpub1seller".into(),
            items: vec!["prod-1".into(), "prod-2".into()],
            total_cents: 5000,
            status: OrderStatus::Placed,
            payment_ref: Some("pay-ref-001".into()),
        };
        let digit = order_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "commerce.order");

        let parsed = parse_order_meta(&digit).unwrap();
        assert_eq!(parsed.buyer_pubkey, "cpub1buyer");
        assert_eq!(parsed.items, vec!["prod-1", "prod-2"]);
        assert_eq!(parsed.total_cents, 5000);
        assert_eq!(parsed.status, OrderStatus::Placed);
        assert_eq!(parsed.payment_ref.as_deref(), Some("pay-ref-001"));
    }

    #[test]
    fn order_no_payment() {
        let meta = OrderMeta {
            buyer_pubkey: "cpub1buyer".into(),
            seller_pubkey: "cpub1seller".into(),
            items: vec!["prod-1".into()],
            total_cents: 1000,
            status: OrderStatus::Placed,
            payment_ref: None,
        };
        let digit = order_digit(&meta, "alice").unwrap();
        let parsed = parse_order_meta(&digit).unwrap();
        assert!(parsed.payment_ref.is_none());
    }

    #[test]
    fn review_round_trip() {
        let meta = ReviewMeta {
            rating: 5,
            text: Some("Excellent product!".into()),
            author_pubkey: "cpub1reviewer".into(),
            product_ref: "product-abc".into(),
        };
        let digit = review_digit(&meta, "alice").unwrap();
        assert_eq!(digit.digit_type(), "commerce.review");

        let parsed = parse_review_meta(&digit).unwrap();
        assert_eq!(parsed.rating, 5);
        assert_eq!(parsed.text.as_deref(), Some("Excellent product!"));
        assert_eq!(parsed.author_pubkey, "cpub1reviewer");
        assert_eq!(parsed.product_ref, "product-abc");
    }

    #[test]
    fn review_no_text() {
        let meta = ReviewMeta {
            rating: 3,
            text: None,
            author_pubkey: "cpub1reviewer".into(),
            product_ref: "product-abc".into(),
        };
        let digit = review_digit(&meta, "alice").unwrap();
        let parsed = parse_review_meta(&digit).unwrap();
        assert!(parsed.text.is_none());
    }

    #[test]
    fn review_invalid_rating_zero() {
        let meta = ReviewMeta {
            rating: 0,
            text: None,
            author_pubkey: "cpub1reviewer".into(),
            product_ref: "product-abc".into(),
        };
        assert!(review_digit(&meta, "alice").is_err());
    }

    #[test]
    fn review_invalid_rating_six() {
        let meta = ReviewMeta {
            rating: 6,
            text: None,
            author_pubkey: "cpub1reviewer".into(),
            product_ref: "product-abc".into(),
        };
        assert!(review_digit(&meta, "alice").is_err());
    }

    #[test]
    fn wrong_type_rejected() {
        let digit = Digit::new("text".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_product_meta(&digit).is_err());
        assert!(parse_storefront_meta(&digit).is_err());
        assert!(parse_cart_item_meta(&digit).is_err());
        assert!(parse_order_meta(&digit).is_err());
        assert!(parse_review_meta(&digit).is_err());
    }

    #[test]
    fn missing_property_rejected() {
        let digit = Digit::new("commerce.product".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_product_meta(&digit).is_err());

        let digit = Digit::new("commerce.order".into(), Value::Null, "alice".into()).unwrap();
        assert!(parse_order_meta(&digit).is_err());
    }

    #[test]
    fn all_order_statuses() {
        for (status, name) in [
            (OrderStatus::Placed, "placed"),
            (OrderStatus::Paid, "paid"),
            (OrderStatus::Preparing, "preparing"),
            (OrderStatus::Shipped, "shipped"),
            (OrderStatus::Delivered, "delivered"),
            (OrderStatus::Confirmed, "confirmed"),
            (OrderStatus::Disputed, "disputed"),
        ] {
            assert_eq!(status.to_str(), name);
            assert_eq!(OrderStatus::from_str_value(name).unwrap(), status);
        }
    }

    #[test]
    fn invalid_order_status() {
        assert!(OrderStatus::from_str_value("unknown").is_err());
    }

    #[test]
    fn schema_validates_product() {
        let schema = product_schema();
        let meta = test_product_meta();
        let digit = product_digit(&meta, "alice").unwrap();
        assert!(crate::schema::validate(&digit, &schema).is_ok());
    }

    #[test]
    fn schema_validates_order() {
        let schema = order_schema();
        let meta = OrderMeta {
            buyer_pubkey: "cpub1buyer".into(),
            seller_pubkey: "cpub1seller".into(),
            items: vec!["prod-1".into()],
            total_cents: 1000,
            status: OrderStatus::Paid,
            payment_ref: None,
        };
        let digit = order_digit(&meta, "alice").unwrap();
        assert!(crate::schema::validate(&digit, &schema).is_ok());
    }

    #[test]
    fn serde_round_trip() {
        let meta = test_product_meta();
        let digit = product_digit(&meta, "alice").unwrap();
        let json = serde_json::to_string(&digit).unwrap();
        let rt: Digit = serde_json::from_str(&json).unwrap();
        let parsed = parse_product_meta(&rt).unwrap();
        assert_eq!(parsed.title, "Handmade Mug");
        assert_eq!(parsed.price_cents, 2500);
    }
}
