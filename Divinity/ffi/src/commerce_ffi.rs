use std::ffi::c_char;
use std::sync::Mutex;

use fortune::commerce::{
    cart::{Cart, CartAction},
    checkout::CheckoutEngine,
    order::{Order, OrderItem, Receipt},
    product::{Inventory, Product},
    storefront::{Storefront, StorefrontPolicies},
};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// Product — opaque pointer (inventory + variant management)
// ===================================================================

pub struct CommerceProduct(pub(crate) Mutex<Product>);

/// Create a new product linked to an .idea digit.
///
/// Free with `divi_commerce_product_free`.
///
/// # Safety
/// `idea_id` and `seller_pubkey` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_product_new(
    idea_id: *const c_char,
    seller_pubkey: *const c_char,
) -> *mut CommerceProduct {
    let Some(idea) = c_str_to_str(idea_id) else {
        set_last_error("divi_commerce_product_new: invalid idea_id");
        return std::ptr::null_mut();
    };
    let Some(seller) = c_str_to_str(seller_pubkey) else {
        set_last_error("divi_commerce_product_new: invalid seller_pubkey");
        return std::ptr::null_mut();
    };

    Box::into_raw(Box::new(CommerceProduct(Mutex::new(Product::new(
        idea, seller,
    )))))
}

/// Free a product.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_product_free(ptr: *mut CommerceProduct) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Serialize a product to JSON.
///
/// Returns JSON (Product). Caller must free via `divi_free_string`.
///
/// # Safety
/// `product` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_product_to_json(
    product: *const CommerceProduct,
) -> *mut c_char {
    if product.is_null() {
        set_last_error("divi_commerce_product_to_json: null pointer");
        return std::ptr::null_mut();
    }
    let product = unsafe { &*product };
    let guard = lock_or_recover(&product.0);
    json_to_c(&*guard)
}

/// Deserialize a product from JSON.
///
/// Returns a new opaque pointer. Free with `divi_commerce_product_free`.
///
/// # Safety
/// `json` must be a valid C string containing Product JSON.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_product_from_json(
    json: *const c_char,
) -> *mut CommerceProduct {
    let Some(j) = c_str_to_str(json) else {
        set_last_error("divi_commerce_product_from_json: invalid json");
        return std::ptr::null_mut();
    };

    match serde_json::from_str::<Product>(j) {
        Ok(p) => Box::into_raw(Box::new(CommerceProduct(Mutex::new(p)))),
        Err(e) => {
            set_last_error(format!("divi_commerce_product_from_json: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Add a variant to the product.
///
/// `variant_json` is a JSON ProductVariant.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `product` must be a valid pointer. `variant_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_product_add_variant(
    product: *const CommerceProduct,
    variant_json: *const c_char,
) -> i32 {
    clear_last_error();

    if product.is_null() {
        set_last_error("divi_commerce_product_add_variant: null pointer");
        return -1;
    }
    let product = unsafe { &*product };

    let Some(vj) = c_str_to_str(variant_json) else {
        set_last_error("divi_commerce_product_add_variant: invalid variant_json");
        return -1;
    };

    let variant = match serde_json::from_str(vj) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_commerce_product_add_variant: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&product.0);
    guard.add_variant(variant);
    0
}

/// Remove a variant by UUID string.
///
/// Returns true if found and removed, false otherwise.
///
/// # Safety
/// `product` must be a valid pointer. `variant_id` must be a valid C string UUID.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_product_remove_variant(
    product: *const CommerceProduct,
    variant_id: *const c_char,
) -> bool {
    if product.is_null() {
        return false;
    }
    let product = unsafe { &*product };

    let Some(vid) = c_str_to_str(variant_id) else {
        return false;
    };

    let id = match uuid::Uuid::parse_str(vid) {
        Ok(id) => id,
        Err(_) => return false,
    };

    let mut guard = lock_or_recover(&product.0);
    guard.remove_variant(&id)
}

/// Reserve stock for a pending order.
///
/// Returns 0 on success, -1 on error (insufficient inventory).
///
/// # Safety
/// `product` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_product_reserve(
    product: *const CommerceProduct,
    quantity: u32,
) -> i32 {
    clear_last_error();

    if product.is_null() {
        set_last_error("divi_commerce_product_reserve: null pointer");
        return -1;
    }
    let product = unsafe { &*product };

    let mut guard = lock_or_recover(&product.0);
    match guard.reserve(quantity) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Release a stock reservation (e.g., cancelled order).
///
/// # Safety
/// `product` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_product_release_reservation(
    product: *const CommerceProduct,
    quantity: u32,
) {
    if product.is_null() {
        return;
    }
    let product = unsafe { &*product };
    let mut guard = lock_or_recover(&product.0);
    guard.release_reservation(quantity);
}

/// Set the product inventory from JSON.
///
/// `inventory_json` is a JSON Inventory.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `product` must be a valid pointer. `inventory_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_product_set_inventory(
    product: *const CommerceProduct,
    inventory_json: *const c_char,
) -> i32 {
    clear_last_error();

    if product.is_null() {
        set_last_error("divi_commerce_product_set_inventory: null pointer");
        return -1;
    }
    let product = unsafe { &*product };

    let Some(ij) = c_str_to_str(inventory_json) else {
        set_last_error("divi_commerce_product_set_inventory: invalid inventory_json");
        return -1;
    };

    let inventory: Inventory = match serde_json::from_str(ij) {
        Ok(i) => i,
        Err(e) => {
            set_last_error(format!("divi_commerce_product_set_inventory: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&product.0);
    guard.inventory = inventory;
    guard.updated_at = chrono::Utc::now();
    0
}

/// Activate the product listing.
///
/// # Safety
/// `product` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_product_activate(product: *const CommerceProduct) {
    if product.is_null() {
        return;
    }
    let product = unsafe { &*product };
    let mut guard = lock_or_recover(&product.0);
    guard.activate();
}

/// Deactivate the product listing.
///
/// # Safety
/// `product` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_product_deactivate(product: *const CommerceProduct) {
    if product.is_null() {
        return;
    }
    let product = unsafe { &*product };
    let mut guard = lock_or_recover(&product.0);
    guard.deactivate();
}

/// Whether the product is currently in stock.
///
/// # Safety
/// `product` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_product_is_in_stock(
    product: *const CommerceProduct,
) -> bool {
    if product.is_null() {
        return false;
    }
    let product = unsafe { &*product };
    let guard = lock_or_recover(&product.0);
    guard.is_in_stock()
}

/// Available stock. Returns -1 for unlimited (None), actual count otherwise.
///
/// # Safety
/// `product` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_product_available_stock(
    product: *const CommerceProduct,
) -> i64 {
    if product.is_null() {
        return -1;
    }
    let product = unsafe { &*product };
    let guard = lock_or_recover(&product.0);
    match guard.available_stock() {
        None => -1,
        Some(count) => count as i64,
    }
}

// ===================================================================
// Cart — opaque pointer (local shopping cart)
// ===================================================================

pub struct CommerceCart(pub(crate) Mutex<Cart>);

/// Create an empty cart.
///
/// Free with `divi_commerce_cart_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_commerce_cart_new() -> *mut CommerceCart {
    Box::into_raw(Box::new(CommerceCart(Mutex::new(Cart::new()))))
}

/// Free a cart.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_cart_free(ptr: *mut CommerceCart) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Serialize the cart to JSON.
///
/// Returns JSON (Cart). Caller must free via `divi_free_string`.
///
/// # Safety
/// `cart` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_cart_to_json(cart: *const CommerceCart) -> *mut c_char {
    if cart.is_null() {
        set_last_error("divi_commerce_cart_to_json: null pointer");
        return std::ptr::null_mut();
    }
    let cart = unsafe { &*cart };
    let guard = lock_or_recover(&cart.0);
    json_to_c(&*guard)
}

/// Deserialize a cart from JSON.
///
/// Returns a new opaque pointer. Free with `divi_commerce_cart_free`.
///
/// # Safety
/// `json` must be a valid C string containing Cart JSON.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_cart_from_json(json: *const c_char) -> *mut CommerceCart {
    let Some(j) = c_str_to_str(json) else {
        set_last_error("divi_commerce_cart_from_json: invalid json");
        return std::ptr::null_mut();
    };

    match serde_json::from_str::<Cart>(j) {
        Ok(c) => Box::into_raw(Box::new(CommerceCart(Mutex::new(c)))),
        Err(e) => {
            set_last_error(format!("divi_commerce_cart_from_json: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Apply an action to the cart.
///
/// `action_json` is a JSON CartAction (Add, Remove, UpdateQuantity, Clear).
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `cart` must be a valid pointer. `action_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_cart_apply(
    cart: *const CommerceCart,
    action_json: *const c_char,
) -> i32 {
    clear_last_error();

    if cart.is_null() {
        set_last_error("divi_commerce_cart_apply: null pointer");
        return -1;
    }
    let cart = unsafe { &*cart };

    let Some(aj) = c_str_to_str(action_json) else {
        set_last_error("divi_commerce_cart_apply: invalid action_json");
        return -1;
    };

    let action: CartAction = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_commerce_cart_apply: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&cart.0);
    guard.apply(action);
    0
}

/// Total price in Cool cents.
///
/// # Safety
/// `cart` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_cart_total_cents(cart: *const CommerceCart) -> i64 {
    if cart.is_null() {
        return 0;
    }
    let cart = unsafe { &*cart };
    let guard = lock_or_recover(&cart.0);
    guard.total_cents()
}

/// Total item count (sum of quantities).
///
/// # Safety
/// `cart` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_cart_item_count(cart: *const CommerceCart) -> usize {
    if cart.is_null() {
        return 0;
    }
    let cart = unsafe { &*cart };
    let guard = lock_or_recover(&cart.0);
    guard.item_count()
}

/// Whether the cart is empty.
///
/// # Safety
/// `cart` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_cart_is_empty(cart: *const CommerceCart) -> bool {
    if cart.is_null() {
        return true;
    }
    let cart = unsafe { &*cart };
    let guard = lock_or_recover(&cart.0);
    guard.is_empty()
}

/// Find a cart item by product reference.
///
/// Returns JSON (CartItem) or null if not found. Caller must free via `divi_free_string`.
///
/// # Safety
/// `cart` must be a valid pointer. `product_ref` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_cart_find_item(
    cart: *const CommerceCart,
    product_ref: *const c_char,
) -> *mut c_char {
    if cart.is_null() {
        return std::ptr::null_mut();
    }
    let cart = unsafe { &*cart };

    let Some(pr) = c_str_to_str(product_ref) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&cart.0);
    match guard.find_item(pr) {
        Some(item) => json_to_c(item),
        None => std::ptr::null_mut(),
    }
}

/// Unique seller pubkeys in the cart.
///
/// Returns JSON array of strings. Caller must free via `divi_free_string`.
///
/// # Safety
/// `cart` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_cart_sellers(cart: *const CommerceCart) -> *mut c_char {
    if cart.is_null() {
        set_last_error("divi_commerce_cart_sellers: null pointer");
        return std::ptr::null_mut();
    }
    let cart = unsafe { &*cart };
    let guard = lock_or_recover(&cart.0);
    let sellers = guard.sellers();
    json_to_c(&sellers)
}

/// Items from a specific seller.
///
/// Returns JSON array of CartItem. Caller must free via `divi_free_string`.
///
/// # Safety
/// `cart` must be a valid pointer. `seller` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_cart_items_for_seller(
    cart: *const CommerceCart,
    seller: *const c_char,
) -> *mut c_char {
    if cart.is_null() {
        set_last_error("divi_commerce_cart_items_for_seller: null pointer");
        return std::ptr::null_mut();
    }
    let cart = unsafe { &*cart };

    let Some(s) = c_str_to_str(seller) else {
        set_last_error("divi_commerce_cart_items_for_seller: invalid seller");
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&cart.0);
    let items = guard.items_for_seller(s);
    json_to_c(&items)
}

// ===================================================================
// Order — opaque pointer (purchase lifecycle)
// ===================================================================

pub struct CommerceOrder(pub(crate) Mutex<Order>);

/// Create a new order.
///
/// `items_json` is a JSON array of OrderItem.
/// Free with `divi_commerce_order_free`.
///
/// # Safety
/// `buyer`, `seller`, and `items_json` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_order_new(
    buyer: *const c_char,
    seller: *const c_char,
    items_json: *const c_char,
) -> *mut CommerceOrder {
    let Some(b) = c_str_to_str(buyer) else {
        set_last_error("divi_commerce_order_new: invalid buyer");
        return std::ptr::null_mut();
    };
    let Some(s) = c_str_to_str(seller) else {
        set_last_error("divi_commerce_order_new: invalid seller");
        return std::ptr::null_mut();
    };
    let Some(ij) = c_str_to_str(items_json) else {
        set_last_error("divi_commerce_order_new: invalid items_json");
        return std::ptr::null_mut();
    };

    let items: Vec<OrderItem> = match serde_json::from_str(ij) {
        Ok(i) => i,
        Err(e) => {
            set_last_error(format!("divi_commerce_order_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(CommerceOrder(Mutex::new(Order::new(b, s, items)))))
}

/// Free an order.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_order_free(ptr: *mut CommerceOrder) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Serialize an order to JSON.
///
/// Returns JSON (Order). Caller must free via `divi_free_string`.
///
/// # Safety
/// `order` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_order_to_json(order: *const CommerceOrder) -> *mut c_char {
    if order.is_null() {
        set_last_error("divi_commerce_order_to_json: null pointer");
        return std::ptr::null_mut();
    }
    let order = unsafe { &*order };
    let guard = lock_or_recover(&order.0);
    json_to_c(&*guard)
}

/// Deserialize an order from JSON.
///
/// Returns a new opaque pointer. Free with `divi_commerce_order_free`.
///
/// # Safety
/// `json` must be a valid C string containing Order JSON.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_order_from_json(json: *const c_char) -> *mut CommerceOrder {
    let Some(j) = c_str_to_str(json) else {
        set_last_error("divi_commerce_order_from_json: invalid json");
        return std::ptr::null_mut();
    };

    match serde_json::from_str::<Order>(j) {
        Ok(o) => Box::into_raw(Box::new(CommerceOrder(Mutex::new(o)))),
        Err(e) => {
            set_last_error(format!("divi_commerce_order_from_json: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Advance the order status through its lifecycle.
///
/// Placed -> Paid -> Preparing -> Shipped -> Delivered -> Confirmed.
/// Returns 0 on success, -1 on error (invalid transition).
///
/// # Safety
/// `order` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_order_advance_status(
    order: *const CommerceOrder,
) -> i32 {
    clear_last_error();

    if order.is_null() {
        set_last_error("divi_commerce_order_advance_status: null pointer");
        return -1;
    }
    let order = unsafe { &*order };

    let mut guard = lock_or_recover(&order.0);
    match guard.advance_status() {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Dispute the order (only valid from Delivered status).
///
/// Returns 0 on success, -1 on error (invalid transition).
///
/// # Safety
/// `order` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_order_dispute(order: *const CommerceOrder) -> i32 {
    clear_last_error();

    if order.is_null() {
        set_last_error("divi_commerce_order_dispute: null pointer");
        return -1;
    }
    let order = unsafe { &*order };

    let mut guard = lock_or_recover(&order.0);
    match guard.dispute() {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Set the payment reference (Ledger transaction UUID).
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `order` must be a valid pointer. `uuid_str` must be a valid C string UUID.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_order_set_payment_ref(
    order: *const CommerceOrder,
    uuid_str: *const c_char,
) -> i32 {
    clear_last_error();

    if order.is_null() {
        set_last_error("divi_commerce_order_set_payment_ref: null pointer");
        return -1;
    }
    let order = unsafe { &*order };

    let Some(u) = c_str_to_str(uuid_str) else {
        set_last_error("divi_commerce_order_set_payment_ref: invalid uuid_str");
        return -1;
    };

    let id = match uuid::Uuid::parse_str(u) {
        Ok(id) => id,
        Err(e) => {
            set_last_error(format!("divi_commerce_order_set_payment_ref: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&order.0);
    guard.set_payment_ref(id);
    0
}

/// Set the delivery reference (Caravan delivery UUID).
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `order` must be a valid pointer. `uuid_str` must be a valid C string UUID.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_order_set_delivery_ref(
    order: *const CommerceOrder,
    uuid_str: *const c_char,
) -> i32 {
    clear_last_error();

    if order.is_null() {
        set_last_error("divi_commerce_order_set_delivery_ref: null pointer");
        return -1;
    }
    let order = unsafe { &*order };

    let Some(u) = c_str_to_str(uuid_str) else {
        set_last_error("divi_commerce_order_set_delivery_ref: invalid uuid_str");
        return -1;
    };

    let id = match uuid::Uuid::parse_str(u) {
        Ok(id) => id,
        Err(e) => {
            set_last_error(format!("divi_commerce_order_set_delivery_ref: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&order.0);
    guard.set_delivery_ref(id);
    0
}

/// Whether the order is in a terminal state (Confirmed or Disputed).
///
/// # Safety
/// `order` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_order_is_complete(
    order: *const CommerceOrder,
) -> bool {
    if order.is_null() {
        return false;
    }
    let order = unsafe { &*order };
    let guard = lock_or_recover(&order.0);
    guard.is_complete()
}

/// Get the current order status as JSON.
///
/// Returns JSON (OrderStatus). Caller must free via `divi_free_string`.
///
/// # Safety
/// `order` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_order_status(
    order: *const CommerceOrder,
) -> *mut c_char {
    if order.is_null() {
        set_last_error("divi_commerce_order_status: null pointer");
        return std::ptr::null_mut();
    }
    let order = unsafe { &*order };
    let guard = lock_or_recover(&order.0);
    json_to_c(&guard.status)
}

// ===================================================================
// Storefront — JSON round-trip (stateless data)
// ===================================================================

/// Create a new storefront.
///
/// Returns JSON (Storefront). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_storefront_new(
    owner: *const c_char,
    name: *const c_char,
    description: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(o) = c_str_to_str(owner) else {
        set_last_error("divi_commerce_storefront_new: invalid owner");
        return std::ptr::null_mut();
    };
    let Some(n) = c_str_to_str(name) else {
        set_last_error("divi_commerce_storefront_new: invalid name");
        return std::ptr::null_mut();
    };
    let Some(d) = c_str_to_str(description) else {
        set_last_error("divi_commerce_storefront_new: invalid description");
        return std::ptr::null_mut();
    };

    let store = Storefront::new(o, n, d);
    json_to_c(&store)
}

/// Get default storefront policies as JSON.
///
/// Returns JSON (StorefrontPolicies). Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_commerce_storefront_default_policies() -> *mut c_char {
    json_to_c(&StorefrontPolicies::default())
}

// ===================================================================
// Receipt — JSON (from opaque Order pointer)
// ===================================================================

/// Create a receipt from an order.
///
/// Clones the order data and wraps it in a Receipt. Returns JSON (Receipt).
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `order` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_receipt_from_order(
    order: *const CommerceOrder,
) -> *mut c_char {
    if order.is_null() {
        set_last_error("divi_commerce_receipt_from_order: null pointer");
        return std::ptr::null_mut();
    }
    let order = unsafe { &*order };
    let guard = lock_or_recover(&order.0);
    let receipt = Receipt::from_order(guard.clone());
    json_to_c(&receipt)
}

// ===================================================================
// Checkout — stateless functions (Cart + args -> JSON/Order)
// ===================================================================

/// Create a checkout proposal from a cart for a specific seller.
///
/// Returns JSON (TradeProposal). Caller must free via `divi_free_string`.
/// Returns null on error.
///
/// # Safety
/// `cart` must be a valid pointer. `seller` and `buyer` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_checkout_create_proposal(
    cart: *const CommerceCart,
    seller: *const c_char,
    buyer: *const c_char,
) -> *mut c_char {
    clear_last_error();

    if cart.is_null() {
        set_last_error("divi_commerce_checkout_create_proposal: null cart");
        return std::ptr::null_mut();
    }
    let cart = unsafe { &*cart };

    let Some(s) = c_str_to_str(seller) else {
        set_last_error("divi_commerce_checkout_create_proposal: invalid seller");
        return std::ptr::null_mut();
    };
    let Some(b) = c_str_to_str(buyer) else {
        set_last_error("divi_commerce_checkout_create_proposal: invalid buyer");
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&cart.0);
    match CheckoutEngine::create_proposal(&guard, s, b) {
        Ok(proposal) => json_to_c(&proposal),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Execute a checkout: convert a TradeProposal + Cart into an Order.
///
/// `proposal_json` is a JSON TradeProposal. `cart` is the opaque cart pointer.
/// Returns a new `CommerceOrder` opaque pointer. Free with `divi_commerce_order_free`.
/// Returns null on error.
///
/// # Safety
/// `proposal_json` must be a valid C string. `cart` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_commerce_checkout_execute(
    proposal_json: *const c_char,
    cart: *const CommerceCart,
) -> *mut CommerceOrder {
    clear_last_error();

    let Some(pj) = c_str_to_str(proposal_json) else {
        set_last_error("divi_commerce_checkout_execute: invalid proposal_json");
        return std::ptr::null_mut();
    };

    if cart.is_null() {
        set_last_error("divi_commerce_checkout_execute: null cart");
        return std::ptr::null_mut();
    }
    let cart = unsafe { &*cart };

    let proposal = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_commerce_checkout_execute: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&cart.0);
    match CheckoutEngine::execute(&proposal, &guard) {
        Ok(order) => Box::into_raw(Box::new(CommerceOrder(Mutex::new(order)))),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}
