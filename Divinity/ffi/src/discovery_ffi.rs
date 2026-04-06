use std::ffi::c_char;

use globe::discovery::local::{LocalAdvertiser, LocalBrowser, LocalPeer};

use crate::helpers::{c_str_to_str, json_to_c};
use crate::{clear_last_error, set_last_error};

/// Opaque wrapper for the mDNS advertiser.
pub struct GlobeAdvertiser(#[allow(dead_code)] LocalAdvertiser);

/// Opaque wrapper for the mDNS browser.
pub struct GlobeBrowser(LocalBrowser);

// ===================================================================
// Advertiser
// ===================================================================

/// Start advertising this device's relay on the local network.
///
/// `instance_name` is a human-readable name (e.g., "Sam's Mac").
/// `port` is the relay server port.
/// `pubkey_hex` is optional — the device's public key (null if not known yet).
///
/// Returns an advertiser pointer. Free with `divi_discovery_advertiser_free`.
///
/// # Safety
/// `instance_name` must be a valid C string. `pubkey_hex` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_discovery_advertise(
    instance_name: *const c_char,
    port: u16,
    pubkey_hex: *const c_char,
) -> *mut GlobeAdvertiser {
    clear_last_error();
    let Some(name) = c_str_to_str(instance_name) else {
        set_last_error("divi_discovery_advertise: invalid instance_name");
        return std::ptr::null_mut();
    };

    let pk = c_str_to_str(pubkey_hex);

    match LocalAdvertiser::start(name, port, pk) {
        Ok(adv) => Box::into_raw(Box::new(GlobeAdvertiser(adv))),
        Err(e) => {
            set_last_error(format!("mDNS advertise failed: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Stop advertising and free the advertiser.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_discovery_advertise`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_discovery_advertiser_free(ptr: *mut GlobeAdvertiser) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

// ===================================================================
// Browser
// ===================================================================

/// Start browsing for Omnidea relays on the local network.
///
/// Returns a browser pointer. Free with `divi_discovery_browser_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_discovery_browse() -> *mut GlobeBrowser {
    clear_last_error();
    match LocalBrowser::start() {
        Ok(browser) => Box::into_raw(Box::new(GlobeBrowser(browser))),
        Err(e) => {
            set_last_error(format!("mDNS browse failed: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Get all currently discovered peers as a JSON array.
///
/// Returns JSON like: `[{"name":"Sam's Mac","addresses":["192.168.1.5"],"port":8080,"pubkey_hex":null}]`
/// Free via `divi_free_string`.
///
/// # Safety
/// `browser` must be a valid pointer from `divi_discovery_browse`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_discovery_peers(browser: *const GlobeBrowser) -> *mut c_char {
    let browser = unsafe { &*browser };
    let peers = browser.0.peers();
    let serializable: Vec<PeerJson> = peers.into_iter().map(PeerJson::from).collect();
    json_to_c(&serializable)
}

/// Get the number of currently discovered peers.
///
/// # Safety
/// `browser` must be a valid pointer from `divi_discovery_browse`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_discovery_peer_count(browser: *const GlobeBrowser) -> u32 {
    let browser = unsafe { &*browser };
    browser.0.peers().len() as u32
}

/// Stop browsing and free the browser.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_discovery_browse`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_discovery_browser_free(ptr: *mut GlobeBrowser) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

// ===================================================================
// Serializable peer type
// ===================================================================

#[derive(serde::Serialize)]
struct PeerJson {
    name: String,
    addresses: Vec<String>,
    port: u16,
    pubkey_hex: Option<String>,
    ws_url: Option<String>,
}

impl From<LocalPeer> for PeerJson {
    fn from(peer: LocalPeer) -> Self {
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
