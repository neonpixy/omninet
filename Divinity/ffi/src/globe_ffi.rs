use std::ffi::c_char;
use std::os::raw::c_void;
use std::sync::{Arc, Mutex};

use globe::{EventBuilder, GlobeConfig, OmniEvent, OmniFilter, RelayPool, UnsignedEvent};
use serde::Deserialize;
use url::Url;

use crate::crown_ffi::CrownKeyring;
use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover, string_to_c};
use crate::runtime_ffi::DiviRuntime;
use crate::{clear_last_error, set_last_error};

// ===================================================================
// Event building (sync — no runtime needed)
// ===================================================================

/// Deserializable version of UnsignedEvent for FFI input.
#[derive(Deserialize)]
struct UnsignedEventInput {
    kind: u32,
    #[serde(default)]
    tags: Vec<Vec<String>>,
    #[serde(default)]
    content: String,
}

impl From<UnsignedEventInput> for UnsignedEvent {
    fn from(input: UnsignedEventInput) -> Self {
        let mut event = UnsignedEvent::new(input.kind, input.content);
        event.tags = input.tags;
        event
    }
}

/// Build and sign a text note event (kind 1).
///
/// Returns JSON OmniEvent, or null on error. Free via `divi_free_string`.
///
/// # Safety
/// `content` must be a valid C string. `keyring` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_event_text_note(
    content: *const c_char,
    keyring: *const CrownKeyring,
) -> *mut c_char {
    clear_last_error();
    let Some(content_str) = c_str_to_str(content) else {
        set_last_error("divi_globe_event_text_note: invalid content");
        return std::ptr::null_mut();
    };
    let keyring = unsafe { &*keyring };
    let guard = lock_or_recover(&keyring.0);

    let keypair = match extract_keypair(&guard) {
        Ok(kp) => kp,
        Err(msg) => {
            set_last_error(msg);
            return std::ptr::null_mut();
        }
    };

    match EventBuilder::text_note(content_str, &keypair) {
        Ok(event) => json_to_c(&event),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Sign an unsigned event (JSON) with a keyring.
///
/// Input JSON: `{"kind": 1, "tags": [...], "content": "..."}`
/// Returns JSON OmniEvent, or null on error. Free via `divi_free_string`.
///
/// # Safety
/// `unsigned_json` must be a valid C string. `keyring` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_event_sign(
    unsigned_json: *const c_char,
    keyring: *const CrownKeyring,
) -> *mut c_char {
    clear_last_error();
    let Some(json_str) = c_str_to_str(unsigned_json) else {
        set_last_error("divi_globe_event_sign: invalid json");
        return std::ptr::null_mut();
    };
    let keyring = unsafe { &*keyring };
    let guard = lock_or_recover(&keyring.0);

    let input: UnsignedEventInput = match serde_json::from_str(json_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_globe_event_sign: JSON parse error: {e}"));
            return std::ptr::null_mut();
        }
    };
    let unsigned: UnsignedEvent = input.into();

    let keypair = match extract_keypair(&guard) {
        Ok(kp) => kp,
        Err(msg) => {
            set_last_error(msg);
            return std::ptr::null_mut();
        }
    };

    match EventBuilder::sign(&unsigned, &keypair) {
        Ok(event) => json_to_c(&event),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Verify an event's ID and signature.
///
/// Returns true if valid, false otherwise.
///
/// # Safety
/// `event_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_event_verify(event_json: *const c_char) -> bool {
    let Some(json_str) = c_str_to_str(event_json) else {
        return false;
    };

    let event: OmniEvent = match serde_json::from_str(json_str) {
        Ok(e) => e,
        Err(_) => return false,
    };

    EventBuilder::verify(&event).unwrap_or(false)
}

// ===================================================================
// Filter building (sync)
// ===================================================================

/// Build an OmniFilter for a user's profile (kind 0).
///
/// `pubkey_hex` is the 64-char hex public key.
/// Returns JSON filter. Free via `divi_free_string`.
///
/// # Safety
/// `pubkey_hex` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_filter_for_profile(
    pubkey_hex: *const c_char,
) -> *mut c_char {
    let Some(pk) = c_str_to_str(pubkey_hex) else {
        return std::ptr::null_mut();
    };
    let filter = OmniFilter::for_profile(pk);
    json_to_c(&filter)
}

/// Build an OmniFilter for a user's contact list (kind 3).
///
/// Returns JSON filter. Free via `divi_free_string`.
///
/// # Safety
/// `pubkey_hex` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_filter_for_contact_list(
    pubkey_hex: *const c_char,
) -> *mut c_char {
    let Some(pk) = c_str_to_str(pubkey_hex) else {
        return std::ptr::null_mut();
    };
    let filter = OmniFilter::for_contact_list(pk);
    json_to_c(&filter)
}

/// Build an OmniFilter from JSON.
///
/// Returns JSON filter (round-trip for validation). Free via `divi_free_string`.
///
/// # Safety
/// `json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_filter_from_json(
    json: *const c_char,
) -> *mut c_char {
    let Some(json_str) = c_str_to_str(json) else {
        return std::ptr::null_mut();
    };
    let filter: OmniFilter = match serde_json::from_str(json_str) {
        Ok(f) => f,
        Err(e) => {
            set_last_error(format!("invalid filter JSON: {e}"));
            return std::ptr::null_mut();
        }
    };
    json_to_c(&filter)
}

// ===================================================================
// Relay pool (async — requires runtime)
// ===================================================================

/// Callback for receiving pool events.
pub type GlobeEventCallback =
    extern "C" fn(event_json: *const c_char, source_relay: *const c_char, context: *mut c_void);

/// Wraps RelayPool with a runtime reference for async operations.
pub struct GlobePool {
    pool: Mutex<RelayPool>,
    runtime: Arc<tokio::runtime::Runtime>,
}

/// Create a new relay pool.
///
/// `runtime` must be a valid DiviRuntime pointer.
/// `config_json` is optional — pass null for defaults.
/// Returns a pool pointer. Free with `divi_globe_pool_free`.
///
/// # Safety
/// `runtime` must be a valid pointer from `divi_runtime_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_pool_new(
    runtime: *const DiviRuntime,
    config_json: *const c_char,
) -> *mut GlobePool {
    let runtime = unsafe { &*runtime };

    let config = if config_json.is_null() {
        GlobeConfig::default()
    } else if let Some(json_str) = c_str_to_str(config_json) {
        match serde_json::from_str(json_str) {
            Ok(c) => c,
            Err(e) => {
                set_last_error(format!("invalid config JSON: {e}"));
                return std::ptr::null_mut();
            }
        }
    } else {
        GlobeConfig::default()
    };

    let _guard = runtime.runtime.enter();
    let pool = RelayPool::new(config);

    Box::into_raw(Box::new(GlobePool {
        pool: Mutex::new(pool),
        runtime: runtime.runtime.clone(),
    }))
}

/// Free a relay pool.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_globe_pool_new`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_pool_free(ptr: *mut GlobePool) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Add a relay to the pool and connect.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `pool` must be a valid pointer. `url` must be a valid C string (e.g., "ws://localhost:8080").
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_pool_add_relay(
    pool: *const GlobePool,
    url: *const c_char,
) -> i32 {
    clear_last_error();
    let pool = unsafe { &*pool };
    let Some(url_str) = c_str_to_str(url) else {
        set_last_error("divi_globe_pool_add_relay: invalid url");
        return -1;
    };

    let parsed = match Url::parse(url_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("invalid URL: {e}"));
            return -1;
        }
    };

    let _guard = pool.runtime.enter();
    let mut pool_guard = lock_or_recover(&pool.pool);
    match pool_guard.add_relay(parsed) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Publish an event to all connected relays.
///
/// `event_json` is a JSON OmniEvent.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `pool` must be a valid pointer. `event_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_pool_publish(
    pool: *const GlobePool,
    event_json: *const c_char,
) -> i32 {
    clear_last_error();
    let pool = unsafe { &*pool };
    let Some(json_str) = c_str_to_str(event_json) else {
        set_last_error("divi_globe_pool_publish: invalid json");
        return -1;
    };

    let event: OmniEvent = match serde_json::from_str(json_str) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_globe_pool_publish: JSON parse error: {e}"));
            return -1;
        }
    };

    // publish is async — block on it. We must not hold the pool lock across await.
    let pool_guard = lock_or_recover(&pool.pool);
    let result = pool.runtime.block_on(pool_guard.publish(event));
    drop(pool_guard);

    match result {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Subscribe to events matching the given filters.
///
/// `filters_json` is a JSON array of OmniFilter objects.
/// Returns the subscription ID as a string, or null on error.
/// Free the returned string via `divi_free_string`.
///
/// # Safety
/// `pool` must be a valid pointer. `filters_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_pool_subscribe(
    pool: *const GlobePool,
    filters_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let pool = unsafe { &*pool };
    let Some(json_str) = c_str_to_str(filters_json) else {
        set_last_error("divi_globe_pool_subscribe: invalid json");
        return std::ptr::null_mut();
    };

    let filters: Vec<OmniFilter> = match serde_json::from_str(json_str) {
        Ok(f) => f,
        Err(e) => {
            set_last_error(format!("divi_globe_pool_subscribe: JSON parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let _guard = pool.runtime.enter();
    let mut pool_guard = lock_or_recover(&pool.pool);
    let (sub_id, _rx) = pool_guard.subscribe(filters);

    string_to_c(sub_id)
}

/// Register a callback for incoming pool events.
///
/// The callback fires on a background thread for each deduplicated event.
/// Only one callback can be active — calling again replaces the previous one.
///
/// Events arrive as JSON strings. The callback receives the event JSON,
/// the source relay URL, and the context pointer.
///
/// # Safety
/// `pool` must be a valid pointer. `context` must be valid for the lifetime
/// of the callback, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_pool_on_event(
    pool: *const GlobePool,
    callback: GlobeEventCallback,
    context: *mut c_void,
) {
    let pool = unsafe { &*pool };
    let ctx = context as usize;

    // Get a broadcast receiver by subscribing with no filters.
    // The receiver gets ALL events from all forwarding tasks regardless.
    let _enter = pool.runtime.enter();
    let mut pool_guard = lock_or_recover(&pool.pool);
    let (_sub_id, mut rx) = pool_guard.subscribe(vec![]);
    drop(pool_guard);

    let cb = callback;
    pool.runtime.spawn(async move {
        loop {
            match rx.recv().await {
                Ok(pool_event) => {
                    let event_json = match serde_json::to_string(&pool_event.event) {
                        Ok(j) => j,
                        Err(_) => continue,
                    };
                    let relay_str = pool_event.source_relay.to_string();

                    if let (Ok(ej), Ok(rs)) = (
                        std::ffi::CString::new(event_json),
                        std::ffi::CString::new(relay_str),
                    ) {
                        cb(ej.as_ptr(), rs.as_ptr(), ctx as *mut c_void);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    log::warn!("FFI event callback lagged {n} events");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

// ===================================================================
// Privacy — padding, shaping, forwarding, anonymous
// ===================================================================

/// Create a BucketPaddingConfig from a mode string.
///
/// `mode` is one of: `"disabled"`, `"minimal"`, `"standard"`, `"aggressive"`.
/// Returns JSON BucketPaddingConfig. Caller must free via `divi_free_string`.
///
/// # Safety
/// `mode` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_padding_config_new(
    mode: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let Some(mode_str) = c_str_to_str(mode) else {
        set_last_error("divi_globe_padding_config_new: invalid mode string");
        return std::ptr::null_mut();
    };

    let padding_mode = match mode_str.to_lowercase().as_str() {
        "disabled" => globe::PaddingMode::Disabled,
        "minimal" => globe::PaddingMode::Minimal,
        "standard" => globe::PaddingMode::Standard,
        "aggressive" => globe::PaddingMode::Aggressive,
        other => {
            set_last_error(format!(
                "divi_globe_padding_config_new: unknown mode '{other}', expected disabled|minimal|standard|aggressive"
            ));
            return std::ptr::null_mut();
        }
    };

    let config = globe::BucketPaddingConfig {
        mode: padding_mode,
        custom_bucket_size: None,
        pad_binary_frames: false,
    };

    json_to_c(&config)
}

/// Pad data to a bucket size boundary.
///
/// Returns padded bytes. Caller must free via `divi_free_bytes(ptr, *out_len)`.
/// Returns null on error.
///
/// # Safety
/// `data` must be valid for `data_len` bytes. `out_len` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_pad_to_bucket(
    data: *const u8,
    data_len: usize,
    bucket_size: usize,
    out_len: *mut usize,
) -> *mut u8 {
    clear_last_error();

    if bucket_size < 5 {
        set_last_error("divi_globe_pad_to_bucket: bucket_size must be >= 5");
        return std::ptr::null_mut();
    }

    let slice = if data.is_null() || data_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }
    };

    let padded = globe::pad_to_bucket(slice, bucket_size);
    let (ptr, len) = crate::helpers::bytes_to_owned(padded);
    unsafe { *out_len = len; }
    ptr
}

/// Remove bucket padding and recover original data.
///
/// Returns unpadded bytes. Caller must free via `divi_free_bytes(ptr, *out_len)`.
/// Returns null on error.
///
/// # Safety
/// `data` must be valid for `data_len` bytes. `out_len` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_unpad(
    data: *const u8,
    data_len: usize,
    out_len: *mut usize,
) -> *mut u8 {
    clear_last_error();

    if data.is_null() || data_len == 0 {
        set_last_error("divi_globe_unpad: null or empty data");
        return std::ptr::null_mut();
    }

    let slice = unsafe { std::slice::from_raw_parts(data, data_len) };

    match globe::privacy::unpad(slice) {
        Ok(original) => {
            let (ptr, len) = crate::helpers::bytes_to_owned(original);
            unsafe { *out_len = len; }
            ptr
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Create a ShapingConfig with the given parameters.
///
/// Returns JSON ShapingConfig. Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_globe_shaping_config_new(
    bucket_secs: u32,
    jitter_min_ms: u64,
    jitter_max_ms: u64,
) -> *mut c_char {
    clear_last_error();

    let config = globe::ShapingConfig {
        enabled: true,
        timestamp_bucket_secs: bucket_secs,
        publish_jitter_min_ms: jitter_min_ms,
        publish_jitter_max_ms: jitter_max_ms,
        response_batch_interval_ms: 200,
    };

    if let Err(e) = config.validate() {
        set_last_error(format!("divi_globe_shaping_config_new: {e}"));
        return std::ptr::null_mut();
    }

    json_to_c(&config)
}

/// Build a forwarding envelope for multi-hop relay delivery.
///
/// `payload` is the (typically onion-encrypted) message body.
/// `path_json` is a JSON array of relay URL strings.
/// Returns JSON ForwardEnvelope. Caller must free via `divi_free_string`.
///
/// # Safety
/// `payload` must be valid for `payload_len` bytes. `path_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_forward_envelope_create(
    payload: *const u8,
    payload_len: usize,
    path_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(pj) = c_str_to_str(path_json) else {
        set_last_error("divi_globe_forward_envelope_create: invalid path_json");
        return std::ptr::null_mut();
    };

    let path: Vec<String> = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_globe_forward_envelope_create: path JSON: {e}"));
            return std::ptr::null_mut();
        }
    };

    let data = if payload.is_null() || payload_len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(payload, payload_len) }
    };

    match globe::build_forward_envelope(data, &path) {
        Ok(envelope) => json_to_c(&envelope),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Process a forwarding envelope at the current relay.
///
/// `envelope_json` is a JSON ForwardEnvelope.
/// Returns JSON with either `{"action":"forward","next":"url","envelope":{...}}`
/// or `{"action":"deliver","payload":[...]}`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `envelope_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_forward_process(
    envelope_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(ej) = c_str_to_str(envelope_json) else {
        set_last_error("divi_globe_forward_process: invalid envelope_json");
        return std::ptr::null_mut();
    };

    let envelope: globe::ForwardEnvelope = match serde_json::from_str(ej) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_globe_forward_process: envelope JSON: {e}"));
            return std::ptr::null_mut();
        }
    };

    match globe::process_forward(envelope) {
        Ok(action) => {
            let result = match action {
                globe::ForwardAction::Forward { next, envelope } => {
                    serde_json::json!({
                        "action": "forward",
                        "next": next,
                        "envelope": serde_json::to_value(&envelope).unwrap_or_default(),
                    })
                }
                globe::ForwardAction::Deliver { payload } => {
                    serde_json::json!({
                        "action": "deliver",
                        "payload": payload,
                    })
                }
            };
            json_to_c(&result)
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Create an ephemeral session for anonymous subscriptions.
///
/// Returns JSON EphemeralSession. Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_globe_ephemeral_session_create() -> *mut c_char {
    clear_last_error();
    let session = globe::create_ephemeral_session();
    json_to_c(&session)
}

/// Create an anonymous auth response for a relay challenge.
///
/// `challenge` is the relay's challenge string.
/// `relay_url` is the relay URL.
/// `session_json` is a JSON EphemeralSession.
/// Returns JSON AnonymousAuthResponse. Caller must free via `divi_free_string`.
///
/// # Safety
/// All parameters must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_anonymous_auth(
    challenge: *const c_char,
    relay_url: *const c_char,
    session_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(ch) = c_str_to_str(challenge) else {
        set_last_error("divi_globe_anonymous_auth: invalid challenge");
        return std::ptr::null_mut();
    };
    let Some(ru) = c_str_to_str(relay_url) else {
        set_last_error("divi_globe_anonymous_auth: invalid relay_url");
        return std::ptr::null_mut();
    };
    let Some(sj) = c_str_to_str(session_json) else {
        set_last_error("divi_globe_anonymous_auth: invalid session_json");
        return std::ptr::null_mut();
    };

    let session: globe::EphemeralSession = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_globe_anonymous_auth: session JSON: {e}"));
            return std::ptr::null_mut();
        }
    };

    match globe::create_anonymous_auth(ch, ru, &session) {
        Ok(response) => json_to_c(&response),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Strip the author field from an event JSON string.
///
/// `event_json` is a JSON OmniEvent.
/// Returns JSON with the author field replaced by an empty string.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `event_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_strip_author(
    event_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(ej) = c_str_to_str(event_json) else {
        set_last_error("divi_globe_strip_author: invalid event_json");
        return std::ptr::null_mut();
    };

    match globe::strip_author_from_event(ej) {
        Ok(stripped) => string_to_c(stripped),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// Internal helpers
// ===================================================================

/// Extract a CrownKeypair from a locked Keyring.
fn extract_keypair(keyring: &crown::Keyring) -> Result<crown::CrownKeypair, String> {
    let export = keyring
        .export()
        .map_err(|e| format!("keyring export failed: {e}"))?;

    let storage: serde_json::Value =
        serde_json::from_slice(&export).map_err(|e| format!("parse failed: {e}"))?;

    let hex_key = storage
        .get("primary_private_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "no primary private key".to_string())?;

    let key_bytes = hex::decode(hex_key).map_err(|e| format!("hex decode: {e}"))?;

    crown::CrownKeypair::from_private_key(&key_bytes).map_err(|e| format!("invalid key: {e}"))
}
