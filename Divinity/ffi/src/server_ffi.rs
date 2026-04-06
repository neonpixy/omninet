use std::ffi::c_char;
use std::net::SocketAddr;

use globe::event::OmniEvent;
use globe::server::listener::{RelayServer, ServerConfig};
use globe::server::storage::EventStore;

use crate::helpers::{c_str_to_str, string_to_c};
use crate::runtime_ffi::DiviRuntime;
use crate::{clear_last_error, set_last_error};

/// FFI wrapper for Globe's RelayServer.
pub struct GlobeServer {
    _server: RelayServer,
    addr: SocketAddr,
    store: EventStore,
}

/// Start a relay server.
///
/// `runtime` must be a valid DiviRuntime pointer.
/// `port` is the TCP port to bind. Use 0 for OS-assigned.
/// `bind_all` — if true, binds to 0.0.0.0 (reachable from LAN). If false, 127.0.0.1 only.
/// `config_json` is optional server configuration (null for defaults).
///
/// Returns a server pointer. Free with `divi_globe_server_free`.
///
/// # Safety
/// `runtime` must be a valid pointer from `divi_runtime_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_server_start(
    runtime: *const DiviRuntime,
    port: u16,
    bind_all: bool,
    config_json: *const c_char,
) -> *mut GlobeServer {
    clear_last_error();
    let runtime = unsafe { &*runtime };

    // ServerConfig is not Deserialize — use defaults.
    // Future: add individual setters if needed.
    let _ = config_json; // reserved for future use
    let config = ServerConfig::default();

    let host = if bind_all { "0.0.0.0" } else { "127.0.0.1" };
    let addr: SocketAddr = match format!("{host}:{port}").parse() {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("invalid address: {e}"));
            return std::ptr::null_mut();
        }
    };

    let result = runtime.runtime.block_on(RelayServer::start_at(addr, config));

    match result {
        Ok((server, actual_addr)) => {
            let store = server.store().clone();
            Box::into_raw(Box::new(GlobeServer {
                _server: server,
                addr: actual_addr,
                store,
            }))
        }
        Err(e) => {
            set_last_error(format!("server start failed: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Get the port the server is listening on.
///
/// # Safety
/// `server` must be a valid pointer from `divi_globe_server_start`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_server_port(server: *const GlobeServer) -> u16 {
    let server = unsafe { &*server };
    server.addr.port()
}

/// Get the number of active connections.
///
/// # Safety
/// `server` must be a valid pointer from `divi_globe_server_start`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_server_connections(server: *const GlobeServer) -> u32 {
    let server = unsafe { &*server };
    server._server.active_connections() as u32
}

/// Inject an event directly into the server's event store.
///
/// Bypasses WebSocket — useful for seeding test data or bootstrapping.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `server` must be a valid pointer. `event_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_server_seed_event(
    server: *const GlobeServer,
    event_json: *const c_char,
) -> i32 {
    clear_last_error();
    let server = unsafe { &*server };
    let Some(json_str) = c_str_to_str(event_json) else {
        set_last_error("divi_globe_server_seed_event: invalid json");
        return -1;
    };

    let event: OmniEvent = match serde_json::from_str(json_str) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_globe_server_seed_event: JSON parse error: {e}"));
            return -1;
        }
    };

    server.store.insert(event);
    0
}

/// Get the server's WebSocket URL (e.g., "ws://0.0.0.0:8080").
///
/// Returns a C string. Free via `divi_free_string`.
///
/// # Safety
/// `server` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_server_url(server: *const GlobeServer) -> *mut c_char {
    let server = unsafe { &*server };
    string_to_c(format!("ws://{}", server.addr))
}

/// Free a relay server.
///
/// # Safety
/// `server` must be a valid pointer from `divi_globe_server_start`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_globe_server_free(ptr: *mut GlobeServer) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}
