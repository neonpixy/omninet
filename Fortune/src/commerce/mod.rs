//! # Commerce — Marketplace Business Logic
//!
//! Product listings, storefronts, shopping carts, orders, and checkout.
//! Business logic wrappers that complement the Ideas-layer digit types.
//!
//! Content fields (title, description, images) live in .idea digits.
//! Fortune Commerce handles inventory, pricing, order lifecycle, and checkout flow.
//!
//! From Consortium Art. 2 §2: "Fair compensation, consent-based agreements,
//! and transparency in value flows shall be required."

pub mod cart;
pub mod checkout;
pub mod order;
pub mod product;
pub mod storefront;

pub use cart::{Cart, CartAction, CartItem, CartSuggestion};
pub use checkout::CheckoutEngine;
pub use order::{Order, OrderItem, Receipt};
pub use product::{Inventory, Product, ProductVariant};
pub use storefront::{Storefront, StorefrontPolicies};
