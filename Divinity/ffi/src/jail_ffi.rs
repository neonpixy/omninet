use std::ffi::c_char;
use std::sync::Mutex;

use jail::{
    AccountabilityFlag, FlagCategory, FlagSeverity,
    GraduatedResponse, JailConfig, TrustGraph, VerificationEdge,
    check_admission,
};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover};
use crate::{clear_last_error, set_last_error};

// ---------------------------------------------------------------------------
// TrustGraph — stateful opaque pointer (adjacency lists are expensive to serialize)
// ---------------------------------------------------------------------------

pub struct JailTrustGraph(pub(crate) Mutex<TrustGraph>);

/// Create a new empty trust graph.
/// Free with `divi_jail_trust_graph_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_jail_trust_graph_new() -> *mut JailTrustGraph {
    Box::into_raw(Box::new(JailTrustGraph(Mutex::new(TrustGraph::new()))))
}

/// Free a trust graph.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_jail_trust_graph_new`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_jail_trust_graph_free(ptr: *mut JailTrustGraph) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Add a verification edge to the trust graph.
///
/// `edge_json` is a JSON VerificationEdge.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `graph` must be a valid pointer. `edge_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_jail_trust_graph_add_edge(
    graph: *const JailTrustGraph,
    edge_json: *const c_char,
) -> i32 {
    clear_last_error();

    let graph = unsafe { &*graph };
    let Some(ej) = c_str_to_str(edge_json) else {
        set_last_error("divi_jail_trust_graph_add_edge: invalid edge_json");
        return -1;
    };

    let edge: VerificationEdge = match serde_json::from_str(ej) {
        Ok(e) => e,
        Err(e) => {
            set_last_error(format!("divi_jail_trust_graph_add_edge: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&graph.0);
    match guard.add_edge(edge) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Remove a verification edge by ID (UUID string).
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `graph` must be a valid pointer. `edge_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_jail_trust_graph_remove_edge(
    graph: *const JailTrustGraph,
    edge_id: *const c_char,
) -> i32 {
    clear_last_error();

    let graph = unsafe { &*graph };
    let Some(id_str) = c_str_to_str(edge_id) else {
        set_last_error("divi_jail_trust_graph_remove_edge: invalid edge_id");
        return -1;
    };

    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_jail_trust_graph_remove_edge: invalid UUID: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&graph.0);
    match guard.remove_edge(&uuid) {
        Ok(_) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Query network intelligence about a target person.
///
/// `flags_json` is a JSON array of AccountabilityFlags.
/// `config_json` is a JSON JailConfig (or null for defaults).
/// Returns JSON (NetworkIntelligence). Caller must free via `divi_free_string`.
///
/// # Safety
/// `graph` must be a valid pointer. All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_jail_trust_graph_query_intelligence(
    graph: *const JailTrustGraph,
    querier: *const c_char,
    target: *const c_char,
    flags_json: *const c_char,
    config_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let graph = unsafe { &*graph };

    let Some(q) = c_str_to_str(querier) else {
        set_last_error("divi_jail_trust_graph_query_intelligence: invalid querier");
        return std::ptr::null_mut();
    };

    let Some(t) = c_str_to_str(target) else {
        set_last_error("divi_jail_trust_graph_query_intelligence: invalid target");
        return std::ptr::null_mut();
    };

    let Some(fj) = c_str_to_str(flags_json) else {
        set_last_error("divi_jail_trust_graph_query_intelligence: invalid flags_json");
        return std::ptr::null_mut();
    };

    let flags: Vec<AccountabilityFlag> = match serde_json::from_str(fj) {
        Ok(f) => f,
        Err(e) => {
            set_last_error(format!("divi_jail_trust_graph_query_intelligence: {e}"));
            return std::ptr::null_mut();
        }
    };

    let config = if config_json.is_null() {
        JailConfig::default()
    } else if let Some(cj) = c_str_to_str(config_json) {
        serde_json::from_str(cj).unwrap_or_default()
    } else {
        JailConfig::default()
    };

    let guard = lock_or_recover(&graph.0);
    let intel = jail::trust_graph::query_intelligence(&guard, q, t, &flags, &config);
    json_to_c(&intel)
}

/// Get the node count in the trust graph.
///
/// # Safety
/// `graph` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_jail_trust_graph_node_count(
    graph: *const JailTrustGraph,
) -> u32 {
    let graph = unsafe { &*graph };
    let guard = lock_or_recover(&graph.0);
    guard.node_count() as u32
}

/// Get the edge count in the trust graph.
///
/// # Safety
/// `graph` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_jail_trust_graph_edge_count(
    graph: *const JailTrustGraph,
) -> u32 {
    let graph = unsafe { &*graph };
    let guard = lock_or_recover(&graph.0);
    guard.edge_count() as u32
}

// ===================================================================
// Accountability Flags — JSON in/out
// ===================================================================

/// Raise an accountability flag.
///
/// `category_json` is a JSON FlagCategory. `severity_json` is a JSON FlagSeverity.
/// Returns JSON (AccountabilityFlag). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_jail_flag_raise(
    flagger: *const c_char,
    flagged: *const c_char,
    category_json: *const c_char,
    severity_json: *const c_char,
    description: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(fr) = c_str_to_str(flagger) else {
        set_last_error("divi_jail_flag_raise: invalid flagger");
        return std::ptr::null_mut();
    };

    let Some(fd) = c_str_to_str(flagged) else {
        set_last_error("divi_jail_flag_raise: invalid flagged");
        return std::ptr::null_mut();
    };

    let Some(cj) = c_str_to_str(category_json) else {
        set_last_error("divi_jail_flag_raise: invalid category_json");
        return std::ptr::null_mut();
    };

    let Some(sj) = c_str_to_str(severity_json) else {
        set_last_error("divi_jail_flag_raise: invalid severity_json");
        return std::ptr::null_mut();
    };

    let Some(desc) = c_str_to_str(description) else {
        set_last_error("divi_jail_flag_raise: invalid description");
        return std::ptr::null_mut();
    };

    let category: FlagCategory = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_jail_flag_raise: {e}"));
            return std::ptr::null_mut();
        }
    };

    let severity: FlagSeverity = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_jail_flag_raise: {e}"));
            return std::ptr::null_mut();
        }
    };

    let flag = AccountabilityFlag::raise(fr, fd, category, severity, desc);
    json_to_c(&flag)
}

/// Check admission for a prospect to a community.
///
/// `members_json` is a JSON array of pubkey strings.
/// `flags_json` is a JSON array of AccountabilityFlags.
/// Returns JSON (AdmissionRecommendation). Caller must free via `divi_free_string`.
///
/// # Safety
/// `graph` must be a valid pointer. All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_jail_check_admission(
    graph: *const JailTrustGraph,
    prospect: *const c_char,
    community_id: *const c_char,
    members_json: *const c_char,
    flags_json: *const c_char,
    config_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let graph = unsafe { &*graph };

    let Some(pr) = c_str_to_str(prospect) else {
        set_last_error("divi_jail_check_admission: invalid prospect");
        return std::ptr::null_mut();
    };

    let Some(cid) = c_str_to_str(community_id) else {
        set_last_error("divi_jail_check_admission: invalid community_id");
        return std::ptr::null_mut();
    };

    let Some(mj) = c_str_to_str(members_json) else {
        set_last_error("divi_jail_check_admission: invalid members_json");
        return std::ptr::null_mut();
    };

    let Some(fj) = c_str_to_str(flags_json) else {
        set_last_error("divi_jail_check_admission: invalid flags_json");
        return std::ptr::null_mut();
    };

    let members: Vec<String> = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_jail_check_admission: {e}"));
            return std::ptr::null_mut();
        }
    };

    let flags: Vec<AccountabilityFlag> = match serde_json::from_str(fj) {
        Ok(f) => f,
        Err(e) => {
            set_last_error(format!("divi_jail_check_admission: {e}"));
            return std::ptr::null_mut();
        }
    };

    let config = if config_json.is_null() {
        JailConfig::default()
    } else if let Some(cj) = c_str_to_str(config_json) {
        serde_json::from_str(cj).unwrap_or_default()
    } else {
        JailConfig::default()
    };

    let guard = lock_or_recover(&graph.0);
    let rec = check_admission(&guard, pr, cid, &members, &flags, &config);
    json_to_c(&rec)
}

// ===================================================================
// Graduated Response — JSON round-trip
// ===================================================================

/// Begin a graduated response against a target.
///
/// Returns JSON (GraduatedResponse). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_jail_response_begin(
    target_pubkey: *const c_char,
    reason: *const c_char,
    initiated_by: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(target) = c_str_to_str(target_pubkey) else {
        set_last_error("divi_jail_response_begin: invalid target_pubkey");
        return std::ptr::null_mut();
    };

    let Some(rsn) = c_str_to_str(reason) else {
        set_last_error("divi_jail_response_begin: invalid reason");
        return std::ptr::null_mut();
    };

    let Some(by) = c_str_to_str(initiated_by) else {
        set_last_error("divi_jail_response_begin: invalid initiated_by");
        return std::ptr::null_mut();
    };

    let response = GraduatedResponse::begin(target, rsn, by);
    json_to_c(&response)
}

/// Escalate a graduated response.
///
/// Takes response JSON, returns modified response JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_jail_response_escalate(
    response_json: *const c_char,
    reason: *const c_char,
    initiated_by: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(rj) = c_str_to_str(response_json) else {
        set_last_error("divi_jail_response_escalate: invalid response_json");
        return std::ptr::null_mut();
    };

    let Some(rsn) = c_str_to_str(reason) else {
        set_last_error("divi_jail_response_escalate: invalid reason");
        return std::ptr::null_mut();
    };

    let Some(by) = c_str_to_str(initiated_by) else {
        set_last_error("divi_jail_response_escalate: invalid initiated_by");
        return std::ptr::null_mut();
    };

    let mut response: GraduatedResponse = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_jail_response_escalate: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = response.escalate(rsn, by) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&response)
}

/// De-escalate a graduated response.
///
/// Takes response JSON, returns modified response JSON.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_jail_response_de_escalate(
    response_json: *const c_char,
    reason: *const c_char,
    initiated_by: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(rj) = c_str_to_str(response_json) else {
        set_last_error("divi_jail_response_de_escalate: invalid response_json");
        return std::ptr::null_mut();
    };

    let Some(rsn) = c_str_to_str(reason) else {
        set_last_error("divi_jail_response_de_escalate: invalid reason");
        return std::ptr::null_mut();
    };

    let Some(by) = c_str_to_str(initiated_by) else {
        set_last_error("divi_jail_response_de_escalate: invalid initiated_by");
        return std::ptr::null_mut();
    };

    let mut response: GraduatedResponse = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_jail_response_de_escalate: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = response.de_escalate(rsn, by) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&response)
}

/// Get the default JailConfig as JSON.
///
/// Returns JSON (JailConfig). Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_jail_config_default() -> *mut c_char {
    json_to_c(&JailConfig::default())
}
