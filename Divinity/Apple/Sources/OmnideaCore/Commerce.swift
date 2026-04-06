import COmnideaFFI
import Foundation

// MARK: - CommerceProduct

/// A product listing backed by an .idea digit.
///
/// Manages variants, inventory, and stock reservations. The product is
/// linked to a specific .idea file and seller identity.
///
/// ```swift
/// let product = CommerceProduct(ideaId: "abc-123", sellerPubkey: "cpub1...")
/// try product.setInventory(inventoryJSON: "{\"stock\":100}")
/// product.activate()
/// ```
public final class CommerceProduct: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a new product linked to an .idea digit.
    public init?(ideaId: String, sellerPubkey: String) {
        guard let p = divi_commerce_product_new(ideaId, sellerPubkey) else {
            return nil
        }
        ptr = p
    }

    /// Internal init from a raw pointer (used by deserialization).
    private init(ptr: OpaquePointer) {
        self.ptr = ptr
    }

    deinit {
        divi_commerce_product_free(ptr)
    }

    /// Serialize the product to JSON.
    public func toJSON() -> String {
        let json = divi_commerce_product_to_json(ptr)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Deserialize a product from JSON. Returns nil on invalid input.
    public static func fromJSON(_ json: String) -> CommerceProduct? {
        guard let p = divi_commerce_product_from_json(json) else {
            return nil
        }
        return CommerceProduct(ptr: p)
    }

    /// Add a variant to the product.
    ///
    /// - Parameter variantJSON: JSON representation of a ProductVariant.
    public func addVariant(variantJSON: String) throws {
        let result = divi_commerce_product_add_variant(ptr, variantJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add variant")
        }
    }

    /// Remove a variant by UUID string.
    ///
    /// - Returns: `true` if found and removed, `false` otherwise.
    @discardableResult
    public func removeVariant(variantId: String) -> Bool {
        divi_commerce_product_remove_variant(ptr, variantId)
    }

    /// Reserve stock for a pending order.
    ///
    /// Fails if insufficient inventory is available.
    public func reserve(quantity: UInt32) throws {
        let result = divi_commerce_product_reserve(ptr, quantity)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to reserve \(quantity) units")
        }
    }

    /// Release a stock reservation (e.g., cancelled order).
    public func releaseReservation(quantity: UInt32) {
        divi_commerce_product_release_reservation(ptr, quantity)
    }

    /// Set the product inventory from JSON.
    ///
    /// - Parameter inventoryJSON: JSON representation of an Inventory.
    public func setInventory(inventoryJSON: String) throws {
        let result = divi_commerce_product_set_inventory(ptr, inventoryJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set inventory")
        }
    }

    /// Activate the product listing (makes it visible/purchasable).
    public func activate() {
        divi_commerce_product_activate(ptr)
    }

    /// Deactivate the product listing (hides it from buyers).
    public func deactivate() {
        divi_commerce_product_deactivate(ptr)
    }

    /// Whether the product is currently in stock.
    public var isInStock: Bool {
        divi_commerce_product_is_in_stock(ptr)
    }

    /// Available stock count. Returns -1 for unlimited inventory.
    public var availableStock: Int64 {
        divi_commerce_product_available_stock(ptr)
    }
}

// MARK: - CommerceCart

/// A local shopping cart. Consent-gated — sellers never see it until checkout.
///
/// ```swift
/// let cart = CommerceCart()
/// try cart.apply(actionJSON: "{\"Add\":{...}}")
/// print(cart.totalCents) // total in Cool cents
/// ```
public final class CommerceCart: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create an empty cart.
    public init() {
        ptr = divi_commerce_cart_new()!
    }

    /// Internal init from a raw pointer (used by deserialization).
    private init(ptr: OpaquePointer) {
        self.ptr = ptr
    }

    deinit {
        divi_commerce_cart_free(ptr)
    }

    /// Serialize the cart to JSON.
    public func toJSON() -> String {
        let json = divi_commerce_cart_to_json(ptr)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Deserialize a cart from JSON. Returns nil on invalid input.
    public static func fromJSON(_ json: String) -> CommerceCart? {
        guard let p = divi_commerce_cart_from_json(json) else {
            return nil
        }
        return CommerceCart(ptr: p)
    }

    /// Apply an action to the cart (Add, Remove, UpdateQuantity, Clear).
    ///
    /// - Parameter actionJSON: JSON representation of a CartAction.
    public func apply(actionJSON: String) throws {
        let result = divi_commerce_cart_apply(ptr, actionJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to apply cart action")
        }
    }

    /// Total price in Cool cents.
    public var totalCents: Int64 {
        divi_commerce_cart_total_cents(ptr)
    }

    /// Total item count (sum of quantities across all items).
    public var itemCount: Int {
        Int(divi_commerce_cart_item_count(ptr))
    }

    /// Whether the cart is empty.
    public var isEmpty: Bool {
        divi_commerce_cart_is_empty(ptr)
    }

    /// Find a cart item by product reference.
    ///
    /// - Returns: JSON CartItem, or nil if not found.
    public func findItem(productRef: String) -> String? {
        guard let json = divi_commerce_cart_find_item(ptr, productRef) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Unique seller pubkeys in the cart.
    ///
    /// - Returns: JSON array of seller pubkey strings.
    public func sellers() -> String {
        let json = divi_commerce_cart_sellers(ptr)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Items from a specific seller.
    ///
    /// - Returns: JSON array of CartItem.
    public func itemsForSeller(_ seller: String) -> String {
        let json = divi_commerce_cart_items_for_seller(ptr, seller)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - CommerceOrder

/// A purchase order tracking its lifecycle from placement to completion.
///
/// Status flows: Placed -> Paid -> Preparing -> Shipped -> Delivered -> Confirmed.
/// Can also be disputed from the Delivered state.
///
/// ```swift
/// let order = CommerceOrder(buyer: "cpub1...", seller: "cpub1...", itemsJSON: "[...]")
/// try order.advanceStatus() // Placed -> Paid
/// ```
public final class CommerceOrder: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a new order.
    ///
    /// - Parameters:
    ///   - buyer: Buyer's public key.
    ///   - seller: Seller's public key.
    ///   - itemsJSON: JSON array of OrderItem.
    public init?(buyer: String, seller: String, itemsJSON: String) {
        guard let p = divi_commerce_order_new(buyer, seller, itemsJSON) else {
            return nil
        }
        ptr = p
    }

    /// Internal init from a raw pointer (used by checkout.execute).
    fileprivate init(ptr: OpaquePointer) {
        self.ptr = ptr
    }

    deinit {
        divi_commerce_order_free(ptr)
    }

    /// Serialize the order to JSON.
    public func toJSON() -> String {
        let json = divi_commerce_order_to_json(ptr)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Deserialize an order from JSON. Returns nil on invalid input.
    public static func fromJSON(_ json: String) -> CommerceOrder? {
        guard let p = divi_commerce_order_from_json(json) else {
            return nil
        }
        return CommerceOrder(ptr: p)
    }

    /// Advance the order status through its lifecycle.
    ///
    /// Placed -> Paid -> Preparing -> Shipped -> Delivered -> Confirmed.
    public func advanceStatus() throws {
        let result = divi_commerce_order_advance_status(ptr)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to advance order status")
        }
    }

    /// Dispute the order (only valid from Delivered status).
    public func dispute() throws {
        let result = divi_commerce_order_dispute(ptr)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to dispute order")
        }
    }

    /// Set the payment reference (Ledger transaction UUID).
    public func setPaymentRef(uuid: String) throws {
        let result = divi_commerce_order_set_payment_ref(ptr, uuid)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set payment ref")
        }
    }

    /// Set the delivery reference (Caravan delivery UUID).
    public func setDeliveryRef(uuid: String) throws {
        let result = divi_commerce_order_set_delivery_ref(ptr, uuid)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set delivery ref")
        }
    }

    /// Whether the order is in a terminal state (Confirmed or Disputed).
    public var isComplete: Bool {
        divi_commerce_order_is_complete(ptr)
    }

    /// Get the current order status as JSON (OrderStatus).
    public func status() -> String {
        let json = divi_commerce_order_status(ptr)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - CommerceStorefront

/// Storefront creation and policies. Stateless JSON round-trip.
public enum CommerceStorefront {

    /// Create a new storefront.
    ///
    /// - Returns: JSON representation of the Storefront.
    public static func new(owner: String, name: String, description: String) -> String {
        let json = divi_commerce_storefront_new(owner, name, description)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get the default storefront policies as JSON (StorefrontPolicies).
    public static func defaultPolicies() -> String {
        let json = divi_commerce_storefront_default_policies()!
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - CommerceCheckout

/// Checkout engine — converts a cart into an order via a trade proposal.
public enum CommerceCheckout {

    /// Create a checkout proposal from a cart for a specific seller.
    ///
    /// - Parameters:
    ///   - cart: The shopping cart.
    ///   - seller: Seller's public key.
    ///   - buyer: Buyer's public key.
    /// - Returns: JSON TradeProposal.
    public static func createProposal(cart: CommerceCart, seller: String, buyer: String) throws -> String {
        guard let json = divi_commerce_checkout_create_proposal(cart.ptr, seller, buyer) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create checkout proposal")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Execute a checkout: convert a TradeProposal + Cart into an Order.
    ///
    /// - Parameters:
    ///   - proposalJSON: JSON TradeProposal (from `createProposal`).
    ///   - cart: The shopping cart.
    /// - Returns: A new `CommerceOrder` representing the placed order.
    public static func execute(proposalJSON: String, cart: CommerceCart) throws -> CommerceOrder {
        guard let orderPtr = divi_commerce_checkout_execute(proposalJSON, cart.ptr) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to execute checkout")
        }
        return CommerceOrder(ptr: orderPtr)
    }
}

// MARK: - CommerceReceipt

/// Receipt generation from completed orders.
public enum CommerceReceipt {

    /// Create a receipt from an order.
    ///
    /// Clones the order data and wraps it in a Receipt.
    /// - Returns: JSON Receipt.
    public static func fromOrder(order: CommerceOrder) throws -> String {
        guard let json = divi_commerce_receipt_from_order(order.ptr) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create receipt from order")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}
