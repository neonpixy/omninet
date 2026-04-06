//! Quest FFI -- C bindings for the Quest gamification system.
//!
//! Exposes `QuestEngine` as an opaque pointer with JSON round-trip for data types.
//! All functions use the `divi_quest_` prefix.

use std::ffi::c_char;
use std::sync::Mutex;

use quest::{
    Achievement, AchievementContext, Challenge, ChallengeParticipant, CounterCriteria,
    FlagCriteria, Mission, QuestConfig, QuestEngine, RewardSource, RewardType,
};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// QuestEngineFFI -- opaque pointer
// ===================================================================

/// Thread-safe wrapper around `QuestEngine` for FFI.
pub struct QuestEngineFFI(pub(crate) Mutex<QuestEngine>);

// ===================================================================
// Lifecycle
// ===================================================================

/// Create a new Quest engine.
///
/// `config_json` is a JSON `QuestConfig` (or null for default).
/// Free with `divi_quest_engine_free`.
///
/// # Safety
/// `config_json` may be null (uses default config).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_engine_new(
    config_json: *const c_char,
) -> *mut QuestEngineFFI {
    clear_last_error();

    let config = if config_json.is_null() {
        QuestConfig::default()
    } else if let Some(cj) = c_str_to_str(config_json) {
        match serde_json::from_str(cj) {
            Ok(c) => c,
            Err(e) => {
                set_last_error(format!("divi_quest_engine_new: invalid config JSON: {e}"));
                return std::ptr::null_mut();
            }
        }
    } else {
        set_last_error("divi_quest_engine_new: invalid config string");
        return std::ptr::null_mut();
    };

    Box::into_raw(Box::new(QuestEngineFFI(Mutex::new(QuestEngine::new(
        config,
    )))))
}

/// Free a Quest engine.
///
/// # Safety
/// `ptr` must be valid and called exactly once, or null (no-op).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_engine_free(ptr: *mut QuestEngineFFI) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

// ===================================================================
// Progression
// ===================================================================

/// Award XP to an actor. Returns the new level, or 0 on error.
///
/// # Safety
/// `engine` must be a valid pointer. `actor` and `source` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_award_xp(
    engine: *const QuestEngineFFI,
    actor: *const c_char,
    amount: u64,
    source: *const c_char,
) -> u32 {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return 0;
    };
    let Some(actor) = c_str_to_str(actor) else {
        set_last_error("divi_quest_award_xp: invalid actor");
        return 0;
    };
    let source = c_str_to_str(source).unwrap_or("ffi");

    let mut guard = lock_or_recover(&eng.0);
    guard.award_xp(actor, amount, source)
}

/// Get an actor's progression as JSON. Returns null if the actor has no progression.
///
/// Caller must free the returned string via `divi_free_string`.
///
/// # Safety
/// `engine` and `actor` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_progression(
    engine: *const QuestEngineFFI,
    actor: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return std::ptr::null_mut();
    };
    let Some(actor) = c_str_to_str(actor) else {
        set_last_error("divi_quest_progression: invalid actor");
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&eng.0);
    match guard.progression_for(actor) {
        Some(p) => json_to_c(p),
        None => {
            set_last_error(format!("divi_quest_progression: no progression for {actor}"));
            std::ptr::null_mut()
        }
    }
}

/// Get an actor's quest status as JSON.
///
/// Caller must free the returned string via `divi_free_string`.
///
/// # Safety
/// `engine` and `actor` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_status(
    engine: *const QuestEngineFFI,
    actor: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return std::ptr::null_mut();
    };
    let Some(actor) = c_str_to_str(actor) else {
        set_last_error("divi_quest_status: invalid actor");
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&eng.0);
    let status = guard.status(actor);
    json_to_c(&status)
}

// ===================================================================
// Achievements
// ===================================================================

/// Register a counter-based criteria for achievement evaluation.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// All pointer args must be valid C strings or null-checked.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_register_criteria_counter(
    engine: *const QuestEngineFFI,
    id: *const c_char,
    counter_key: *const c_char,
    target: u64,
) -> i32 {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return -1;
    };
    let Some(id) = c_str_to_str(id) else {
        set_last_error("divi_quest_register_criteria_counter: invalid id");
        return -1;
    };
    let Some(counter_key) = c_str_to_str(counter_key) else {
        set_last_error("divi_quest_register_criteria_counter: invalid counter_key");
        return -1;
    };

    let mut guard = lock_or_recover(&eng.0);
    guard.register_criteria(Box::new(CounterCriteria::new(id, counter_key, target)));
    0
}

/// Register a flag-based criteria for achievement evaluation.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// All pointer args must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_register_criteria_flag(
    engine: *const QuestEngineFFI,
    id: *const c_char,
    flag_key: *const c_char,
    expected: bool,
) -> i32 {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return -1;
    };
    let Some(id) = c_str_to_str(id) else {
        set_last_error("divi_quest_register_criteria_flag: invalid id");
        return -1;
    };
    let Some(flag_key) = c_str_to_str(flag_key) else {
        set_last_error("divi_quest_register_criteria_flag: invalid flag_key");
        return -1;
    };

    let mut guard = lock_or_recover(&eng.0);
    guard.register_criteria(Box::new(FlagCriteria::new(id, flag_key, expected)));
    0
}

/// Register an achievement definition from JSON.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `engine` must be valid. `achievement_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_register_achievement(
    engine: *const QuestEngineFFI,
    achievement_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return -1;
    };
    let Some(json) = c_str_to_str(achievement_json) else {
        set_last_error("divi_quest_register_achievement: invalid JSON");
        return -1;
    };

    let achievement: Achievement = match serde_json::from_str(json) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_quest_register_achievement: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&eng.0);
    guard.register_achievement(achievement);
    0
}

/// Evaluate all achievements for an actor given a context. Returns JSON array of
/// newly-achieved achievement IDs.
///
/// Caller must free the returned string via `divi_free_string`.
///
/// # Safety
/// All pointer args must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_evaluate_achievements(
    engine: *const QuestEngineFFI,
    actor: *const c_char,
    context_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return std::ptr::null_mut();
    };
    let Some(actor) = c_str_to_str(actor) else {
        set_last_error("divi_quest_evaluate_achievements: invalid actor");
        return std::ptr::null_mut();
    };
    let Some(ctx_str) = c_str_to_str(context_json) else {
        set_last_error("divi_quest_evaluate_achievements: invalid context JSON");
        return std::ptr::null_mut();
    };

    let context: AchievementContext = match serde_json::from_str(ctx_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_quest_evaluate_achievements: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut guard = lock_or_recover(&eng.0);
    let achieved = guard.evaluate_achievements(actor, &context);
    json_to_c(&achieved)
}

// ===================================================================
// Missions
// ===================================================================

/// Add a mission definition from JSON.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `engine` must be valid. `mission_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_add_mission(
    engine: *const QuestEngineFFI,
    mission_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return -1;
    };
    let Some(json) = c_str_to_str(mission_json) else {
        set_last_error("divi_quest_add_mission: invalid JSON");
        return -1;
    };

    let mission: Mission = match serde_json::from_str(json) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_quest_add_mission: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&eng.0);
    guard.add_mission(mission);
    0
}

/// Start a mission for an actor.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// All pointer args must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_start_mission(
    engine: *const QuestEngineFFI,
    actor: *const c_char,
    mission_id: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return -1;
    };
    let Some(actor) = c_str_to_str(actor) else {
        set_last_error("divi_quest_start_mission: invalid actor");
        return -1;
    };
    let Some(mid_str) = c_str_to_str(mission_id) else {
        set_last_error("divi_quest_start_mission: invalid mission_id");
        return -1;
    };
    let mid = match uuid::Uuid::parse_str(mid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_quest_start_mission: invalid UUID: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&eng.0);
    match guard.start_mission(actor, &mid) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(format!("divi_quest_start_mission: {e}"));
            -1
        }
    }
}

/// Update progress on a specific objective within a mission.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// All pointer args must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_update_objective(
    engine: *const QuestEngineFFI,
    actor: *const c_char,
    mission_id: *const c_char,
    objective_id: *const c_char,
    increment: u64,
) -> i32 {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return -1;
    };
    let Some(actor) = c_str_to_str(actor) else {
        set_last_error("divi_quest_update_objective: invalid actor");
        return -1;
    };
    let Some(mid_str) = c_str_to_str(mission_id) else {
        set_last_error("divi_quest_update_objective: invalid mission_id");
        return -1;
    };
    let mid = match uuid::Uuid::parse_str(mid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_quest_update_objective: invalid UUID: {e}"));
            return -1;
        }
    };
    let Some(obj_id) = c_str_to_str(objective_id) else {
        set_last_error("divi_quest_update_objective: invalid objective_id");
        return -1;
    };

    let mut guard = lock_or_recover(&eng.0);
    match guard.complete_objective(actor, &mid, obj_id, increment) {
        Ok(_) => 0,
        Err(e) => {
            set_last_error(format!("divi_quest_update_objective: {e}"));
            -1
        }
    }
}

/// Get active missions for an actor as JSON array of `MissionProgress`.
///
/// Caller must free the returned string via `divi_free_string`.
///
/// # Safety
/// `engine` and `actor` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_active_missions(
    engine: *const QuestEngineFFI,
    actor: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return std::ptr::null_mut();
    };
    let Some(actor) = c_str_to_str(actor) else {
        set_last_error("divi_quest_active_missions: invalid actor");
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&eng.0);
    let active = guard.active_missions(actor);
    json_to_c(&active)
}

/// Get available missions for an actor as JSON array of `Mission`.
///
/// Caller must free the returned string via `divi_free_string`.
///
/// # Safety
/// `engine` and `actor` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_available_missions(
    engine: *const QuestEngineFFI,
    actor: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return std::ptr::null_mut();
    };
    let Some(actor) = c_str_to_str(actor) else {
        set_last_error("divi_quest_available_missions: invalid actor");
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&eng.0);
    let available = guard.available_missions(actor);
    json_to_c(&available)
}

// ===================================================================
// Challenges
// ===================================================================

/// Add a challenge definition from JSON.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `engine` must be valid. `challenge_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_add_challenge(
    engine: *const QuestEngineFFI,
    challenge_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return -1;
    };
    let Some(json) = c_str_to_str(challenge_json) else {
        set_last_error("divi_quest_add_challenge: invalid JSON");
        return -1;
    };

    let challenge: Challenge = match serde_json::from_str(json) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_quest_add_challenge: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&eng.0);
    guard.challenges.add_challenge(challenge);
    0
}

/// Join a challenge. `participant_json` is a JSON `ChallengeParticipant`.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// All pointer args must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_join_challenge(
    engine: *const QuestEngineFFI,
    challenge_id: *const c_char,
    participant_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return -1;
    };
    let Some(cid_str) = c_str_to_str(challenge_id) else {
        set_last_error("divi_quest_join_challenge: invalid challenge_id");
        return -1;
    };
    let cid = match uuid::Uuid::parse_str(cid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_quest_join_challenge: invalid UUID: {e}"));
            return -1;
        }
    };
    let Some(pj) = c_str_to_str(participant_json) else {
        set_last_error("divi_quest_join_challenge: invalid participant JSON");
        return -1;
    };
    let participant: ChallengeParticipant = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!("divi_quest_join_challenge: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&eng.0);
    match guard.challenges.join(cid, participant) {
        Ok(_) => 0,
        Err(e) => {
            set_last_error(format!("divi_quest_join_challenge: {e}"));
            -1
        }
    }
}

/// Get challenge progress as JSON (entries for a challenge).
///
/// Caller must free the returned string via `divi_free_string`.
///
/// # Safety
/// All pointer args must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_challenge_progress(
    engine: *const QuestEngineFFI,
    challenge_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return std::ptr::null_mut();
    };
    let Some(cid_str) = c_str_to_str(challenge_id) else {
        set_last_error("divi_quest_challenge_progress: invalid challenge_id");
        return std::ptr::null_mut();
    };
    let cid = match uuid::Uuid::parse_str(cid_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!(
                "divi_quest_challenge_progress: invalid UUID: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&eng.0);
    let entries = guard.challenges.participants(cid);
    json_to_c(&entries)
}

// ===================================================================
// Rewards
// ===================================================================

/// Grant a reward to an actor.
///
/// `reward_json` is a JSON `RewardType`. `source_type_json` is a JSON `RewardSource`.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// All pointer args must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_grant_reward(
    engine: *const QuestEngineFFI,
    actor: *const c_char,
    reward_json: *const c_char,
    source_id: *const c_char,
    source_type_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return -1;
    };
    let Some(actor) = c_str_to_str(actor) else {
        set_last_error("divi_quest_grant_reward: invalid actor");
        return -1;
    };
    let Some(rj) = c_str_to_str(reward_json) else {
        set_last_error("divi_quest_grant_reward: invalid reward JSON");
        return -1;
    };
    let reward_type: RewardType = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_quest_grant_reward: invalid reward: {e}"));
            return -1;
        }
    };
    let Some(sid) = c_str_to_str(source_id) else {
        set_last_error("divi_quest_grant_reward: invalid source_id");
        return -1;
    };
    let Some(stj) = c_str_to_str(source_type_json) else {
        set_last_error("divi_quest_grant_reward: invalid source_type JSON");
        return -1;
    };
    let source_type: RewardSource = match serde_json::from_str(stj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!(
                "divi_quest_grant_reward: invalid source_type: {e}"
            ));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&eng.0);
    guard.grant_reward(actor, reward_type, sid, source_type);
    0
}

/// Get all rewards for an actor as JSON array.
///
/// Caller must free the returned string via `divi_free_string`.
///
/// # Safety
/// `engine` and `actor` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_rewards_for(
    engine: *const QuestEngineFFI,
    actor: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return std::ptr::null_mut();
    };
    let Some(actor) = c_str_to_str(actor) else {
        set_last_error("divi_quest_rewards_for: invalid actor");
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&eng.0);
    let rewards = guard.rewards_for(actor);
    json_to_c(&rewards)
}

/// Get total Cool currency earned by an actor.
///
/// Returns 0 if the actor has no rewards or on error.
///
/// # Safety
/// `engine` and `actor` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_total_cool(
    engine: *const QuestEngineFFI,
    actor: *const c_char,
) -> u64 {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return 0;
    };
    let Some(actor) = c_str_to_str(actor) else {
        set_last_error("divi_quest_total_cool: invalid actor");
        return 0;
    };

    let guard = lock_or_recover(&eng.0);
    guard.total_cool_for(actor)
}

// ===================================================================
// Summary & Config
// ===================================================================

/// Get the engine's aggregate summary as JSON.
///
/// Caller must free the returned string via `divi_free_string`.
///
/// # Safety
/// `engine` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_summary(
    engine: *const QuestEngineFFI,
) -> *mut c_char {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&eng.0);
    let summary = guard.summary();
    json_to_c(&summary)
}

/// Get the engine's current configuration as JSON.
///
/// Caller must free the returned string via `divi_free_string`.
///
/// # Safety
/// `engine` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_quest_engine_config(
    engine: *const QuestEngineFFI,
) -> *mut c_char {
    clear_last_error();

    let Some(eng) = checked_engine(engine) else {
        return std::ptr::null_mut();
    };

    let guard = lock_or_recover(&eng.0);
    json_to_c(&guard.config)
}

// ===================================================================
// Internal helpers
// ===================================================================

/// Validate and dereference an engine pointer.
fn checked_engine(ptr: *const QuestEngineFFI) -> Option<&'static QuestEngineFFI> {
    if ptr.is_null() {
        set_last_error("null engine pointer");
        return None;
    }
    // SAFETY: caller guarantees the pointer is valid for the duration of the call.
    Some(unsafe { &*ptr })
}

// ===================================================================
// Tests
// ===================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::c_str_to_str;
    use std::ffi::CString;

    /// Helper to create a C string from a Rust string.
    fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    // --- Lifecycle ---

    #[test]
    fn engine_new_default() {
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            assert!(!engine.is_null());
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn engine_new_with_config() {
        let config = QuestConfig::casual();
        let json = serde_json::to_string(&config).unwrap();
        let cjson = c(&json);
        unsafe {
            let engine = divi_quest_engine_new(cjson.as_ptr());
            assert!(!engine.is_null());
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn engine_new_invalid_json() {
        let cjson = c("{not valid json}");
        unsafe {
            let engine = divi_quest_engine_new(cjson.as_ptr());
            assert!(engine.is_null());
        }
    }

    #[test]
    fn engine_free_null_is_noop() {
        unsafe {
            divi_quest_engine_free(std::ptr::null_mut());
        }
    }

    // --- XP & Progression ---

    #[test]
    fn award_xp_basic() {
        let actor = c("alice");
        let source = c("test");
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let level = divi_quest_award_xp(engine, actor.as_ptr(), 100, source.as_ptr());
            assert_eq!(level, 2); // default: 100 XP = level 2
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn award_xp_null_engine() {
        let actor = c("alice");
        let source = c("test");
        unsafe {
            let level = divi_quest_award_xp(std::ptr::null(), actor.as_ptr(), 100, source.as_ptr());
            assert_eq!(level, 0);
        }
    }

    #[test]
    fn award_xp_null_actor() {
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let source = c("test");
            let level = divi_quest_award_xp(engine, std::ptr::null(), 100, source.as_ptr());
            assert_eq!(level, 0);
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn progression_after_xp() {
        let actor = c("alice");
        let source = c("test");
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            divi_quest_award_xp(engine, actor.as_ptr(), 50, source.as_ptr());

            let json_ptr = divi_quest_progression(engine, actor.as_ptr());
            assert!(!json_ptr.is_null());
            let json = c_str_to_str(json_ptr).unwrap();
            assert!(json.contains("\"total_xp\":50"));
            crate::helpers::divi_free_string(json_ptr);

            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn progression_null_engine() {
        let actor = c("alice");
        unsafe {
            let ptr = divi_quest_progression(std::ptr::null(), actor.as_ptr());
            assert!(ptr.is_null());
        }
    }

    #[test]
    fn progression_no_actor() {
        let actor = c("nobody");
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let ptr = divi_quest_progression(engine, actor.as_ptr());
            assert!(ptr.is_null()); // no progression for unknown actor
            divi_quest_engine_free(engine);
        }
    }

    // --- Status ---

    #[test]
    fn status_returns_json() {
        let actor = c("alice");
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let json_ptr = divi_quest_status(engine, actor.as_ptr());
            assert!(!json_ptr.is_null());
            let json = c_str_to_str(json_ptr).unwrap();
            assert!(json.contains("\"actor\":\"alice\""));
            assert!(json.contains("\"level\":1"));
            crate::helpers::divi_free_string(json_ptr);
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn status_null_engine() {
        let actor = c("alice");
        unsafe {
            let ptr = divi_quest_status(std::ptr::null(), actor.as_ptr());
            assert!(ptr.is_null());
        }
    }

    // --- Achievements ---

    #[test]
    fn register_criteria_counter() {
        let id = c("posts-10");
        let key = c("posts_created");
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let result =
                divi_quest_register_criteria_counter(engine, id.as_ptr(), key.as_ptr(), 10);
            assert_eq!(result, 0);
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn register_criteria_counter_null_engine() {
        let id = c("x");
        let key = c("y");
        unsafe {
            let result =
                divi_quest_register_criteria_counter(std::ptr::null(), id.as_ptr(), key.as_ptr(), 1);
            assert_eq!(result, -1);
        }
    }

    #[test]
    fn register_criteria_flag() {
        let id = c("onboarded");
        let key = c("is_onboarded");
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let result =
                divi_quest_register_criteria_flag(engine, id.as_ptr(), key.as_ptr(), true);
            assert_eq!(result, 0);
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn register_criteria_flag_null_engine() {
        let id = c("x");
        let key = c("y");
        unsafe {
            let result =
                divi_quest_register_criteria_flag(std::ptr::null(), id.as_ptr(), key.as_ptr(), true);
            assert_eq!(result, -1);
        }
    }

    #[test]
    fn register_achievement_from_json() {
        let achievement = Achievement::new(
            "first",
            "First",
            "desc",
            quest::AchievementCategory::Creation,
            "posts-10",
            quest::AchievementTier::Bronze,
        );
        let json_str = serde_json::to_string(&achievement).unwrap();
        let cjson = c(&json_str);
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let result = divi_quest_register_achievement(engine, cjson.as_ptr());
            assert_eq!(result, 0);
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn register_achievement_invalid_json() {
        let cjson = c("{bad}");
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let result = divi_quest_register_achievement(engine, cjson.as_ptr());
            assert_eq!(result, -1);
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn evaluate_achievements() {
        let id_c = c("posts-10");
        let key_c = c("posts_created");
        let actor_c = c("alice");

        let achievement = Achievement::new(
            "first",
            "First",
            "desc",
            quest::AchievementCategory::Creation,
            "posts-10",
            quest::AchievementTier::Bronze,
        );
        let ach_json = c(&serde_json::to_string(&achievement).unwrap());

        let mut ctx = AchievementContext::new("alice");
        ctx.counters.insert("posts_created".to_owned(), 10);
        let ctx_json = c(&serde_json::to_string(&ctx).unwrap());

        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            divi_quest_register_criteria_counter(engine, id_c.as_ptr(), key_c.as_ptr(), 10);
            divi_quest_register_achievement(engine, ach_json.as_ptr());

            let result_ptr =
                divi_quest_evaluate_achievements(engine, actor_c.as_ptr(), ctx_json.as_ptr());
            assert!(!result_ptr.is_null());
            let result_str = c_str_to_str(result_ptr).unwrap();
            assert!(result_str.contains("first"));
            crate::helpers::divi_free_string(result_ptr);

            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn evaluate_achievements_null_engine() {
        let actor = c("alice");
        let ctx = c("{}");
        unsafe {
            let ptr =
                divi_quest_evaluate_achievements(std::ptr::null(), actor.as_ptr(), ctx.as_ptr());
            assert!(ptr.is_null());
        }
    }

    // --- Missions ---

    #[test]
    fn add_mission_from_json() {
        let mission = Mission::new("Test", "A test mission");
        let json_str = serde_json::to_string(&mission).unwrap();
        let cjson = c(&json_str);
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let result = divi_quest_add_mission(engine, cjson.as_ptr());
            assert_eq!(result, 0);
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn add_mission_invalid_json() {
        let cjson = c("{bad}");
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let result = divi_quest_add_mission(engine, cjson.as_ptr());
            assert_eq!(result, -1);
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn mission_lifecycle_through_ffi() {
        use quest::mission::Objective;

        let mission = Mission::new("Basics", "Learn basics")
            .with_objective(Objective::new("open", "Open app", 1, "opened"))
            .with_xp_reward(50);
        let mid = mission.id;
        let mid_str = mid.to_string();
        let mission_json = c(&serde_json::to_string(&mission).unwrap());
        let actor = c("alice");
        let mid_c = c(&mid_str);
        let obj_c = c("open");

        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            divi_quest_add_mission(engine, mission_json.as_ptr());
            let result = divi_quest_start_mission(engine, actor.as_ptr(), mid_c.as_ptr());
            assert_eq!(result, 0);

            // Check active missions
            let active_ptr = divi_quest_active_missions(engine, actor.as_ptr());
            assert!(!active_ptr.is_null());
            let active_str = c_str_to_str(active_ptr).unwrap();
            assert!(active_str.contains(&mid_str));
            crate::helpers::divi_free_string(active_ptr);

            // Update objective
            let result = divi_quest_update_objective(
                engine,
                actor.as_ptr(),
                mid_c.as_ptr(),
                obj_c.as_ptr(),
                1,
            );
            assert_eq!(result, 0);

            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn start_mission_not_found() {
        let actor = c("alice");
        let mid = c("00000000-0000-0000-0000-000000000000");
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let result = divi_quest_start_mission(engine, actor.as_ptr(), mid.as_ptr());
            assert_eq!(result, -1);
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn start_mission_null_engine() {
        let actor = c("alice");
        let mid = c("00000000-0000-0000-0000-000000000000");
        unsafe {
            let result = divi_quest_start_mission(std::ptr::null(), actor.as_ptr(), mid.as_ptr());
            assert_eq!(result, -1);
        }
    }

    #[test]
    fn available_missions_empty() {
        let actor = c("alice");
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let ptr = divi_quest_available_missions(engine, actor.as_ptr());
            assert!(!ptr.is_null());
            let json = c_str_to_str(ptr).unwrap();
            assert_eq!(json, "[]");
            crate::helpers::divi_free_string(ptr);
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn available_missions_null_engine() {
        let actor = c("alice");
        unsafe {
            let ptr = divi_quest_available_missions(std::ptr::null(), actor.as_ptr());
            assert!(ptr.is_null());
        }
    }

    // --- Challenges ---

    #[test]
    fn add_challenge_from_json() {
        let challenge = Challenge::new("Sprint", "Publish ideas")
            .with_criteria(quest::ChallengeCriteria {
                metric: "ideas".into(),
                target: 10,
                scope: quest::CriteriaScope::Collective,
            })
            .with_status(quest::ChallengeStatus::Active);
        let json_str = serde_json::to_string(&challenge).unwrap();
        let cjson = c(&json_str);
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let result = divi_quest_add_challenge(engine, cjson.as_ptr());
            assert_eq!(result, 0);
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn add_challenge_null_engine() {
        let cjson = c("{}");
        unsafe {
            let result = divi_quest_add_challenge(std::ptr::null(), cjson.as_ptr());
            assert_eq!(result, -1);
        }
    }

    #[test]
    fn join_challenge_null_engine() {
        let cid = c("00000000-0000-0000-0000-000000000000");
        let pj = c("{\"Individual\":{\"pubkey\":\"alice\"}}");
        unsafe {
            let result = divi_quest_join_challenge(std::ptr::null(), cid.as_ptr(), pj.as_ptr());
            assert_eq!(result, -1);
        }
    }

    #[test]
    fn challenge_progress_null_engine() {
        let cid = c("00000000-0000-0000-0000-000000000000");
        unsafe {
            let ptr = divi_quest_challenge_progress(std::ptr::null(), cid.as_ptr());
            assert!(ptr.is_null());
        }
    }

    // --- Rewards ---

    #[test]
    fn grant_reward_and_query() {
        let actor = c("alice");
        let reward = c("{\"Cool\":100}");
        let source_id = c("mission-1");
        let source_type = c("\"Mission\"");

        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let result = divi_quest_grant_reward(
                engine,
                actor.as_ptr(),
                reward.as_ptr(),
                source_id.as_ptr(),
                source_type.as_ptr(),
            );
            assert_eq!(result, 0);

            let cool = divi_quest_total_cool(engine, actor.as_ptr());
            assert_eq!(cool, 100);

            let rewards_ptr = divi_quest_rewards_for(engine, actor.as_ptr());
            assert!(!rewards_ptr.is_null());
            let rewards_str = c_str_to_str(rewards_ptr).unwrap();
            assert!(rewards_str.contains("Cool"));
            crate::helpers::divi_free_string(rewards_ptr);

            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn grant_reward_null_engine() {
        let actor = c("alice");
        let reward = c("{\"Cool\":100}");
        let source = c("x");
        let stype = c("\"Mission\"");
        unsafe {
            let result = divi_quest_grant_reward(
                std::ptr::null(),
                actor.as_ptr(),
                reward.as_ptr(),
                source.as_ptr(),
                stype.as_ptr(),
            );
            assert_eq!(result, -1);
        }
    }

    #[test]
    fn total_cool_null_engine() {
        let actor = c("alice");
        unsafe {
            let cool = divi_quest_total_cool(std::ptr::null(), actor.as_ptr());
            assert_eq!(cool, 0);
        }
    }

    #[test]
    fn rewards_for_null_engine() {
        let actor = c("alice");
        unsafe {
            let ptr = divi_quest_rewards_for(std::ptr::null(), actor.as_ptr());
            assert!(ptr.is_null());
        }
    }

    // --- Summary & Config ---

    #[test]
    fn summary_returns_json() {
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let ptr = divi_quest_summary(engine);
            assert!(!ptr.is_null());
            let json = c_str_to_str(ptr).unwrap();
            assert!(json.contains("\"total_participants\":0"));
            crate::helpers::divi_free_string(ptr);
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn summary_null_engine() {
        unsafe {
            let ptr = divi_quest_summary(std::ptr::null());
            assert!(ptr.is_null());
        }
    }

    #[test]
    fn config_returns_json() {
        unsafe {
            let engine = divi_quest_engine_new(std::ptr::null());
            let ptr = divi_quest_engine_config(engine);
            assert!(!ptr.is_null());
            let json = c_str_to_str(ptr).unwrap();
            assert!(json.contains("\"max_active_missions\":5"));
            crate::helpers::divi_free_string(ptr);
            divi_quest_engine_free(engine);
        }
    }

    #[test]
    fn config_null_engine() {
        unsafe {
            let ptr = divi_quest_engine_config(std::ptr::null());
            assert!(ptr.is_null());
        }
    }
}
