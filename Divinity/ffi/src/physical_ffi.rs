//! FFI bindings for World/Physical.
//!
//! JSON round-trip pattern for all types. Lat/lon cross the boundary as
//! `f64` scalars and are assembled into `GeoCoordinate` on the Rust side.
//! `DateTime<Utc>` values cross as `i64` Unix timestamps.
//!
//! **Covenant note:** Presence types (`PresenceSignal`, `PresenceStatus`,
//! `ProximityLevel`, `PresenceAudience`, `PresenceConfig`) are intentionally
//! NOT exposed. They do not implement `Serialize` by design — presence is
//! never persisted, logged, or transmitted beyond the local device.

use std::ffi::c_char;

use physical::{
    Delivery, Handoff, HandoffPurpose, LanternConfig, LanternShare, LanternSos,
    OmniTagIdentity, Place, PlaceType, PlaceVisibility, ProximityProofRef, Region,
    RegionBoundary, RegionDeclaration, RegionType, Rendezvous, RendezvousPurpose,
    RsvpResponse,
};
use physical::lantern::{LanternAudience, LanternPurpose};
use x::GeoCoordinate;

use crate::helpers::{c_str_to_str, json_to_c};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// Helper: construct GeoCoordinate from raw f64 lat/lon
// ===================================================================

/// Build a `GeoCoordinate` from raw f64 lat/lon, setting the FFI error
/// and returning `None` if the values are out of range.
fn geo_from_f64(lat: f64, lon: f64, fn_name: &str) -> Option<GeoCoordinate> {
    match GeoCoordinate::new(lat, lon) {
        Ok(c) => Some(c),
        Err(e) => {
            set_last_error(format!("{fn_name}: invalid coordinates ({lat}, {lon}): {e}"));
            None
        }
    }
}

/// Deserialize a JSON `*const c_char` into `T`, setting the FFI error on failure.
fn deser<T: serde::de::DeserializeOwned>(ptr: *const c_char, fn_name: &str, param: &str) -> Option<T> {
    let Some(s) = c_str_to_str(ptr) else {
        set_last_error(format!("{fn_name}: invalid {param}"));
        return None;
    };
    match serde_json::from_str(s) {
        Ok(v) => Some(v),
        Err(e) => {
            set_last_error(format!("{fn_name}: {param} parse error: {e}"));
            None
        }
    }
}

// ===================================================================
// Place (5 functions)
// ===================================================================

/// Create a new Place with Private visibility.
///
/// `type_json` is a JSON `PlaceType` (e.g. `"Cafe"`, `{"Custom":"Treehouse"}`).
/// Returns JSON (Place). Caller must free via `divi_free_string`.
///
/// # Safety
/// `name`, `type_json`, and `owner` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_place_new(
    name: *const c_char,
    type_json: *const c_char,
    lat: f64,
    lon: f64,
    owner: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_place_new";

    let Some(name_str) = c_str_to_str(name) else {
        set_last_error(format!("{fn_name}: invalid name"));
        return std::ptr::null_mut();
    };
    let Some(place_type) = deser::<PlaceType>(type_json, fn_name, "type_json") else {
        return std::ptr::null_mut();
    };
    let Some(coord) = geo_from_f64(lat, lon, fn_name) else {
        return std::ptr::null_mut();
    };
    let Some(owner_str) = c_str_to_str(owner) else {
        set_last_error(format!("{fn_name}: invalid owner"));
        return std::ptr::null_mut();
    };

    let place = Place::new(name_str, place_type, coord, owner_str);
    json_to_c(&place)
}

/// Update a Place's location. Only the owner may do this.
///
/// Takes Place JSON, returns modified Place JSON.
///
/// # Safety
/// `place_json` and `updater` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_place_update_location(
    place_json: *const c_char,
    lat: f64,
    lon: f64,
    updater: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_place_update_location";

    let Some(mut place) = deser::<Place>(place_json, fn_name, "place_json") else {
        return std::ptr::null_mut();
    };
    let Some(coord) = geo_from_f64(lat, lon, fn_name) else {
        return std::ptr::null_mut();
    };
    let Some(updater_str) = c_str_to_str(updater) else {
        set_last_error(format!("{fn_name}: invalid updater"));
        return std::ptr::null_mut();
    };

    if let Err(e) = place.update_location(coord, updater_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&place)
}

/// Update a Place's name. Only the owner may do this.
///
/// Takes Place JSON, returns modified Place JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_place_update_name(
    place_json: *const c_char,
    name: *const c_char,
    updater: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_place_update_name";

    let Some(mut place) = deser::<Place>(place_json, fn_name, "place_json") else {
        return std::ptr::null_mut();
    };
    let Some(name_str) = c_str_to_str(name) else {
        set_last_error(format!("{fn_name}: invalid name"));
        return std::ptr::null_mut();
    };
    let Some(updater_str) = c_str_to_str(updater) else {
        set_last_error(format!("{fn_name}: invalid updater"));
        return std::ptr::null_mut();
    };

    if let Err(e) = place.update_name(name_str, updater_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&place)
}

/// Check whether a viewer can see a Place.
///
/// `memberships_json` is a JSON `Vec<String>` of community IDs the viewer belongs to.
/// Returns 1=visible, 0=not visible, -1=error.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_place_is_visible_to(
    place_json: *const c_char,
    viewer: *const c_char,
    memberships_json: *const c_char,
) -> i32 {
    clear_last_error();
    let fn_name = "divi_physical_place_is_visible_to";

    let Some(place) = deser::<Place>(place_json, fn_name, "place_json") else {
        return -1;
    };
    let Some(viewer_str) = c_str_to_str(viewer) else {
        set_last_error(format!("{fn_name}: invalid viewer"));
        return -1;
    };
    let Some(memberships) = deser::<Vec<String>>(memberships_json, fn_name, "memberships_json") else {
        return -1;
    };

    if place.is_visible_to(viewer_str, &memberships) { 1 } else { 0 }
}

/// Set a Place's visibility.
///
/// `visibility_json` is a JSON `PlaceVisibility` (e.g. `"Public"`, `"Private"`,
/// `{"Shared":["cpub1..."]}`).
/// Returns modified Place JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_place_set_visibility(
    place_json: *const c_char,
    visibility_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_place_set_visibility";

    let Some(mut place) = deser::<Place>(place_json, fn_name, "place_json") else {
        return std::ptr::null_mut();
    };
    let Some(vis) = deser::<PlaceVisibility>(visibility_json, fn_name, "visibility_json") else {
        return std::ptr::null_mut();
    };

    place.visibility = vis;
    json_to_c(&place)
}

// ===================================================================
// Region (2 functions)
// ===================================================================

/// Create a new Region.
///
/// `type_json` is a JSON `RegionType` (e.g. `"City"`, `{"Custom":"Watershed"}`).
/// `boundary_json` is a JSON `RegionBoundary`.
/// Returns JSON (Region). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_region_new(
    name: *const c_char,
    type_json: *const c_char,
    boundary_json: *const c_char,
    creator: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_region_new";

    let Some(name_str) = c_str_to_str(name) else {
        set_last_error(format!("{fn_name}: invalid name"));
        return std::ptr::null_mut();
    };
    let Some(region_type) = deser::<RegionType>(type_json, fn_name, "type_json") else {
        return std::ptr::null_mut();
    };
    let Some(boundary) = deser::<RegionBoundary>(boundary_json, fn_name, "boundary_json") else {
        return std::ptr::null_mut();
    };
    let Some(creator_str) = c_str_to_str(creator) else {
        set_last_error(format!("{fn_name}: invalid creator"));
        return std::ptr::null_mut();
    };

    let region = Region::new(name_str, region_type, boundary, creator_str);
    json_to_c(&region)
}

/// Check whether a point falls inside a Region's boundary.
///
/// Returns 1=inside, 0=outside, -1=error.
///
/// # Safety
/// `region_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_region_contains_point(
    region_json: *const c_char,
    lat: f64,
    lon: f64,
) -> i32 {
    clear_last_error();
    let fn_name = "divi_physical_region_contains_point";

    let Some(region) = deser::<Region>(region_json, fn_name, "region_json") else {
        return -1;
    };
    let Some(coord) = geo_from_f64(lat, lon, fn_name) else {
        return -1;
    };

    if region.contains_point(&coord) { 1 } else { 0 }
}

// ===================================================================
// RegionDeclaration (3 functions)
// ===================================================================

/// Create a new RegionDeclaration.
///
/// `region_ids_json` is a JSON `Vec<Uuid>` (array of UUID strings).
/// Returns JSON (RegionDeclaration). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_declaration_new(
    person: *const c_char,
    region_ids_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_declaration_new";

    let Some(person_str) = c_str_to_str(person) else {
        set_last_error(format!("{fn_name}: invalid person"));
        return std::ptr::null_mut();
    };
    let Some(ids) = deser::<Vec<uuid::Uuid>>(region_ids_json, fn_name, "region_ids_json") else {
        return std::ptr::null_mut();
    };

    let decl = RegionDeclaration::new(person_str, ids);
    json_to_c(&decl)
}

/// Add a region to a RegionDeclaration (idempotent).
///
/// `id` is a UUID string. Returns modified declaration JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_declaration_add_region(
    decl_json: *const c_char,
    id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_declaration_add_region";

    let Some(mut decl) = deser::<RegionDeclaration>(decl_json, fn_name, "decl_json") else {
        return std::ptr::null_mut();
    };
    let Some(id_str) = c_str_to_str(id) else {
        set_last_error(format!("{fn_name}: invalid id"));
        return std::ptr::null_mut();
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("{fn_name}: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    decl.add_region(uuid);
    json_to_c(&decl)
}

/// Remove a region from a RegionDeclaration.
///
/// `id` is a UUID string. Returns modified declaration JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_declaration_remove_region(
    decl_json: *const c_char,
    id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_declaration_remove_region";

    let Some(mut decl) = deser::<RegionDeclaration>(decl_json, fn_name, "decl_json") else {
        return std::ptr::null_mut();
    };
    let Some(id_str) = c_str_to_str(id) else {
        set_last_error(format!("{fn_name}: invalid id"));
        return std::ptr::null_mut();
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("{fn_name}: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    decl.remove_region(uuid);
    json_to_c(&decl)
}

// ===================================================================
// Rendezvous (7 functions)
// ===================================================================

/// Create a new Rendezvous in Proposed status.
///
/// `scheduled_at` is a Unix timestamp in seconds. `purpose_json` is a JSON
/// `RendezvousPurpose` (e.g. `"Social"`, `{"Custom":"Hackathon"}`).
/// Returns JSON (Rendezvous). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_rendezvous_new(
    title: *const c_char,
    organizer: *const c_char,
    scheduled_at: i64,
    purpose_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_rendezvous_new";

    let Some(title_str) = c_str_to_str(title) else {
        set_last_error(format!("{fn_name}: invalid title"));
        return std::ptr::null_mut();
    };
    let Some(organizer_str) = c_str_to_str(organizer) else {
        set_last_error(format!("{fn_name}: invalid organizer"));
        return std::ptr::null_mut();
    };
    let Some(purpose) = deser::<RendezvousPurpose>(purpose_json, fn_name, "purpose_json") else {
        return std::ptr::null_mut();
    };
    let time = chrono::DateTime::from_timestamp(scheduled_at, 0)
        .unwrap_or_else(chrono::Utc::now);

    let rv = Rendezvous::new(title_str, organizer_str, time, purpose);
    json_to_c(&rv)
}

/// Invite a participant to a Rendezvous (idempotent).
///
/// Returns modified Rendezvous JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_rendezvous_invite(
    rv_json: *const c_char,
    crown_id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_rendezvous_invite";

    let Some(mut rv) = deser::<Rendezvous>(rv_json, fn_name, "rv_json") else {
        return std::ptr::null_mut();
    };
    let Some(crown_id_str) = c_str_to_str(crown_id) else {
        set_last_error(format!("{fn_name}: invalid crown_id"));
        return std::ptr::null_mut();
    };

    rv.invite(crown_id_str);
    json_to_c(&rv)
}

/// Record or update an RSVP for a Rendezvous.
///
/// `response_json` is a JSON `RsvpResponse` (e.g. `"Attending"`, `"Maybe"`, `"Declined"`).
/// `message` may be null. Returns modified Rendezvous JSON.
///
/// # Safety
/// All C strings must be valid. `message` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_rendezvous_rsvp(
    rv_json: *const c_char,
    person: *const c_char,
    response_json: *const c_char,
    message: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_rendezvous_rsvp";

    let Some(mut rv) = deser::<Rendezvous>(rv_json, fn_name, "rv_json") else {
        return std::ptr::null_mut();
    };
    let Some(person_str) = c_str_to_str(person) else {
        set_last_error(format!("{fn_name}: invalid person"));
        return std::ptr::null_mut();
    };
    let Some(response) = deser::<RsvpResponse>(response_json, fn_name, "response_json") else {
        return std::ptr::null_mut();
    };
    let msg = c_str_to_str(message).map(String::from);

    rv.rsvp(person_str, response, msg);
    json_to_c(&rv)
}

/// Confirm a Rendezvous (Proposed -> Confirmed). Only the organizer.
///
/// Returns modified Rendezvous JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_rendezvous_confirm(
    rv_json: *const c_char,
    updater: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_rendezvous_confirm";

    let Some(mut rv) = deser::<Rendezvous>(rv_json, fn_name, "rv_json") else {
        return std::ptr::null_mut();
    };
    let Some(updater_str) = c_str_to_str(updater) else {
        set_last_error(format!("{fn_name}: invalid updater"));
        return std::ptr::null_mut();
    };

    if let Err(e) = rv.confirm(updater_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&rv)
}

/// Cancel a Rendezvous. Only the organizer.
///
/// `reason` may be null. Returns modified Rendezvous JSON.
///
/// # Safety
/// All C strings must be valid. `reason` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_rendezvous_cancel(
    rv_json: *const c_char,
    reason: *const c_char,
    canceller: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_rendezvous_cancel";

    let Some(mut rv) = deser::<Rendezvous>(rv_json, fn_name, "rv_json") else {
        return std::ptr::null_mut();
    };
    let reason_opt = c_str_to_str(reason).map(String::from);
    let Some(canceller_str) = c_str_to_str(canceller) else {
        set_last_error(format!("{fn_name}: invalid canceller"));
        return std::ptr::null_mut();
    };

    if let Err(e) = rv.cancel(reason_opt, canceller_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&rv)
}

/// Complete a Rendezvous. Only the organizer.
///
/// Returns modified Rendezvous JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_rendezvous_complete(
    rv_json: *const c_char,
    updater: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_rendezvous_complete";

    let Some(mut rv) = deser::<Rendezvous>(rv_json, fn_name, "rv_json") else {
        return std::ptr::null_mut();
    };
    let Some(updater_str) = c_str_to_str(updater) else {
        set_last_error(format!("{fn_name}: invalid updater"));
        return std::ptr::null_mut();
    };

    if let Err(e) = rv.complete(updater_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&rv)
}

/// Reschedule a Rendezvous. Only the organizer.
///
/// `new_time` is a Unix timestamp in seconds. Returns modified Rendezvous JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_rendezvous_reschedule(
    rv_json: *const c_char,
    new_time: i64,
    updater: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_rendezvous_reschedule";

    let Some(mut rv) = deser::<Rendezvous>(rv_json, fn_name, "rv_json") else {
        return std::ptr::null_mut();
    };
    let Some(updater_str) = c_str_to_str(updater) else {
        set_last_error(format!("{fn_name}: invalid updater"));
        return std::ptr::null_mut();
    };
    let time = chrono::DateTime::from_timestamp(new_time, 0)
        .unwrap_or_else(chrono::Utc::now);

    if let Err(e) = rv.reschedule(time, updater_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&rv)
}

// ===================================================================
// Lantern (4 functions)
// ===================================================================

/// Light a new Lantern beacon.
///
/// `audience_json` is a JSON `LanternAudience`. `purpose_json` is a JSON
/// `LanternPurpose`. `config_json` may be null (uses default LanternConfig).
/// Returns JSON (LanternShare). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid. `config_json` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_lantern_new(
    person: *const c_char,
    lat: f64,
    lon: f64,
    audience_json: *const c_char,
    purpose_json: *const c_char,
    config_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_lantern_new";

    let Some(person_str) = c_str_to_str(person) else {
        set_last_error(format!("{fn_name}: invalid person"));
        return std::ptr::null_mut();
    };
    let Some(coord) = geo_from_f64(lat, lon, fn_name) else {
        return std::ptr::null_mut();
    };
    let Some(audience) = deser::<LanternAudience>(audience_json, fn_name, "audience_json") else {
        return std::ptr::null_mut();
    };
    let Some(purpose) = deser::<LanternPurpose>(purpose_json, fn_name, "purpose_json") else {
        return std::ptr::null_mut();
    };

    let config = if config_json.is_null() || c_str_to_str(config_json).is_none() {
        LanternConfig::default()
    } else {
        match deser::<LanternConfig>(config_json, fn_name, "config_json") {
            Some(c) => c,
            None => return std::ptr::null_mut(),
        }
    };

    let lantern = LanternShare::new(person_str, coord, audience, purpose, &config);
    json_to_c(&lantern)
}

/// Update a Lantern's position.
///
/// Returns modified LanternShare JSON.
///
/// # Safety
/// `lantern_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_lantern_update_location(
    lantern_json: *const c_char,
    lat: f64,
    lon: f64,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_lantern_update_location";

    let Some(mut lantern) = deser::<LanternShare>(lantern_json, fn_name, "lantern_json") else {
        return std::ptr::null_mut();
    };
    let Some(coord) = geo_from_f64(lat, lon, fn_name) else {
        return std::ptr::null_mut();
    };

    lantern.update_location(coord);
    json_to_c(&lantern)
}

/// Extinguish a Lantern (set expiry to now).
///
/// Returns modified LanternShare JSON.
///
/// # Safety
/// `lantern_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_lantern_extinguish(
    lantern_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_lantern_extinguish";

    let Some(mut lantern) = deser::<LanternShare>(lantern_json, fn_name, "lantern_json") else {
        return std::ptr::null_mut();
    };

    lantern.extinguish();
    json_to_c(&lantern)
}

/// Extend a Lantern's lifetime by `seconds`.
///
/// `config_json` may be null (uses default LanternConfig for max TTL check).
/// Returns modified LanternShare JSON, or null on error (e.g. TTL exceeded).
///
/// # Safety
/// `lantern_json` must be a valid C string. `config_json` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_lantern_extend(
    lantern_json: *const c_char,
    seconds: u64,
    config_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_lantern_extend";

    let Some(mut lantern) = deser::<LanternShare>(lantern_json, fn_name, "lantern_json") else {
        return std::ptr::null_mut();
    };

    let config = if config_json.is_null() || c_str_to_str(config_json).is_none() {
        LanternConfig::default()
    } else {
        match deser::<LanternConfig>(config_json, fn_name, "config_json") {
            Some(c) => c,
            None => return std::ptr::null_mut(),
        }
    };

    if let Err(e) = lantern.extend(seconds, &config) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&lantern)
}

// ===================================================================
// LanternSos (3 functions)
// ===================================================================

/// Activate an SOS emergency beacon.
///
/// `contacts_json` is a JSON `Vec<String>` of emergency contact crown IDs.
/// Returns JSON (LanternSos). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_lantern_sos_activate(
    person: *const c_char,
    lat: f64,
    lon: f64,
    contacts_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_lantern_sos_activate";

    let Some(person_str) = c_str_to_str(person) else {
        set_last_error(format!("{fn_name}: invalid person"));
        return std::ptr::null_mut();
    };
    let Some(coord) = geo_from_f64(lat, lon, fn_name) else {
        return std::ptr::null_mut();
    };
    let Some(contacts) = deser::<Vec<String>>(contacts_json, fn_name, "contacts_json") else {
        return std::ptr::null_mut();
    };

    let sos = LanternSos::activate(person_str, coord, contacts);
    json_to_c(&sos)
}

/// Update an SOS beacon's position.
///
/// Returns modified LanternSos JSON.
///
/// # Safety
/// `sos_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_lantern_sos_update_location(
    sos_json: *const c_char,
    lat: f64,
    lon: f64,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_lantern_sos_update_location";

    let Some(mut sos) = deser::<LanternSos>(sos_json, fn_name, "sos_json") else {
        return std::ptr::null_mut();
    };
    let Some(coord) = geo_from_f64(lat, lon, fn_name) else {
        return std::ptr::null_mut();
    };

    sos.update_location(coord);
    json_to_c(&sos)
}

/// Resolve (deactivate) an SOS beacon.
///
/// Returns modified LanternSos JSON.
///
/// # Safety
/// `sos_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_lantern_sos_resolve(
    sos_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_lantern_sos_resolve";

    let Some(mut sos) = deser::<LanternSos>(sos_json, fn_name, "sos_json") else {
        return std::ptr::null_mut();
    };

    sos.resolve();
    json_to_c(&sos)
}

// ===================================================================
// Handoff (7 functions)
// ===================================================================

/// Create a new Handoff between two parties.
///
/// `purpose_json` is a JSON `HandoffPurpose` (e.g. `"CashExchange"`).
/// Fails if initiator == counterparty.
/// Returns JSON (Handoff). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_handoff_new(
    initiator: *const c_char,
    counterparty: *const c_char,
    purpose_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_handoff_new";

    let Some(init_str) = c_str_to_str(initiator) else {
        set_last_error(format!("{fn_name}: invalid initiator"));
        return std::ptr::null_mut();
    };
    let Some(cp_str) = c_str_to_str(counterparty) else {
        set_last_error(format!("{fn_name}: invalid counterparty"));
        return std::ptr::null_mut();
    };
    let Some(purpose) = deser::<HandoffPurpose>(purpose_json, fn_name, "purpose_json") else {
        return std::ptr::null_mut();
    };

    match Handoff::new(init_str, cp_str, purpose) {
        Ok(h) => json_to_c(&h),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Record proximity verification for a Handoff.
///
/// `proof_ref_json` is a JSON `ProximityProofRef`.
/// Returns modified Handoff JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_handoff_verify_proximity(
    handoff_json: *const c_char,
    proof_ref_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_handoff_verify_proximity";

    let Some(mut handoff) = deser::<Handoff>(handoff_json, fn_name, "handoff_json") else {
        return std::ptr::null_mut();
    };
    let Some(proof_ref) = deser::<ProximityProofRef>(proof_ref_json, fn_name, "proof_ref_json") else {
        return std::ptr::null_mut();
    };

    if let Err(e) = handoff.verify_proximity(proof_ref) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&handoff)
}

/// Record the initiator's signature on a Handoff.
///
/// Returns modified Handoff JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_handoff_sign_initiator(
    handoff_json: *const c_char,
    signature: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_handoff_sign_initiator";

    let Some(mut handoff) = deser::<Handoff>(handoff_json, fn_name, "handoff_json") else {
        return std::ptr::null_mut();
    };
    let Some(sig_str) = c_str_to_str(signature) else {
        set_last_error(format!("{fn_name}: invalid signature"));
        return std::ptr::null_mut();
    };

    if let Err(e) = handoff.sign_initiator(sig_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&handoff)
}

/// Record the counterparty's signature on a Handoff.
///
/// Returns modified Handoff JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_handoff_sign_counterparty(
    handoff_json: *const c_char,
    signature: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_handoff_sign_counterparty";

    let Some(mut handoff) = deser::<Handoff>(handoff_json, fn_name, "handoff_json") else {
        return std::ptr::null_mut();
    };
    let Some(sig_str) = c_str_to_str(signature) else {
        set_last_error(format!("{fn_name}: invalid signature"));
        return std::ptr::null_mut();
    };

    if let Err(e) = handoff.sign_counterparty(sig_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&handoff)
}

/// Complete a fully-signed Handoff.
///
/// Returns modified Handoff JSON.
///
/// # Safety
/// `handoff_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_handoff_complete(
    handoff_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_handoff_complete";

    let Some(mut handoff) = deser::<Handoff>(handoff_json, fn_name, "handoff_json") else {
        return std::ptr::null_mut();
    };

    if let Err(e) = handoff.complete() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&handoff)
}

/// Cancel a Handoff. Available from any non-Completed state.
///
/// Returns modified Handoff JSON.
///
/// # Safety
/// `handoff_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_handoff_cancel(
    handoff_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_handoff_cancel";

    let Some(mut handoff) = deser::<Handoff>(handoff_json, fn_name, "handoff_json") else {
        return std::ptr::null_mut();
    };

    if let Err(e) = handoff.cancel() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&handoff)
}

/// Raise a dispute on a completed Handoff.
///
/// Returns modified Handoff JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_handoff_dispute(
    handoff_json: *const c_char,
    reason: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_handoff_dispute";

    let Some(mut handoff) = deser::<Handoff>(handoff_json, fn_name, "handoff_json") else {
        return std::ptr::null_mut();
    };
    let Some(reason_str) = c_str_to_str(reason) else {
        set_last_error(format!("{fn_name}: invalid reason"));
        return std::ptr::null_mut();
    };

    if let Err(e) = handoff.dispute(reason_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&handoff)
}

// ===================================================================
// OmniTag (3 functions)
// ===================================================================

/// Create a new OmniTag identity (active by default).
///
/// Returns JSON (OmniTagIdentity). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_tag_new(
    owner: *const c_char,
    tag_pubkey: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_tag_new";

    let Some(owner_str) = c_str_to_str(owner) else {
        set_last_error(format!("{fn_name}: invalid owner"));
        return std::ptr::null_mut();
    };
    let Some(tag_pk) = c_str_to_str(tag_pubkey) else {
        set_last_error(format!("{fn_name}: invalid tag_pubkey"));
        return std::ptr::null_mut();
    };

    let tag = OmniTagIdentity::new(owner_str, tag_pk);
    json_to_c(&tag)
}

/// Deactivate an OmniTag.
///
/// Returns modified OmniTagIdentity JSON.
///
/// # Safety
/// `tag_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_tag_deactivate(
    tag_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_tag_deactivate";

    let Some(mut tag) = deser::<OmniTagIdentity>(tag_json, fn_name, "tag_json") else {
        return std::ptr::null_mut();
    };

    tag.deactivate();
    json_to_c(&tag)
}

/// Activate a previously deactivated OmniTag.
///
/// Returns modified OmniTagIdentity JSON.
///
/// # Safety
/// `tag_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_tag_activate(
    tag_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_tag_activate";

    let Some(mut tag) = deser::<OmniTagIdentity>(tag_json, fn_name, "tag_json") else {
        return std::ptr::null_mut();
    };

    tag.activate();
    json_to_c(&tag)
}

// ===================================================================
// Delivery (8 functions)
// ===================================================================

/// Create a new Delivery in Created status.
///
/// Returns JSON (Delivery). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_delivery_new(
    sender: *const c_char,
    recipient: *const c_char,
    description: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_delivery_new";

    let Some(sender_str) = c_str_to_str(sender) else {
        set_last_error(format!("{fn_name}: invalid sender"));
        return std::ptr::null_mut();
    };
    let Some(recipient_str) = c_str_to_str(recipient) else {
        set_last_error(format!("{fn_name}: invalid recipient"));
        return std::ptr::null_mut();
    };
    let Some(desc_str) = c_str_to_str(description) else {
        set_last_error(format!("{fn_name}: invalid description"));
        return std::ptr::null_mut();
    };

    let delivery = Delivery::new(sender_str, recipient_str, desc_str);
    json_to_c(&delivery)
}

/// Assign a courier to a Delivery. Only valid from Created status.
///
/// Returns modified Delivery JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_delivery_assign_courier(
    delivery_json: *const c_char,
    courier: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_delivery_assign_courier";

    let Some(mut delivery) = deser::<Delivery>(delivery_json, fn_name, "delivery_json") else {
        return std::ptr::null_mut();
    };
    let Some(courier_str) = c_str_to_str(courier) else {
        set_last_error(format!("{fn_name}: invalid courier"));
        return std::ptr::null_mut();
    };

    if let Err(e) = delivery.assign_courier(courier_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&delivery)
}

/// Mark a Delivery as picked up. Only valid from CourierAssigned.
///
/// `handoff_id` is a UUID string for the pickup Handoff.
/// Returns modified Delivery JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_delivery_mark_picked_up(
    delivery_json: *const c_char,
    handoff_id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_delivery_mark_picked_up";

    let Some(mut delivery) = deser::<Delivery>(delivery_json, fn_name, "delivery_json") else {
        return std::ptr::null_mut();
    };
    let Some(id_str) = c_str_to_str(handoff_id) else {
        set_last_error(format!("{fn_name}: invalid handoff_id"));
        return std::ptr::null_mut();
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("{fn_name}: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = delivery.mark_picked_up(uuid) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&delivery)
}

/// Mark a Delivery as in transit. Only valid from PickedUp.
///
/// Returns modified Delivery JSON.
///
/// # Safety
/// `delivery_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_delivery_mark_in_transit(
    delivery_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_delivery_mark_in_transit";

    let Some(mut delivery) = deser::<Delivery>(delivery_json, fn_name, "delivery_json") else {
        return std::ptr::null_mut();
    };

    if let Err(e) = delivery.mark_in_transit() {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&delivery)
}

/// Mark a Delivery as delivered. Valid from InTransit or NearDestination.
///
/// `handoff_id` is a UUID string for the delivery Handoff.
/// Returns modified Delivery JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_delivery_mark_delivered(
    delivery_json: *const c_char,
    handoff_id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_delivery_mark_delivered";

    let Some(mut delivery) = deser::<Delivery>(delivery_json, fn_name, "delivery_json") else {
        return std::ptr::null_mut();
    };
    let Some(id_str) = c_str_to_str(handoff_id) else {
        set_last_error(format!("{fn_name}: invalid handoff_id"));
        return std::ptr::null_mut();
    };
    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("{fn_name}: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = delivery.mark_delivered(uuid) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&delivery)
}

/// Confirm a Delivery. Only the recipient, only from Delivered status.
///
/// Returns modified Delivery JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_delivery_confirm(
    delivery_json: *const c_char,
    confirmer: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_delivery_confirm";

    let Some(mut delivery) = deser::<Delivery>(delivery_json, fn_name, "delivery_json") else {
        return std::ptr::null_mut();
    };
    let Some(confirmer_str) = c_str_to_str(confirmer) else {
        set_last_error(format!("{fn_name}: invalid confirmer"));
        return std::ptr::null_mut();
    };

    if let Err(e) = delivery.confirm(confirmer_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&delivery)
}

/// Dispute a Delivery. Valid from Delivered or Confirmed.
///
/// Returns modified Delivery JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_delivery_dispute(
    delivery_json: *const c_char,
    reason: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_delivery_dispute";

    let Some(mut delivery) = deser::<Delivery>(delivery_json, fn_name, "delivery_json") else {
        return std::ptr::null_mut();
    };
    let Some(reason_str) = c_str_to_str(reason) else {
        set_last_error(format!("{fn_name}: invalid reason"));
        return std::ptr::null_mut();
    };

    if let Err(e) = delivery.dispute(reason_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&delivery)
}

/// Cancel a Delivery. Valid from any state except Confirmed.
///
/// Returns modified Delivery JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_physical_delivery_cancel(
    delivery_json: *const c_char,
    canceller: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_physical_delivery_cancel";

    let Some(mut delivery) = deser::<Delivery>(delivery_json, fn_name, "delivery_json") else {
        return std::ptr::null_mut();
    };
    let Some(canceller_str) = c_str_to_str(canceller) else {
        set_last_error(format!("{fn_name}: invalid canceller"));
        return std::ptr::null_mut();
    };

    if let Err(e) = delivery.cancel(canceller_str) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&delivery)
}
