use std::ffi::c_char;
use std::sync::Mutex;

use fortune::{
    DemurrageEngine, FortunePolicy, Ledger, MintReason, NetworkMetrics,
    TradeProposal, Transaction, TransactionReason, Treasury, UbiDistributor,
};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover, string_to_c};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// Ledger — opaque pointer (tracks all balances + transaction history)
// ===================================================================

pub struct FortuneLedger(pub(crate) Mutex<Ledger>);

/// Create a new empty ledger.
/// Free with `divi_fortune_ledger_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_fortune_ledger_new() -> *mut FortuneLedger {
    Box::into_raw(Box::new(FortuneLedger(Mutex::new(Ledger::new()))))
}

/// Free a ledger.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_ledger_free(ptr: *mut FortuneLedger) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr)); }
    }
}

/// Get balance for a pubkey.
///
/// Returns JSON (Balance). Caller must free via `divi_free_string`.
///
/// # Safety
/// `ledger` must be a valid pointer. `pubkey` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_ledger_balance(
    ledger: *const FortuneLedger,
    pubkey: *const c_char,
) -> *mut c_char {
    let ledger = unsafe { &*ledger };
    let Some(pk) = c_str_to_str(pubkey) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&ledger.0);
    json_to_c(&guard.balance(pk))
}

/// Credit a pubkey with Cool.
///
/// `reason_json` is a JSON TransactionReason. `reference` may be null.
/// Returns 0 on success.
///
/// # Safety
/// `ledger` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_ledger_credit(
    ledger: *const FortuneLedger,
    pubkey: *const c_char,
    amount: i64,
    reason_json: *const c_char,
    reference: *const c_char,
) -> i32 {
    clear_last_error();

    let ledger = unsafe { &*ledger };
    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_fortune_ledger_credit: invalid pubkey");
        return -1;
    };

    let Some(rj) = c_str_to_str(reason_json) else {
        set_last_error("divi_fortune_ledger_credit: invalid reason_json");
        return -1;
    };

    let reason: TransactionReason = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_fortune_ledger_credit: {e}"));
            return -1;
        }
    };

    let ref_opt = c_str_to_str(reference).map(String::from);

    let mut guard = lock_or_recover(&ledger.0);
    guard.credit(pk, amount, reason, ref_opt);
    0
}

/// Debit Cool from a pubkey.
///
/// Returns 0 on success, -1 on error (insufficient balance).
///
/// # Safety
/// `ledger` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_ledger_debit(
    ledger: *const FortuneLedger,
    pubkey: *const c_char,
    amount: i64,
    reason_json: *const c_char,
    reference: *const c_char,
) -> i32 {
    clear_last_error();

    let ledger = unsafe { &*ledger };
    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_fortune_ledger_debit: invalid pubkey");
        return -1;
    };

    let Some(rj) = c_str_to_str(reason_json) else {
        set_last_error("divi_fortune_ledger_debit: invalid reason_json");
        return -1;
    };

    let reason: TransactionReason = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_fortune_ledger_debit: {e}"));
            return -1;
        }
    };

    let ref_opt = c_str_to_str(reference).map(String::from);

    let mut guard = lock_or_recover(&ledger.0);
    match guard.debit(pk, amount, reason, ref_opt) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Transfer Cool between two pubkeys.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `ledger` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_ledger_transfer(
    ledger: *const FortuneLedger,
    sender: *const c_char,
    recipient: *const c_char,
    amount: i64,
    reference: *const c_char,
) -> i32 {
    clear_last_error();

    let ledger = unsafe { &*ledger };
    let Some(s) = c_str_to_str(sender) else {
        set_last_error("divi_fortune_ledger_transfer: invalid sender");
        return -1;
    };

    let Some(r) = c_str_to_str(recipient) else {
        set_last_error("divi_fortune_ledger_transfer: invalid recipient");
        return -1;
    };

    let ref_opt = c_str_to_str(reference).map(String::from);

    let mut guard = lock_or_recover(&ledger.0);
    match guard.transfer(s, r, amount, ref_opt) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get transaction history for a pubkey.
///
/// Returns JSON array of Transactions. Caller must free via `divi_free_string`.
///
/// # Safety
/// `ledger` must be a valid pointer. `pubkey` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_ledger_transactions(
    ledger: *const FortuneLedger,
    pubkey: *const c_char,
) -> *mut c_char {
    let ledger = unsafe { &*ledger };
    let Some(pk) = c_str_to_str(pubkey) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&ledger.0);
    let txs: Vec<&Transaction> = guard.transactions_for(pk);
    json_to_c(&txs)
}

/// Get a transaction summary for a pubkey.
///
/// Returns JSON (TransactionSummary). Caller must free via `divi_free_string`.
///
/// # Safety
/// `ledger` must be a valid pointer. `pubkey` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_ledger_summary(
    ledger: *const FortuneLedger,
    pubkey: *const c_char,
) -> *mut c_char {
    let ledger = unsafe { &*ledger };
    let Some(pk) = c_str_to_str(pubkey) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&ledger.0);
    json_to_c(&guard.summary(pk))
}

// ===================================================================
// Treasury — opaque pointer (tracks money supply)
// ===================================================================

pub struct FortuneTreasury(pub(crate) Mutex<Treasury>);

/// Create a new treasury with a policy.
///
/// `policy_json` is a JSON FortunePolicy (or null for default).
/// Free with `divi_fortune_treasury_free`.
///
/// # Safety
/// `policy_json` may be null (uses default).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_treasury_new(
    policy_json: *const c_char,
) -> *mut FortuneTreasury {
    let policy = if policy_json.is_null() {
        FortunePolicy::default()
    } else if let Some(pj) = c_str_to_str(policy_json) {
        serde_json::from_str(pj).unwrap_or_default()
    } else {
        FortunePolicy::default()
    };

    Box::into_raw(Box::new(FortuneTreasury(Mutex::new(Treasury::new(policy)))))
}

/// Free a treasury.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_treasury_free(ptr: *mut FortuneTreasury) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr)); }
    }
}

/// Get treasury status.
///
/// Returns JSON (TreasuryStatus). Caller must free via `divi_free_string`.
///
/// # Safety
/// `treasury` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_treasury_status(
    treasury: *const FortuneTreasury,
) -> *mut c_char {
    let treasury = unsafe { &*treasury };
    let guard = lock_or_recover(&treasury.0);
    json_to_c(&guard.status())
}

/// Mint Cool into the treasury for a recipient.
///
/// `reason_json` is a JSON MintReason. Returns the actual amount minted (may be capped),
/// or -1 on error.
///
/// # Safety
/// `treasury` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_treasury_mint(
    treasury: *const FortuneTreasury,
    amount: i64,
    recipient: *const c_char,
    reason_json: *const c_char,
) -> i64 {
    clear_last_error();

    let treasury = unsafe { &*treasury };
    let Some(r) = c_str_to_str(recipient) else {
        set_last_error("divi_fortune_treasury_mint: invalid recipient");
        return -1;
    };

    let Some(rj) = c_str_to_str(reason_json) else {
        set_last_error("divi_fortune_treasury_mint: invalid reason_json");
        return -1;
    };

    let reason: MintReason = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_fortune_treasury_mint: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&treasury.0);
    match guard.mint(amount, r, reason) {
        Ok(minted) => minted,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Update network metrics in the treasury.
///
/// `metrics_json` is a JSON NetworkMetrics.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `treasury` must be a valid pointer. `metrics_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_treasury_update_metrics(
    treasury: *const FortuneTreasury,
    metrics_json: *const c_char,
) -> i32 {
    clear_last_error();

    let treasury = unsafe { &*treasury };
    let Some(mj) = c_str_to_str(metrics_json) else {
        set_last_error("divi_fortune_treasury_update_metrics: invalid metrics_json");
        return -1;
    };

    let metrics: NetworkMetrics = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_fortune_treasury_update_metrics: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&treasury.0);
    guard.update_metrics(metrics);
    0
}

// ===================================================================
// UBI Distributor — opaque pointer
// ===================================================================

pub struct FortuneUbi(pub(crate) Mutex<UbiDistributor>);

/// Create a new UBI distributor.
/// Free with `divi_fortune_ubi_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_fortune_ubi_new() -> *mut FortuneUbi {
    Box::into_raw(Box::new(FortuneUbi(Mutex::new(UbiDistributor::new()))))
}

/// Free a UBI distributor.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_ubi_free(ptr: *mut FortuneUbi) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr)); }
    }
}

/// Mark a pubkey as verified for UBI eligibility.
///
/// # Safety
/// `ubi` must be a valid pointer. `pubkey` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_ubi_verify_identity(
    ubi: *const FortuneUbi,
    pubkey: *const c_char,
) {
    let ubi = unsafe { &*ubi };
    if let Some(pk) = c_str_to_str(pubkey) {
        let mut guard = lock_or_recover(&ubi.0);
        guard.verify_identity(pk);
    }
}

/// Check UBI eligibility for a pubkey.
///
/// Returns JSON (UbiEligibility). Caller must free via `divi_free_string`.
///
/// # Safety
/// All pointers must be valid. `policy_json` may be null (uses default).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_ubi_check_eligibility(
    ubi: *const FortuneUbi,
    pubkey: *const c_char,
    ledger: *const FortuneLedger,
    treasury: *const FortuneTreasury,
    policy_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let ubi = unsafe { &*ubi };
    let ledger = unsafe { &*ledger };
    let treasury = unsafe { &*treasury };

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_fortune_ubi_check_eligibility: invalid pubkey");
        return std::ptr::null_mut();
    };

    let policy = if policy_json.is_null() {
        FortunePolicy::default()
    } else if let Some(pj) = c_str_to_str(policy_json) {
        serde_json::from_str(pj).unwrap_or_default()
    } else {
        FortunePolicy::default()
    };

    let ubi_guard = lock_or_recover(&ubi.0);
    let ledger_guard = lock_or_recover(&ledger.0);
    let treasury_guard = lock_or_recover(&treasury.0);

    let eligibility = ubi_guard.check_eligibility(pk, &ledger_guard, &treasury_guard, &policy);
    json_to_c(&eligibility)
}

/// Claim UBI for a pubkey.
///
/// Returns JSON (ClaimRecord) on success. Caller must free via `divi_free_string`.
/// Returns null on error.
///
/// # Safety
/// All pointers must be valid. `policy_json` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_ubi_claim(
    ubi: *const FortuneUbi,
    pubkey: *const c_char,
    ledger: *const FortuneLedger,
    treasury: *const FortuneTreasury,
    policy_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let ubi = unsafe { &*ubi };
    let ledger = unsafe { &*ledger };
    let treasury = unsafe { &*treasury };

    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_fortune_ubi_claim: invalid pubkey");
        return std::ptr::null_mut();
    };

    let policy = if policy_json.is_null() {
        FortunePolicy::default()
    } else if let Some(pj) = c_str_to_str(policy_json) {
        serde_json::from_str(pj).unwrap_or_default()
    } else {
        FortunePolicy::default()
    };

    let mut ubi_guard = lock_or_recover(&ubi.0);
    let mut ledger_guard = lock_or_recover(&ledger.0);
    let mut treasury_guard = lock_or_recover(&treasury.0);

    match ubi_guard.claim(pk, &mut ledger_guard, &mut treasury_guard, &policy) {
        Ok(record) => json_to_c(&record),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// Demurrage — stateless preview
// ===================================================================

/// Preview demurrage for a pubkey.
///
/// Returns JSON (DemurragePreview). Caller must free via `divi_free_string`.
///
/// # Safety
/// `ledger` must be a valid pointer. C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_demurrage_preview(
    pubkey: *const c_char,
    ledger: *const FortuneLedger,
    policy_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let ledger = unsafe { &*ledger };
    let Some(pk) = c_str_to_str(pubkey) else {
        set_last_error("divi_fortune_demurrage_preview: invalid pubkey");
        return std::ptr::null_mut();
    };

    let policy = if policy_json.is_null() {
        FortunePolicy::default()
    } else if let Some(pj) = c_str_to_str(policy_json) {
        serde_json::from_str(pj).unwrap_or_default()
    } else {
        FortunePolicy::default()
    };

    let engine = DemurrageEngine::new();
    let guard = lock_or_recover(&ledger.0);
    let preview = engine.preview(pk, &guard, &policy);
    json_to_c(&preview)
}

// ===================================================================
// Policy — defaults
// ===================================================================

/// Get the default FortunePolicy as JSON.
///
/// Returns JSON (FortunePolicy). Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_fortune_policy_default() -> *mut c_char {
    json_to_c(&FortunePolicy::default())
}

// ===================================================================
// Trade — JSON round-trip
// ===================================================================

/// Create a trade proposal.
///
/// Returns JSON (TradeProposal). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_trade_new(
    proposer: *const c_char,
    recipient: *const c_char,
    offering_cool: i64,
    requesting_cool: i64,
) -> *mut c_char {
    clear_last_error();

    let Some(p) = c_str_to_str(proposer) else {
        set_last_error("divi_fortune_trade_new: invalid proposer");
        return std::ptr::null_mut();
    };

    let Some(r) = c_str_to_str(recipient) else {
        set_last_error("divi_fortune_trade_new: invalid recipient");
        return std::ptr::null_mut();
    };

    match TradeProposal::new(p, r, offering_cool, requesting_cool) {
        Ok(trade) => json_to_c(&trade),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Accept a trade proposal.
///
/// Takes trade JSON, returns modified trade JSON.
///
/// # Safety
/// `trade_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_trade_accept(
    trade_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(trade_json) else {
        set_last_error("divi_fortune_trade_accept: invalid trade_json");
        return std::ptr::null_mut();
    };

    let mut trade: TradeProposal = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_fortune_trade_accept: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = trade.accept() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&trade)
}

/// Execute a trade proposal.
///
/// Takes trade JSON, returns modified trade JSON.
///
/// # Safety
/// `trade_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_trade_execute(
    trade_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(trade_json) else {
        set_last_error("divi_fortune_trade_execute: invalid trade_json");
        return std::ptr::null_mut();
    };

    let mut trade: TradeProposal = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_fortune_trade_execute: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = trade.execute() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&trade)
}

// ===================================================================
// Cash — utility functions
// ===================================================================

/// Generate a random cash serial number.
///
/// Returns a C string. Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_fortune_cash_generate_serial() -> *mut c_char {
    string_to_c(fortune::cash::note::generate_serial())
}

/// Validate a cash serial number format.
///
/// Returns true if valid.
///
/// # Safety
/// `serial` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_fortune_cash_validate_serial(
    serial: *const c_char,
) -> bool {
    let Some(s) = c_str_to_str(serial) else {
        return false;
    };
    fortune::cash::note::validate_serial(s)
}
