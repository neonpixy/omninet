use std::ffi::c_char;
use std::sync::Mutex;

use undercroft::{
    CommunityHealth, EconomicHealth, HealthHistory, HealthMetrics, HealthSnapshot, NetworkHealth,
    QuestHealth,
};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// NetworkHealth — stateless JSON
// ===================================================================

/// Deserialize and re-serialize a NetworkHealth from JSON.
///
/// `network_health_json` is a JSON `NetworkHealth` (pre-computed from
/// `NetworkHealth::from_relay_health` on the Rust side).
///
/// Note: Globe's `RelayHealth` is not serializable (it contains private
/// fields and `Url`/`Duration` types). The aggregation from relay data
/// to `NetworkHealth` must happen in Rust before crossing the FFI
/// boundary. This function validates and round-trips the data.
///
/// Returns JSON (NetworkHealth). Caller must free via `divi_free_string`.
///
/// # Safety
/// `network_health_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_undercroft_network_health(
    network_health_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(nj) = c_str_to_str(network_health_json) else {
        set_last_error("divi_undercroft_network_health: invalid network_health_json");
        return std::ptr::null_mut();
    };

    let health: NetworkHealth = match serde_json::from_str(nj) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(format!("divi_undercroft_network_health: {e}"));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&health)
}

/// Create an empty NetworkHealth snapshot.
///
/// Returns JSON (NetworkHealth). Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_undercroft_network_health_empty() -> *mut c_char {
    json_to_c(&NetworkHealth::empty())
}

/// Aggregate community health from Kingdom and Bulwark data.
///
/// - `community_json` is a JSON `kingdom::Community`.
/// - `proposals_json` is a JSON array of `kingdom::Proposal`.
/// - `pulse_json` is a nullable JSON `bulwark::CollectiveHealthPulse`.
///
/// Returns JSON (CommunityHealth). Caller must free via `divi_free_string`.
///
/// # Safety
/// `community_json` and `proposals_json` must be valid C strings.
/// `pulse_json` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_undercroft_community_health(
    community_json: *const c_char,
    proposals_json: *const c_char,
    pulse_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(community_json) else {
        set_last_error("divi_undercroft_community_health: invalid community_json");
        return std::ptr::null_mut();
    };
    let Some(pj) = c_str_to_str(proposals_json) else {
        set_last_error("divi_undercroft_community_health: invalid proposals_json");
        return std::ptr::null_mut();
    };

    let community: kingdom::Community = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_undercroft_community_health: community: {e}"));
            return std::ptr::null_mut();
        }
    };

    let proposals: Vec<kingdom::Proposal> = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_undercroft_community_health: proposals: {e}"));
            return std::ptr::null_mut();
        }
    };

    let pulse: Option<bulwark::CollectiveHealthPulse> = if pulse_json.is_null() {
        None
    } else if let Some(pj_str) = c_str_to_str(pulse_json) {
        match serde_json::from_str(pj_str) {
            Ok(p) => Some(p),
            Err(e) => {
                set_last_error(format!("divi_undercroft_community_health: pulse: {e}"));
                return std::ptr::null_mut();
            }
        }
    } else {
        None
    };

    let health =
        CommunityHealth::from_community(&community, &proposals, pulse.as_ref());
    json_to_c(&health)
}

/// Aggregate economic health from a Fortune TreasuryStatus.
///
/// `treasury_status_json` is a JSON `fortune::TreasuryStatus`.
/// Returns JSON (EconomicHealth). Caller must free via `divi_free_string`.
///
/// # Safety
/// `treasury_status_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_undercroft_economic_health(
    treasury_status_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(treasury_status_json) else {
        set_last_error("divi_undercroft_economic_health: invalid treasury_status_json");
        return std::ptr::null_mut();
    };

    let status: fortune::TreasuryStatus = match serde_json::from_str(tj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_undercroft_economic_health: {e}"));
            return std::ptr::null_mut();
        }
    };

    let health = EconomicHealth::from_treasury_status(&status);
    json_to_c(&health)
}

/// Create an empty EconomicHealth snapshot.
///
/// Returns JSON (EconomicHealth). Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_undercroft_economic_health_empty() -> *mut c_char {
    json_to_c(&EconomicHealth::empty())
}

/// Compute top-level health metrics from a snapshot.
///
/// - `snapshot_json` is a JSON `HealthSnapshot`.
/// - `node_count` is the estimated number of network nodes.
/// - `store_stats_json` is a nullable JSON `globe::StoreStats`.
///
/// Returns JSON (HealthMetrics). Caller must free via `divi_free_string`.
///
/// # Safety
/// `snapshot_json` must be a valid C string. `store_stats_json` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_undercroft_health_metrics(
    snapshot_json: *const c_char,
    node_count: u64,
    store_stats_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(sj) = c_str_to_str(snapshot_json) else {
        set_last_error("divi_undercroft_health_metrics: invalid snapshot_json");
        return std::ptr::null_mut();
    };

    let snapshot: HealthSnapshot = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_undercroft_health_metrics: snapshot: {e}"));
            return std::ptr::null_mut();
        }
    };

    let store_stats: Option<globe::StoreStats> = if store_stats_json.is_null() {
        None
    } else if let Some(ss) = c_str_to_str(store_stats_json) {
        match serde_json::from_str(ss) {
            Ok(s) => Some(s),
            Err(e) => {
                set_last_error(format!(
                    "divi_undercroft_health_metrics: store_stats: {e}"
                ));
                return std::ptr::null_mut();
            }
        }
    } else {
        None
    };

    let metrics = HealthMetrics::from_snapshot(&snapshot, node_count, store_stats.as_ref());
    json_to_c(&metrics)
}

// ===================================================================
// HealthHistory — opaque pointer (ring buffer of snapshots)
// ===================================================================

pub struct UndercraftHistory(pub(crate) Mutex<HealthHistory>);

/// Create a new health history with the given maximum retention.
///
/// Free with `divi_undercroft_history_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_undercroft_history_new(capacity: usize) -> *mut UndercraftHistory {
    Box::into_raw(Box::new(UndercraftHistory(Mutex::new(
        HealthHistory::new(capacity),
    ))))
}

/// Create a health history with default retention (168 = 1 week of hourly snapshots).
///
/// Free with `divi_undercroft_history_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_undercroft_history_default() -> *mut UndercraftHistory {
    Box::into_raw(Box::new(UndercraftHistory(Mutex::new(
        HealthHistory::default(),
    ))))
}

/// Free a health history.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_undercroft_history_free(ptr: *mut UndercraftHistory) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Push a snapshot into the history. Evicts the oldest if at capacity.
///
/// `snapshot_json` is a JSON `HealthSnapshot`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `history` must be a valid pointer. `snapshot_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_undercroft_history_push(
    history: *const UndercraftHistory,
    snapshot_json: *const c_char,
) -> i32 {
    clear_last_error();

    let history = unsafe { &*history };
    let Some(sj) = c_str_to_str(snapshot_json) else {
        set_last_error("divi_undercroft_history_push: invalid snapshot_json");
        return -1;
    };

    let snapshot: HealthSnapshot = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_undercroft_history_push: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&history.0);
    guard.push(snapshot);
    0
}

/// Get the most recent snapshot, if any.
///
/// Returns JSON (HealthSnapshot) or null if empty. Caller must free via `divi_free_string`.
///
/// # Safety
/// `history` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_undercroft_history_latest(
    history: *const UndercraftHistory,
) -> *mut c_char {
    let history = unsafe { &*history };
    let guard = lock_or_recover(&history.0);
    match guard.latest() {
        Some(snapshot) => json_to_c(snapshot),
        None => std::ptr::null_mut(),
    }
}

/// Get the number of snapshots in the history.
///
/// # Safety
/// `history` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_undercroft_history_len(
    history: *const UndercraftHistory,
) -> usize {
    let history = unsafe { &*history };
    let guard = lock_or_recover(&history.0);
    guard.len()
}

/// Whether the history is empty.
///
/// # Safety
/// `history` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_undercroft_history_is_empty(
    history: *const UndercraftHistory,
) -> bool {
    let history = unsafe { &*history };
    let guard = lock_or_recover(&history.0);
    guard.is_empty()
}

/// Get all snapshots as a JSON array (oldest to newest).
///
/// Returns JSON array of HealthSnapshot. Caller must free via `divi_free_string`.
///
/// # Safety
/// `history` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_undercroft_history_all(
    history: *const UndercraftHistory,
) -> *mut c_char {
    let history = unsafe { &*history };
    let guard = lock_or_recover(&history.0);
    let snapshots: Vec<&HealthSnapshot> = guard.iter().collect();
    json_to_c(&snapshots)
}

// ===================================================================
// RelayPrivacyHealth — stateless JSON
// ===================================================================

/// Validate and round-trip a RelayPrivacyHealth JSON.
///
/// `health_json` is a JSON `RelayPrivacyHealth`.
/// Returns JSON (RelayPrivacyHealth). Caller must free via `divi_free_string`.
///
/// # Safety
/// `health_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_undercroft_privacy_health(
    health_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(hj) = c_str_to_str(health_json) else {
        set_last_error("divi_undercroft_privacy_health: invalid health_json");
        return std::ptr::null_mut();
    };

    let health: undercroft::RelayPrivacyHealth = match serde_json::from_str(hj) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(format!("divi_undercroft_privacy_health: {e}"));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&health)
}

/// Compute the privacy health score from a RelayPrivacyHealth JSON.
///
/// `health_json` is a JSON `RelayPrivacyHealth`.
/// Returns the health score (0.0-1.0), or -1.0 on error.
///
/// # Safety
/// `health_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_undercroft_privacy_health_score(
    health_json: *const c_char,
) -> f64 {
    clear_last_error();

    let Some(hj) = c_str_to_str(health_json) else {
        set_last_error("divi_undercroft_privacy_health_score: invalid health_json");
        return -1.0;
    };

    let health: undercroft::RelayPrivacyHealth = match serde_json::from_str(hj) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(format!("divi_undercroft_privacy_health_score: {e}"));
            return -1.0;
        }
    };

    health.health_score()
}

// ===================================================================
// QuestHealth — stateless JSON
// ===================================================================

/// Build quest health from an ObservatoryReport JSON.
///
/// `report_json` is a JSON `quest::ObservatoryReport`.
/// Returns JSON (QuestHealth). Caller must free via `divi_free_string`.
///
/// # Safety
/// `report_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_undercroft_quest_health(
    report_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(rj) = c_str_to_str(report_json) else {
        set_last_error("divi_undercroft_quest_health: invalid report_json");
        return std::ptr::null_mut();
    };

    let report: quest::ObservatoryReport = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_undercroft_quest_health: {e}"));
            return std::ptr::null_mut();
        }
    };

    let health = QuestHealth::from_report(&report);
    json_to_c(&health)
}

/// Create an empty QuestHealth snapshot.
///
/// Returns JSON (QuestHealth). Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_undercroft_quest_health_empty() -> *mut c_char {
    json_to_c(&QuestHealth::empty())
}

/// Compute the quest health score from a QuestHealth JSON.
///
/// `quest_health_json` is a JSON `QuestHealth`.
/// Returns the health score (0.0-1.0), or -1.0 on error.
///
/// # Safety
/// `quest_health_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_undercroft_quest_health_score(
    quest_health_json: *const c_char,
) -> f64 {
    clear_last_error();

    let Some(qj) = c_str_to_str(quest_health_json) else {
        set_last_error("divi_undercroft_quest_health_score: invalid quest_health_json");
        return -1.0;
    };

    let health: QuestHealth = match serde_json::from_str(qj) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(format!("divi_undercroft_quest_health_score: {e}"));
            return -1.0;
        }
    };

    health.health_score()
}

// ===================================================================
// QuestHealth FFI tests
// ===================================================================

#[cfg(test)]
mod quest_ffi_tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn quest_health_empty() {
        let result = divi_undercroft_quest_health_empty();
        assert!(!result.is_null());
        let json = unsafe { std::ffi::CStr::from_ptr(result).to_str().unwrap() };
        let health: QuestHealth = serde_json::from_str(json).unwrap();
        assert_eq!(health.total_participants(), 0);
        assert_eq!(health.health_score(), 0.0);
        unsafe { crate::helpers::divi_free_string(result) };
    }

    #[test]
    fn quest_health_from_report() {
        let report = quest::ObservatoryReport::empty();
        let report_json = serde_json::to_string(&report).unwrap();
        let c_json = CString::new(report_json).unwrap();

        let result = unsafe { divi_undercroft_quest_health(c_json.as_ptr()) };
        assert!(!result.is_null());
        let json = unsafe { std::ffi::CStr::from_ptr(result).to_str().unwrap() };
        let health: QuestHealth = serde_json::from_str(json).unwrap();
        assert_eq!(health.total_participants(), 0);
        unsafe { crate::helpers::divi_free_string(result) };
    }

    #[test]
    fn quest_health_null_safety() {
        let result = unsafe { divi_undercroft_quest_health(std::ptr::null()) };
        assert!(result.is_null());
    }

    #[test]
    fn quest_health_invalid_json() {
        let bad = CString::new("not json").unwrap();
        let result = unsafe { divi_undercroft_quest_health(bad.as_ptr()) };
        assert!(result.is_null());
    }

    #[test]
    fn quest_health_score_happy_path() {
        let health = QuestHealth::empty();
        let health_json = serde_json::to_string(&health).unwrap();
        let c_json = CString::new(health_json).unwrap();

        let score = unsafe { divi_undercroft_quest_health_score(c_json.as_ptr()) };
        assert!((score - 0.0).abs() < 0.001);
    }

    #[test]
    fn quest_health_score_null_safety() {
        let score = unsafe { divi_undercroft_quest_health_score(std::ptr::null()) };
        assert!((score - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn quest_health_score_invalid_json() {
        let bad = CString::new("garbage").unwrap();
        let score = unsafe { divi_undercroft_quest_health_score(bad.as_ptr()) };
        assert!((score - (-1.0)).abs() < 0.001);
    }
}
