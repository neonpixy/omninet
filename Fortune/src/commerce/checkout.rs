use crate::exchange::TradeProposal;
use crate::FortuneError;

use super::cart::Cart;
use super::order::{Order, OrderItem};

/// Stateless checkout engine — orchestrates Cart -> TradeProposal -> Order.
///
/// The actual Ledger transfer happens at the caller level (they call
/// `ledger.transfer()` with the amounts from the proposal). CheckoutEngine
/// only creates the data structures.
///
/// From Consortium Art. 2 §2: "Fair compensation, consent-based agreements,
/// and transparency in value flows shall be required."
pub struct CheckoutEngine;

impl CheckoutEngine {
    /// Create a TradeProposal from a Cart for a specific seller.
    ///
    /// Filters cart items for the given seller and creates a proposal
    /// with the buyer offering Cool for the seller's items.
    pub fn create_proposal(
        cart: &Cart,
        seller: &str,
        buyer_pubkey: &str,
    ) -> Result<TradeProposal, FortuneError> {
        if cart.is_empty() {
            return Err(FortuneError::CartEmpty);
        }

        let seller_items = cart.items_for_seller(seller);
        if seller_items.is_empty() {
            return Err(FortuneError::CheckoutFailed(format!(
                "no items from seller {seller} in cart"
            )));
        }

        let total: i64 = seller_items
            .iter()
            .map(|i| i.price_snapshot_cents * i.quantity as i64)
            .sum();

        let item_refs: Vec<String> = seller_items
            .iter()
            .map(|i| i.product_ref.clone())
            .collect();

        let proposal = TradeProposal::new(buyer_pubkey, seller, total, 0)
            .map_err(|e| FortuneError::CheckoutFailed(e.to_string()))?
            .with_items(Vec::new(), item_refs);

        Ok(proposal)
    }

    /// Execute checkout: create an Order from an accepted TradeProposal.
    ///
    /// The proposal should have been accepted before calling this.
    /// Cart items matching the proposal's recipient (seller) are converted
    /// to OrderItems.
    pub fn execute(proposal: &TradeProposal, cart: &Cart) -> Result<Order, FortuneError> {
        if cart.is_empty() {
            return Err(FortuneError::CartEmpty);
        }

        let seller = &proposal.recipient;
        let seller_items = cart.items_for_seller(seller);

        if seller_items.is_empty() {
            return Err(FortuneError::CheckoutFailed(format!(
                "no items from seller {seller} in cart"
            )));
        }

        let order_items: Vec<OrderItem> = seller_items
            .iter()
            .map(|ci| OrderItem {
                product_ref: ci.product_ref.clone(),
                variant_id: ci.variant_id,
                quantity: ci.quantity,
                unit_price_cents: ci.price_snapshot_cents,
            })
            .collect();

        let order = Order::new(&proposal.proposer, seller, order_items);
        Ok(order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commerce::cart::{CartAction, CartItem};
    use chrono::Utc;

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
    fn create_proposal_from_cart() {
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 2)));
        cart.apply(CartAction::Add(test_item("prod-2", "seller-a", 500, 1)));

        let proposal =
            CheckoutEngine::create_proposal(&cart, "seller-a", "buyer-1").unwrap();

        assert_eq!(proposal.proposer, "buyer-1");
        assert_eq!(proposal.recipient, "seller-a");
        assert_eq!(proposal.offering_cool, 2500); // 1000*2 + 500*1
        assert_eq!(proposal.requesting_cool, 0);
        assert_eq!(proposal.requesting_items.len(), 2);
    }

    #[test]
    fn create_proposal_filters_by_seller() {
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 1)));
        cart.apply(CartAction::Add(test_item("prod-2", "seller-b", 500, 1)));

        let proposal =
            CheckoutEngine::create_proposal(&cart, "seller-a", "buyer-1").unwrap();
        assert_eq!(proposal.offering_cool, 1000);
        assert_eq!(proposal.requesting_items.len(), 1);
    }

    #[test]
    fn create_proposal_empty_cart_fails() {
        let cart = Cart::new();
        let result = CheckoutEngine::create_proposal(&cart, "seller-a", "buyer-1");
        assert!(matches!(result, Err(FortuneError::CartEmpty)));
    }

    #[test]
    fn create_proposal_no_items_for_seller() {
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 1)));

        let result = CheckoutEngine::create_proposal(&cart, "seller-b", "buyer-1");
        assert!(matches!(result, Err(FortuneError::CheckoutFailed(_))));
    }

    #[test]
    fn execute_creates_order() {
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 2)));
        cart.apply(CartAction::Add(test_item("prod-2", "seller-a", 500, 1)));

        let proposal =
            CheckoutEngine::create_proposal(&cart, "seller-a", "buyer-1").unwrap();
        let order = CheckoutEngine::execute(&proposal, &cart).unwrap();

        assert_eq!(order.buyer_pubkey, "buyer-1");
        assert_eq!(order.seller_pubkey, "seller-a");
        assert_eq!(order.total_cents, 2500);
        assert_eq!(order.items.len(), 2);
        assert_eq!(
            order.status,
            ideas::commerce::OrderStatus::Placed
        );
    }

    #[test]
    fn execute_empty_cart_fails() {
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("prod-1", "seller-a", 1000, 1)));

        let proposal =
            CheckoutEngine::create_proposal(&cart, "seller-a", "buyer-1").unwrap();

        let empty_cart = Cart::new();
        let result = CheckoutEngine::execute(&proposal, &empty_cart);
        assert!(matches!(result, Err(FortuneError::CartEmpty)));
    }

    #[test]
    fn full_checkout_flow() {
        // 1. Build a cart
        let mut cart = Cart::new();
        cart.apply(CartAction::Add(test_item("mug-001", "potter", 2500, 1)));
        cart.apply(CartAction::Add(test_item("bowl-002", "potter", 3000, 2)));

        // 2. Create proposal
        let proposal =
            CheckoutEngine::create_proposal(&cart, "potter", "buyer-alice").unwrap();
        assert_eq!(proposal.offering_cool, 8500); // 2500 + 3000*2

        // 3. Execute checkout
        let order = CheckoutEngine::execute(&proposal, &cart).unwrap();
        assert_eq!(order.total_cents, 8500);
        assert_eq!(order.items.len(), 2);
        assert_eq!(order.buyer_pubkey, "buyer-alice");
        assert_eq!(order.seller_pubkey, "potter");
    }
}
