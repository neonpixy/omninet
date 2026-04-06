use std::ffi::c_char;
use std::os::raw::c_void;
use std::path::PathBuf;

use chrono::Utc;
use globe::event::OmniEvent;
use globe::filter::OmniFilter;
use omnibus::{LogEntry, Omnibus, OmnibusConfig};

use crate::helpers::{c_str_to_str, json_to_c, string_to_c};
use crate::{clear_last_error, set_last_error};

/// Callback for receiving Omnibus events.
pub type OmnibusEventCallback =
    extern "C" fn(event_json: *const c_char, source_relay: *const c_char, context: *mut c_void);

// ===================================================================
// Lifecycle
// ===================================================================

/// Start an Omnibus instance.
///
/// `device_name` — human-readable name (e.g., "Sam's Mac").
/// `port` — relay server port. 0 = OS-assigned.
/// `bind_all` — if true, listens on all interfaces (LAN reachable).
///
/// Returns an Omnibus pointer. Free with `divi_omnibus_free`.
///
/// # Safety
/// `device_name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_start(
    device_name: *const c_char,
    port: u16,
    bind_all: bool,
) -> *mut Omnibus {
    clear_last_error();
    let name = c_str_to_str(device_name).unwrap_or("Omnidea Device");

    let config = OmnibusConfig {
        device_name: name.into(),
        port,
        bind_all,
        ..Default::default()
    };

    match Omnibus::start(config) {
        Ok(omni) => Box::into_raw(Box::new(omni)),
        Err(e) => {
            set_last_error(format!("omnibus start failed: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Free an Omnibus instance.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_omnibus_start`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_free(ptr: *mut Omnibus) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

// ===================================================================
// Identity
// ===================================================================

/// Create a new identity with a display name.
///
/// Returns the crown_id (bech32 public key) as a C string, or null on error.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer. `display_name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_create_identity(
    omni: *const Omnibus,
    display_name: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let omni = unsafe { &*omni };
    let Some(name) = c_str_to_str(display_name) else {
        set_last_error("divi_omnibus_create_identity: invalid display_name");
        return std::ptr::null_mut();
    };

    match omni.create_identity(name) {
        Ok(crown_id) => string_to_c(crown_id),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Get the public key (crown_id bech32), or null if no identity.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_pubkey(omni: *const Omnibus) -> *mut c_char {
    let omni = unsafe { &*omni };
    match omni.pubkey() {
        Some(pk) => string_to_c(pk),
        None => std::ptr::null_mut(),
    }
}

/// Export the keyring as JSON bytes.
/// Output written to `out_data`/`out_len`. Caller must free via `divi_free_bytes`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_export_keyring(
    omni: *const Omnibus,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    clear_last_error();
    let omni = unsafe { &*omni };
    match omni.export_keyring() {
        Ok(bytes) => {
            let (ptr, len) = crate::helpers::bytes_to_owned(bytes);
            unsafe {
                *out_data = ptr;
                *out_len = len;
            }
            0
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get the public key as hex, or null if no identity.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_pubkey_hex(omni: *const Omnibus) -> *mut c_char {
    let omni = unsafe { &*omni };
    match omni.pubkey_hex() {
        Some(pk) => string_to_c(pk),
        None => std::ptr::null_mut(),
    }
}

/// Get the current profile as JSON, or null if no identity.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_profile_json(omni: *const Omnibus) -> *mut c_char {
    let omni = unsafe { &*omni };
    match omni.profile_json() {
        Some(json) => string_to_c(json),
        None => std::ptr::null_mut(),
    }
}

/// Update the display name and re-publish profile.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `omni` must be a valid pointer. `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_update_display_name(
    omni: *const Omnibus,
    name: *const c_char,
) -> i32 {
    clear_last_error();
    let omni = unsafe { &*omni };
    let Some(name_str) = c_str_to_str(name) else {
        set_last_error("divi_omnibus_update_display_name: invalid name");
        return -1;
    };

    match omni.update_display_name(name_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

// ===================================================================
// Network
// ===================================================================

/// Publish a text note. Signs with the loaded identity.
///
/// Returns the signed event as JSON, or null on error.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer. `content` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_post(
    omni: *const Omnibus,
    content: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let omni = unsafe { &*omni };
    let Some(text) = c_str_to_str(content) else {
        set_last_error("divi_omnibus_post: invalid content");
        return std::ptr::null_mut();
    };

    match omni.post(text) {
        Ok(event) => json_to_c(&event),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Publish a pre-signed event to all connected relays.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `omni` must be a valid pointer. `event_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_publish(
    omni: *const Omnibus,
    event_json: *const c_char,
) -> i32 {
    clear_last_error();
    let omni = unsafe { &*omni };
    let Some(json_str) = c_str_to_str(event_json) else {
        set_last_error("divi_omnibus_publish: invalid json");
        return -1;
    };

    let event: OmniEvent = match serde_json::from_str(json_str) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_omnibus_publish: parse error: {e}"));
            return -1;
        }
    };

    match omni.publish(event) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Inject an event directly into the local relay store (bypasses network).
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `omni` must be a valid pointer. `event_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_seed_event(
    omni: *const Omnibus,
    event_json: *const c_char,
) -> i32 {
    clear_last_error();
    let omni = unsafe { &*omni };
    let Some(json_str) = c_str_to_str(event_json) else {
        set_last_error("divi_omnibus_seed_event: invalid json");
        return -1;
    };

    let event: OmniEvent = match serde_json::from_str(json_str) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_omnibus_seed_event: parse error: {e}"));
            return -1;
        }
    };

    omni.seed_event(event);
    0
}

/// Connect to a specific relay.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `omni` must be a valid pointer. `url` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_connect_relay(
    omni: *const Omnibus,
    url: *const c_char,
) -> i32 {
    clear_last_error();
    let omni = unsafe { &*omni };
    let Some(url_str) = c_str_to_str(url) else {
        set_last_error("divi_omnibus_connect_relay: invalid url");
        return -1;
    };

    match omni.connect_relay(url_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Set a home node for persistent sync.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `omni` must be a valid pointer. `url` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_set_home_node(
    omni: *const Omnibus,
    url: *const c_char,
) -> i32 {
    clear_last_error();
    let omni = unsafe { &*omni };
    let Some(url_str) = c_str_to_str(url) else {
        set_last_error("divi_omnibus_set_home_node: invalid url");
        return -1;
    };

    match omni.set_home_node(url_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Query events from the local relay store.
///
/// `filter_json` is a JSON OmniFilter object.
/// Returns a JSON array of matching events. Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer. `filter_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_query(
    omni: *const Omnibus,
    filter_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let omni = unsafe { &*omni };
    let Some(json_str) = c_str_to_str(filter_json) else {
        set_last_error("divi_omnibus_query: invalid json");
        return std::ptr::null_mut();
    };

    let filter: OmniFilter = match serde_json::from_str(json_str) {
        Ok(f) => f,
        Err(e) => {
            set_last_error(format!("divi_omnibus_query: parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let events = omni.query(&filter);
    json_to_c(&events)
}

// ===================================================================
// Events (subscribe + callback)
// ===================================================================

/// Subscribe to events matching filters.
///
/// `filters_json` is a JSON array of OmniFilter objects.
/// Returns the subscription ID as a C string. Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer. `filters_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_subscribe(
    omni: *const Omnibus,
    filters_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let omni = unsafe { &*omni };
    let Some(json_str) = c_str_to_str(filters_json) else {
        set_last_error("divi_omnibus_subscribe: invalid json");
        return std::ptr::null_mut();
    };

    let filters: Vec<OmniFilter> = match serde_json::from_str(json_str) {
        Ok(f) => f,
        Err(e) => {
            set_last_error(format!("divi_omnibus_subscribe: parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let (sub_id, _rx) = omni.subscribe(filters);
    string_to_c(sub_id)
}

/// Register a callback for ALL incoming events.
///
/// The callback fires on a background thread for each event.
/// Only one callback can be active — calling again replaces the previous one.
///
/// # Safety
/// `omni` must be a valid pointer. `context` must be valid for the
/// lifetime of the callback, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_on_event(
    omni: *const Omnibus,
    callback: OmnibusEventCallback,
    context: *mut c_void,
) {
    let omni = unsafe { &*omni };
    let ctx = context as usize;
    let cb = callback;

    let mut rx = omni.event_stream();
    let runtime = omni.runtime().clone();

    runtime.spawn(async move {
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
                    log::warn!("omnibus FFI event callback lagged {n} events");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

// ===================================================================
// Discovery
// ===================================================================

/// Get all currently discovered peers as a JSON array.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_peers(omni: *const Omnibus) -> *mut c_char {
    let omni = unsafe { &*omni };
    let peers = omni.peers();
    let serializable: Vec<PeerJson> = peers.into_iter().map(PeerJson::from).collect();
    json_to_c(&serializable)
}

/// Get the number of currently discovered peers.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_peer_count(omni: *const Omnibus) -> u32 {
    let omni = unsafe { &*omni };
    omni.peers().len() as u32
}

/// Connect to all currently discovered peers.
/// Returns the number of peers connected.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_connect_discovered_peers(omni: *const Omnibus) -> u32 {
    let omni = unsafe { &*omni };
    omni.connect_discovered_peers()
}

// ===================================================================
// Status
// ===================================================================

/// Get the full status as JSON.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_status(omni: *const Omnibus) -> *mut c_char {
    let omni = unsafe { &*omni };
    json_to_c(&omni.status())
}

/// Get the local relay server port.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_port(omni: *const Omnibus) -> u16 {
    let omni = unsafe { &*omni };
    omni.port()
}

/// Get the local relay server WebSocket URL.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_relay_url(omni: *const Omnibus) -> *mut c_char {
    let omni = unsafe { &*omni };
    string_to_c(omni.relay_url())
}

/// Get the public URL of this node (if UPnP mapping succeeded).
/// Returns null if UPnP is unavailable or mapping failed.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_public_url(omni: *const Omnibus) -> *mut c_char {
    let omni = unsafe { &*omni };
    match omni.public_url() {
        Some(url) => string_to_c(url),
        None => std::ptr::null_mut(),
    }
}

// ===================================================================
// Serializable peer type (reused from discovery_ffi)
// ===================================================================

#[derive(serde::Serialize)]
struct PeerJson {
    name: String,
    addresses: Vec<String>,
    port: u16,
    pubkey_hex: Option<String>,
    ws_url: Option<String>,
}

impl From<globe::discovery::local::LocalPeer> for PeerJson {
    fn from(peer: globe::discovery::local::LocalPeer) -> Self {
        let ws_url = peer.ws_url();
        Self {
            name: peer.name,
            addresses: peer.addresses.iter().map(|a| a.to_string()).collect(),
            port: peer.port,
            pubkey_hex: peer.pubkey_hex,
            ws_url,
        }
    }
}

// ===================================================================
// Identity (additional)
// ===================================================================

/// Load an existing identity from a directory path.
///
/// The path should contain a `soul/` subdirectory and optionally `keyring.dat`.
/// Returns the crown_id (bech32 public key) as a C string, or null on error.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer. `path` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_load_identity(
    omni: *const Omnibus,
    path: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let omni = unsafe { &*omni };
    let Some(path_str) = c_str_to_str(path) else {
        set_last_error("divi_omnibus_load_identity: invalid path");
        return std::ptr::null_mut();
    };

    match omni.load_identity(path_str) {
        Ok(crown_id) => string_to_c(crown_id),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Import a keyring from exported bytes (for syncing from another device).
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `omni` must be a valid pointer. `data` must point to `len` valid bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_import_keyring(
    omni: *const Omnibus,
    data: *const u8,
    len: usize,
) -> i32 {
    clear_last_error();
    let omni = unsafe { &*omni };
    if data.is_null() || len == 0 {
        set_last_error("divi_omnibus_import_keyring: null or empty data");
        return -1;
    }

    let slice = unsafe { std::slice::from_raw_parts(data, len) };
    match omni.import_keyring(slice) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

// ===================================================================
// Health & Diagnostics
// ===================================================================

/// Get health snapshots for all relays in the pool as a JSON array.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_relay_health(omni: *const Omnibus) -> *mut c_char {
    let omni = unsafe { &*omni };
    let health = omni.relay_health();
    json_to_c(&health)
}

/// Get health snapshot for a specific relay by URL as JSON.
/// Returns null if the relay is not found. Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer. `url` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_relay_health_for(
    omni: *const Omnibus,
    url: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let omni = unsafe { &*omni };
    let Some(url_str) = c_str_to_str(url) else {
        set_last_error("divi_omnibus_relay_health_for: invalid url");
        return std::ptr::null_mut();
    };

    match omni.relay_health_for(url_str) {
        Some(snap) => json_to_c(&snap),
        None => std::ptr::null_mut(),
    }
}

/// Get statistics about the local event store as JSON.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_store_stats(omni: *const Omnibus) -> *mut c_char {
    let omni = unsafe { &*omni };
    json_to_c(&omni.store_stats())
}

/// Get the most recent log entries as a JSON array.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_recent_logs(
    omni: *const Omnibus,
    count: u32,
) -> *mut c_char {
    let omni = unsafe { &*omni };
    let entries = omni.recent_logs(count as usize);
    json_to_c(&entries)
}

/// Push a log entry into the capture buffer.
///
/// `level` — log level string (e.g., "INFO", "WARN", "ERROR").
/// `module_or_null` — optional module name, or null.
/// `message` — the log message.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `omni` must be a valid pointer. `level` and `message` must be valid C strings.
/// `module_or_null` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_push_log(
    omni: *const Omnibus,
    level: *const c_char,
    module_or_null: *const c_char,
    message: *const c_char,
) -> i32 {
    clear_last_error();
    let omni = unsafe { &*omni };
    let Some(level_str) = c_str_to_str(level) else {
        set_last_error("divi_omnibus_push_log: invalid level");
        return -1;
    };
    let Some(msg_str) = c_str_to_str(message) else {
        set_last_error("divi_omnibus_push_log: invalid message");
        return -1;
    };
    let module = c_str_to_str(module_or_null).map(String::from);

    omni.push_log(LogEntry {
        timestamp: Utc::now(),
        level: level_str.into(),
        module,
        message: msg_str.into(),
    });
    0
}

// ===================================================================
// Gospel
// ===================================================================

/// Get the gospel registry snapshot as JSON, or null if no registry.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_gospel_json(omni: *const Omnibus) -> *mut c_char {
    let omni = unsafe { &*omni };
    match omni.gospel_registry() {
        Some(reg) => json_to_c(&reg.snapshot()),
        None => std::ptr::null_mut(),
    }
}

/// Save the gospel registry to the encrypted database.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_save_gospel(omni: *const Omnibus) {
    let omni = unsafe { &*omni };
    omni.save_gospel();
}

/// Register a gospel entry (an OmniEvent) into the gospel registry.
///
/// `entry_json` is a JSON-serialized OmniEvent.
/// Returns 0 on success (inserted or duplicate), -1 on error (rejected, no registry, or parse error).
///
/// # Safety
/// `omni` must be a valid pointer. `entry_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_register_gospel(
    omni: *const Omnibus,
    entry_json: *const c_char,
) -> i32 {
    clear_last_error();
    let omni = unsafe { &*omni };
    let Some(json_str) = c_str_to_str(entry_json) else {
        set_last_error("divi_omnibus_register_gospel: invalid json");
        return -1;
    };

    let event: OmniEvent = match serde_json::from_str(json_str) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_omnibus_register_gospel: parse error: {e}"));
            return -1;
        }
    };

    let Some(registry) = omni.gospel_registry() else {
        set_last_error("divi_omnibus_register_gospel: no gospel registry");
        return -1;
    };

    use globe::gospel::InsertResult;
    match registry.insert(&event) {
        InsertResult::Inserted | InsertResult::Duplicate => 0,
        InsertResult::Rejected => {
            set_last_error("divi_omnibus_register_gospel: event rejected");
            -1
        }
    }
}

// ===================================================================
// Config Enhancement
// ===================================================================

/// JSON-serializable proxy for OmnibusConfig.
///
/// OmnibusConfig contains non-Serialize types (`Option<ServerConfig>`),
/// so we use this simpler struct for the FFI JSON boundary.
#[derive(serde::Serialize, serde::Deserialize)]
struct OmnibusConfigJson {
    /// Directory for persistent storage, or null for in-memory.
    #[serde(default)]
    data_dir: Option<String>,
    /// Human-readable device name.
    #[serde(default = "default_device_name")]
    device_name: String,
    /// Port for the local relay server. 0 = OS-assigned.
    #[serde(default)]
    port: u16,
    /// Whether to bind to all interfaces.
    #[serde(default = "default_bind_all")]
    bind_all: bool,
    /// Optional home node URL.
    #[serde(default)]
    home_node: Option<String>,
    /// Maximum log capture capacity.
    #[serde(default = "default_log_capacity")]
    log_capture_capacity: usize,
}

fn default_device_name() -> String {
    "Omnidea Device".into()
}

fn default_bind_all() -> bool {
    true
}

fn default_log_capacity() -> usize {
    1000
}

impl OmnibusConfigJson {
    /// Convert to an OmnibusConfig, parsing URLs and paths.
    fn into_config(self) -> Result<OmnibusConfig, String> {
        let home_node = match self.home_node {
            Some(ref url_str) => {
                let parsed: url::Url = url_str
                    .parse()
                    .map_err(|e| format!("invalid home_node URL: {e}"))?;
                Some(parsed)
            }
            None => None,
        };

        Ok(OmnibusConfig {
            data_dir: self.data_dir.map(PathBuf::from),
            device_name: self.device_name,
            port: self.port,
            bind_all: self.bind_all,
            home_node,
            server_config: None,
            log_capture_capacity: self.log_capture_capacity,
            privacy: omnibus::PrivacyConfig::default(),
            enable_upnp: false,
        })
    }

    /// Build from an existing OmnibusConfig.
    fn from_config(config: &OmnibusConfig) -> Self {
        Self {
            data_dir: config.data_dir.as_ref().map(|p| p.display().to_string()),
            device_name: config.device_name.clone(),
            port: config.port,
            bind_all: config.bind_all,
            home_node: config.home_node.as_ref().map(|u| u.to_string()),
            log_capture_capacity: config.log_capture_capacity,
        }
    }
}

/// Start an Omnibus instance with a full JSON configuration.
///
/// `config_json` is a JSON object with fields:
/// - `data_dir` (string | null) — persistent storage directory
/// - `device_name` (string) — human-readable device name
/// - `port` (u16) — relay server port, 0 = OS-assigned
/// - `bind_all` (bool) — listen on all interfaces
/// - `home_node` (string | null) — home node WebSocket URL
/// - `log_capture_capacity` (usize) — max log entries in ring buffer
///
/// Returns an Omnibus pointer. Free with `divi_omnibus_free`.
///
/// # Safety
/// `config_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_start_with_config(
    config_json: *const c_char,
) -> *mut Omnibus {
    clear_last_error();
    let Some(json_str) = c_str_to_str(config_json) else {
        set_last_error("divi_omnibus_start_with_config: invalid json");
        return std::ptr::null_mut();
    };

    let proxy: OmnibusConfigJson = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_omnibus_start_with_config: parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let config = match proxy.into_config() {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_omnibus_start_with_config: {e}"));
            return std::ptr::null_mut();
        }
    };

    match Omnibus::start(config) {
        Ok(omni) => Box::into_raw(Box::new(omni)),
        Err(e) => {
            set_last_error(format!("omnibus start failed: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Get the current Omnibus configuration as JSON.
/// Free via `divi_free_string`.
///
/// # Safety
/// `omni` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_omnibus_config_json(omni: *const Omnibus) -> *mut c_char {
    let omni = unsafe { &*omni };
    let proxy = OmnibusConfigJson::from_config(omni.config());
    json_to_c(&proxy)
}

