use std::ffi::{c_char, CString};
use std::os::raw::c_void;
use std::sync::Mutex;
use std::time::Duration;

use advisor::{
    AdvisorConfig, CognitiveLoop, CognitiveStore, ExpressionConsent, GenerationResult, Memory,
    ProviderCapabilities, ProviderInfo, ProviderPreferences, ProviderRegistry, ProviderRouter,
    ProviderStatus, SecurityTier, Session, SkillDefinition, SkillRegistry, SponsorshipBond,
    StateCommand, Synapse, Thought, ThoughtSource,
};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// CognitiveLoop — opaque pointer (the brain stem)
// ===================================================================

/// Thread-safe wrapper around `CognitiveLoop` for FFI.
pub struct AdvisorLoop(pub(crate) Mutex<CognitiveLoop>);

/// Create a new cognitive loop.
///
/// `config_json` is a JSON `AdvisorConfig` (or null for default).
/// `session_id` is a UUID string for the home session.
/// Free with `divi_advisor_loop_free`.
///
/// # Safety
/// `session_id` must be a valid C string. `config_json` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_loop_new(
    config_json: *const c_char,
    session_id: *const c_char,
) -> *mut AdvisorLoop {
    clear_last_error();

    let Some(sid) = c_str_to_str(session_id) else {
        set_last_error("divi_advisor_loop_new: invalid session_id");
        return std::ptr::null_mut();
    };

    let uuid = match uuid::Uuid::parse_str(sid) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_advisor_loop_new: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let config = if config_json.is_null() {
        AdvisorConfig::default()
    } else if let Some(cj) = c_str_to_str(config_json) {
        serde_json::from_str(cj).unwrap_or_default()
    } else {
        AdvisorConfig::default()
    };

    Box::into_raw(Box::new(AdvisorLoop(Mutex::new(CognitiveLoop::new(
        config, uuid,
    )))))
}

/// Free a cognitive loop.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_loop_free(ptr: *mut AdvisorLoop) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Advance the loop by one tick. Returns JSON array of `CognitiveAction`.
///
/// Caller must free the returned string via `divi_free_string`.
///
/// # Safety
/// `loop_ptr` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_loop_tick(
    loop_ptr: *const AdvisorLoop,
    elapsed_ms: u64,
) -> *mut c_char {
    if loop_ptr.is_null() {
        set_last_error("divi_advisor_loop_tick: null pointer");
        return std::ptr::null_mut();
    }

    let lp = unsafe { &*loop_ptr };
    let mut guard = lock_or_recover(&lp.0);
    let actions = guard.tick(Duration::from_millis(elapsed_ms));
    json_to_c(&actions)
}

/// Feed an LLM generation result back into the loop.
///
/// `result_json` is a JSON `GenerationResult`.
/// Returns JSON array of `CognitiveAction`.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `loop_ptr` must be valid. `result_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_loop_receive_generation(
    loop_ptr: *const AdvisorLoop,
    result_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    if loop_ptr.is_null() {
        set_last_error("divi_advisor_loop_receive_generation: null pointer");
        return std::ptr::null_mut();
    }

    let Some(rj) = c_str_to_str(result_json) else {
        set_last_error("divi_advisor_loop_receive_generation: invalid result_json");
        return std::ptr::null_mut();
    };

    let result: GenerationResult = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!(
                "divi_advisor_loop_receive_generation: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    let lp = unsafe { &*loop_ptr };
    let mut guard = lock_or_recover(&lp.0);
    let actions = guard.receive_generation(result);
    json_to_c(&actions)
}

/// Apply a state command to the loop.
///
/// `cmd_json` is a JSON `StateCommand`.
/// Returns JSON array of `CognitiveAction`.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `loop_ptr` must be valid. `cmd_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_loop_apply_command(
    loop_ptr: *const AdvisorLoop,
    cmd_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    if loop_ptr.is_null() {
        set_last_error("divi_advisor_loop_apply_command: null pointer");
        return std::ptr::null_mut();
    }

    let Some(cj) = c_str_to_str(cmd_json) else {
        set_last_error("divi_advisor_loop_apply_command: invalid cmd_json");
        return std::ptr::null_mut();
    };

    let cmd: StateCommand = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_advisor_loop_apply_command: {e}"));
            return std::ptr::null_mut();
        }
    };

    let lp = unsafe { &*loop_ptr };
    let mut guard = lock_or_recover(&lp.0);
    let actions = guard.apply_command(cmd);
    json_to_c(&actions)
}

/// Set the energy level (0.0-1.0) on the cognitive loop.
///
/// # Safety
/// `loop_ptr` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_loop_set_energy(
    loop_ptr: *const AdvisorLoop,
    energy: f64,
) {
    if loop_ptr.is_null() {
        return;
    }
    let lp = unsafe { &*loop_ptr };
    let mut guard = lock_or_recover(&lp.0);
    guard.set_energy(energy);
}

/// Set the novelty level (0.0-1.0) on the cognitive loop.
///
/// # Safety
/// `loop_ptr` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_loop_set_novelty(
    loop_ptr: *const AdvisorLoop,
    novelty: f64,
) {
    if loop_ptr.is_null() {
        return;
    }
    let lp = unsafe { &*loop_ptr };
    let mut guard = lock_or_recover(&lp.0);
    guard.set_novelty(novelty);
}

/// Notify the loop that a conversation started.
///
/// # Safety
/// `loop_ptr` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_loop_begin_conversation(
    loop_ptr: *const AdvisorLoop,
) {
    if loop_ptr.is_null() {
        return;
    }
    let lp = unsafe { &*loop_ptr };
    let mut guard = lock_or_recover(&lp.0);
    guard.begin_conversation();
}

/// Notify the loop that a conversation ended.
///
/// # Safety
/// `loop_ptr` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_loop_end_conversation(
    loop_ptr: *const AdvisorLoop,
) {
    if loop_ptr.is_null() {
        return;
    }
    let lp = unsafe { &*loop_ptr };
    let mut guard = lock_or_recover(&lp.0);
    guard.end_conversation();
}

/// Get a pressure snapshot from the loop.
///
/// Returns JSON `PressureSnapshot`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `loop_ptr` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_loop_pressure_snapshot(
    loop_ptr: *const AdvisorLoop,
) -> *mut c_char {
    if loop_ptr.is_null() {
        set_last_error("divi_advisor_loop_pressure_snapshot: null pointer");
        return std::ptr::null_mut();
    }
    let lp = unsafe { &*loop_ptr };
    let guard = lock_or_recover(&lp.0);
    // Use the config's thresholds for snapshot
    let snapshot = guard.pressure.snapshot(0.8, 0.95);
    json_to_c(&snapshot)
}

/// Get the current cognitive mode.
///
/// Returns JSON `CognitiveMode`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `loop_ptr` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_loop_mode(
    loop_ptr: *const AdvisorLoop,
) -> *mut c_char {
    if loop_ptr.is_null() {
        set_last_error("divi_advisor_loop_mode: null pointer");
        return std::ptr::null_mut();
    }
    let lp = unsafe { &*loop_ptr };
    let guard = lock_or_recover(&lp.0);
    json_to_c(&guard.mode)
}

/// Get the current expression consent.
///
/// Returns JSON `ExpressionConsent`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `loop_ptr` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_loop_consent(
    loop_ptr: *const AdvisorLoop,
) -> *mut c_char {
    if loop_ptr.is_null() {
        set_last_error("divi_advisor_loop_consent: null pointer");
        return std::ptr::null_mut();
    }
    let lp = unsafe { &*loop_ptr };
    let guard = lock_or_recover(&lp.0);
    json_to_c(&guard.consent)
}

/// Set expression consent on the loop.
///
/// `consent_json` is a JSON `ExpressionConsent`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `loop_ptr` must be valid. `consent_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_loop_set_consent(
    loop_ptr: *const AdvisorLoop,
    consent_json: *const c_char,
) -> i32 {
    clear_last_error();

    if loop_ptr.is_null() {
        set_last_error("divi_advisor_loop_set_consent: null pointer");
        return -1;
    }

    let Some(cj) = c_str_to_str(consent_json) else {
        set_last_error("divi_advisor_loop_set_consent: invalid consent_json");
        return -1;
    };

    let consent: ExpressionConsent = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_advisor_loop_set_consent: {e}"));
            return -1;
        }
    };

    let lp = unsafe { &*loop_ptr };
    let mut guard = lock_or_recover(&lp.0);
    guard.consent = consent;
    0
}

// ===================================================================
// CognitiveStore — opaque pointer (in-memory cognitive state)
// ===================================================================

/// Thread-safe wrapper around `CognitiveStore` for FFI.
pub struct AdvisorStore(pub(crate) Mutex<CognitiveStore>);

/// Create a new cognitive store.
///
/// `clipboard_max` is the maximum number of clipboard entries.
/// Free with `divi_advisor_store_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_advisor_store_new(clipboard_max: usize) -> *mut AdvisorStore {
    Box::into_raw(Box::new(AdvisorStore(Mutex::new(CognitiveStore::new(
        clipboard_max,
    )))))
}

/// Free a cognitive store.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_free(ptr: *mut AdvisorStore) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Save a thought to the store.
///
/// `thought_json` is a JSON `Thought`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `store` must be a valid pointer. `thought_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_save_thought(
    store: *const AdvisorStore,
    thought_json: *const c_char,
) -> i32 {
    clear_last_error();

    if store.is_null() {
        set_last_error("divi_advisor_store_save_thought: null pointer");
        return -1;
    }

    let Some(tj) = c_str_to_str(thought_json) else {
        set_last_error("divi_advisor_store_save_thought: invalid thought_json");
        return -1;
    };

    let thought: Thought = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_advisor_store_save_thought: {e}"));
            return -1;
        }
    };

    let store = unsafe { &*store };
    let mut guard = lock_or_recover(&store.0);
    guard.save_thought(thought);
    0
}

/// Get a thought by ID.
///
/// `id` is a UUID string.
/// Returns JSON `Thought`, or null if not found.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `store` must be valid. `id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_get_thought(
    store: *const AdvisorStore,
    id: *const c_char,
) -> *mut c_char {
    if store.is_null() {
        set_last_error("divi_advisor_store_get_thought: null pointer");
        return std::ptr::null_mut();
    }

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_advisor_store_get_thought: invalid id");
        return std::ptr::null_mut();
    };

    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_advisor_store_get_thought: {e}"));
            return std::ptr::null_mut();
        }
    };

    let store = unsafe { &*store };
    let guard = lock_or_recover(&store.0);
    match guard.get_thought(uuid) {
        Some(thought) => json_to_c(thought),
        None => std::ptr::null_mut(),
    }
}

/// Get all thoughts for a session.
///
/// `session_id` is a UUID string.
/// Returns JSON array of `Thought`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `store` must be valid. `session_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_thoughts_for_session(
    store: *const AdvisorStore,
    session_id: *const c_char,
) -> *mut c_char {
    if store.is_null() {
        set_last_error("divi_advisor_store_thoughts_for_session: null pointer");
        return std::ptr::null_mut();
    }

    let Some(sid) = c_str_to_str(session_id) else {
        set_last_error("divi_advisor_store_thoughts_for_session: invalid session_id");
        return std::ptr::null_mut();
    };

    let uuid = match uuid::Uuid::parse_str(sid) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!(
                "divi_advisor_store_thoughts_for_session: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    let store = unsafe { &*store };
    let guard = lock_or_recover(&store.0);
    let thoughts: Vec<&Thought> = guard.thoughts_for_session(uuid);
    json_to_c(&thoughts)
}

/// Delete a thought by ID.
///
/// `id` is a UUID string.
/// Returns true if the thought existed and was deleted.
///
/// # Safety
/// `store` must be valid. `id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_delete_thought(
    store: *const AdvisorStore,
    id: *const c_char,
) -> bool {
    if store.is_null() {
        return false;
    }

    let Some(id_str) = c_str_to_str(id) else {
        return false;
    };

    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(_) => return false,
    };

    let store = unsafe { &*store };
    let mut guard = lock_or_recover(&store.0);
    guard.delete_thought(uuid)
}

/// Save a session to the store.
///
/// `session_json` is a JSON `Session`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `store` must be valid. `session_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_save_session(
    store: *const AdvisorStore,
    session_json: *const c_char,
) -> i32 {
    clear_last_error();

    if store.is_null() {
        set_last_error("divi_advisor_store_save_session: null pointer");
        return -1;
    }

    let Some(sj) = c_str_to_str(session_json) else {
        set_last_error("divi_advisor_store_save_session: invalid session_json");
        return -1;
    };

    let session: Session = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_advisor_store_save_session: {e}"));
            return -1;
        }
    };

    let store = unsafe { &*store };
    let mut guard = lock_or_recover(&store.0);
    guard.save_session(session);
    0
}

/// Get a session by ID.
///
/// `id` is a UUID string.
/// Returns JSON `Session`, or null if not found.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `store` must be valid. `id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_get_session(
    store: *const AdvisorStore,
    id: *const c_char,
) -> *mut c_char {
    if store.is_null() {
        set_last_error("divi_advisor_store_get_session: null pointer");
        return std::ptr::null_mut();
    }

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_advisor_store_get_session: invalid id");
        return std::ptr::null_mut();
    };

    let uuid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_advisor_store_get_session: {e}"));
            return std::ptr::null_mut();
        }
    };

    let store = unsafe { &*store };
    let guard = lock_or_recover(&store.0);
    match guard.get_session(uuid) {
        Some(session) => json_to_c(session),
        None => std::ptr::null_mut(),
    }
}

/// Get all active (non-archived) sessions.
///
/// Returns JSON array of `Session`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `store` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_active_sessions(
    store: *const AdvisorStore,
) -> *mut c_char {
    if store.is_null() {
        set_last_error("divi_advisor_store_active_sessions: null pointer");
        return std::ptr::null_mut();
    }

    let store = unsafe { &*store };
    let guard = lock_or_recover(&store.0);
    let sessions: Vec<&Session> = guard.active_sessions();
    json_to_c(&sessions)
}

/// Save a memory to the store.
///
/// `memory_json` is a JSON `Memory`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `store` must be valid. `memory_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_save_memory(
    store: *const AdvisorStore,
    memory_json: *const c_char,
) -> i32 {
    clear_last_error();

    if store.is_null() {
        set_last_error("divi_advisor_store_save_memory: null pointer");
        return -1;
    }

    let Some(mj) = c_str_to_str(memory_json) else {
        set_last_error("divi_advisor_store_save_memory: invalid memory_json");
        return -1;
    };

    let memory: Memory = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_advisor_store_save_memory: {e}"));
            return -1;
        }
    };

    let store = unsafe { &*store };
    let mut guard = lock_or_recover(&store.0);
    guard.save_memory(memory);
    0
}

/// Search memories by keyword.
///
/// Returns JSON array of `MemoryResult`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `store` must be valid. `query` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_search_memories(
    store: *const AdvisorStore,
    query: *const c_char,
    max_results: usize,
) -> *mut c_char {
    clear_last_error();

    if store.is_null() {
        set_last_error("divi_advisor_store_search_memories: null pointer");
        return std::ptr::null_mut();
    }

    let Some(q) = c_str_to_str(query) else {
        set_last_error("divi_advisor_store_search_memories: invalid query");
        return std::ptr::null_mut();
    };

    let store = unsafe { &*store };
    let mut guard = lock_or_recover(&store.0);
    let results = guard.search_memories(q, max_results);
    json_to_c(&results)
}

/// Save a synapse to the store.
///
/// `synapse_json` is a JSON `Synapse`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `store` must be valid. `synapse_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_save_synapse(
    store: *const AdvisorStore,
    synapse_json: *const c_char,
) -> i32 {
    clear_last_error();

    if store.is_null() {
        set_last_error("divi_advisor_store_save_synapse: null pointer");
        return -1;
    }

    let Some(sj) = c_str_to_str(synapse_json) else {
        set_last_error("divi_advisor_store_save_synapse: invalid synapse_json");
        return -1;
    };

    let synapse: Synapse = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_advisor_store_save_synapse: {e}"));
            return -1;
        }
    };

    let store = unsafe { &*store };
    let mut guard = lock_or_recover(&store.0);
    guard.save_synapse(synapse);
    0
}

/// Prune weak synapses below the given minimum strength.
///
/// Returns the number of synapses pruned.
///
/// # Safety
/// `store` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_prune_weak_synapses(
    store: *const AdvisorStore,
    min_strength: f64,
) -> usize {
    if store.is_null() {
        return 0;
    }

    let store = unsafe { &*store };
    let mut guard = lock_or_recover(&store.0);
    guard.prune_weak_synapses(min_strength)
}

/// Get the number of thoughts in the store.
///
/// # Safety
/// `store` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_thought_count(
    store: *const AdvisorStore,
) -> usize {
    if store.is_null() {
        return 0;
    }
    let store = unsafe { &*store };
    let guard = lock_or_recover(&store.0);
    guard.thought_count()
}

/// Get the number of sessions in the store.
///
/// # Safety
/// `store` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_session_count(
    store: *const AdvisorStore,
) -> usize {
    if store.is_null() {
        return 0;
    }
    let store = unsafe { &*store };
    let guard = lock_or_recover(&store.0);
    guard.session_count()
}

/// Get the number of memories in the store.
///
/// # Safety
/// `store` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_memory_count(
    store: *const AdvisorStore,
) -> usize {
    if store.is_null() {
        return 0;
    }
    let store = unsafe { &*store };
    let guard = lock_or_recover(&store.0);
    guard.memory_count()
}

/// Get the number of synapses in the store.
///
/// # Safety
/// `store` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_store_synapse_count(
    store: *const AdvisorStore,
) -> usize {
    if store.is_null() {
        return 0;
    }
    let store = unsafe { &*store };
    let guard = lock_or_recover(&store.0);
    guard.synapse_count()
}

// ===================================================================
// ProviderRouter — opaque pointer (multi-provider selection)
// ===================================================================

/// Thread-safe wrapper around `ProviderRouter` for FFI.
pub struct AdvisorRouter(pub(crate) Mutex<ProviderRouter>);

/// Create a new provider router with an empty registry.
/// Free with `divi_advisor_router_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_advisor_router_new() -> *mut AdvisorRouter {
    Box::into_raw(Box::new(AdvisorRouter(Mutex::new(ProviderRouter::new(
        ProviderRegistry::new(),
    )))))
}

/// Free a provider router.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_router_free(ptr: *mut AdvisorRouter) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

// ---------------------------------------------------------------------------
// Callback type for provider status
// ---------------------------------------------------------------------------

/// C function pointer for querying provider status.
///
/// Returns a `*mut c_char` containing JSON `ProviderStatus`.
/// The returned pointer must be allocated by the caller (Swift/C) side.
pub type DiviAdvisorStatusCallback =
    unsafe extern "C" fn(context: *mut c_void) -> *mut c_char;

/// An FFI-bridged cognitive provider that captures registration data
/// and calls back into Swift for `status()`.
struct FfiCognitiveProvider {
    id: String,
    display_name: String,
    capabilities: ProviderCapabilities,
    is_cloud: bool,
    status_fn: DiviAdvisorStatusCallback,
    /// `*mut c_void` cast to `usize` for `Send + Sync`.
    /// Safety: the caller guarantees the context pointer is thread-safe
    /// and valid for the lifetime of this provider.
    ctx: usize,
}

// Safety: The caller guarantees the context pointer and callback are
// thread-safe. This follows the same pattern as formula_ffi.rs.
unsafe impl Send for FfiCognitiveProvider {}
unsafe impl Sync for FfiCognitiveProvider {}

impl advisor::CognitiveProvider for FfiCognitiveProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities
    }

    fn is_cloud(&self) -> bool {
        self.is_cloud
    }

    fn status(&self) -> ProviderStatus {
        let result_ptr =
            unsafe { (self.status_fn)(self.ctx as *mut c_void) };

        if result_ptr.is_null() {
            return ProviderStatus::Unavailable {
                reason: "status callback returned null".into(),
            };
        }

        let result_cstr = unsafe { CString::from_raw(result_ptr) };
        let result_str = match result_cstr.to_str() {
            Ok(s) => s,
            Err(_) => {
                return ProviderStatus::Unavailable {
                    reason: "status callback returned invalid UTF-8".into(),
                }
            }
        };

        serde_json::from_str(result_str).unwrap_or(ProviderStatus::Unavailable {
            reason: "status callback returned invalid JSON".into(),
        })
    }
}

/// Register a provider in the router.
///
/// `id` and `name` are C strings identifying the provider.
/// `capabilities_bitflags` is the raw u32 value of `ProviderCapabilities`.
/// `is_cloud` indicates whether the provider requires cloud access.
/// `status_fn` is called to get the provider's current status.
/// `context` is an opaque pointer passed to the status callback.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `router`, `id`, `name` must be valid pointers. `status_fn` must be valid.
/// `context` must be valid for the lifetime of the provider registration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_router_register_provider(
    router: *const AdvisorRouter,
    id: *const c_char,
    name: *const c_char,
    capabilities_bitflags: u32,
    is_cloud: bool,
    status_fn: DiviAdvisorStatusCallback,
    context: *mut c_void,
) -> i32 {
    clear_last_error();

    if router.is_null() {
        set_last_error("divi_advisor_router_register_provider: null router");
        return -1;
    }

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_advisor_router_register_provider: invalid id");
        return -1;
    };

    let Some(name_str) = c_str_to_str(name) else {
        set_last_error("divi_advisor_router_register_provider: invalid name");
        return -1;
    };

    let caps = ProviderCapabilities::from_bits_truncate(capabilities_bitflags);

    let provider = FfiCognitiveProvider {
        id: id_str.to_string(),
        display_name: name_str.to_string(),
        capabilities: caps,
        is_cloud,
        status_fn,
        ctx: context as usize,
    };

    let router = unsafe { &*router };
    let mut guard = lock_or_recover(&router.0);
    guard.registry.register(Box::new(provider));
    0
}

/// Unregister a provider from the router.
///
/// Returns 0 on success, -1 on error (provider not found).
///
/// # Safety
/// `router` must be valid. `id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_router_unregister(
    router: *const AdvisorRouter,
    id: *const c_char,
) -> i32 {
    clear_last_error();

    if router.is_null() {
        set_last_error("divi_advisor_router_unregister: null router");
        return -1;
    }

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_advisor_router_unregister: invalid id");
        return -1;
    };

    let router = unsafe { &*router };
    let mut guard = lock_or_recover(&router.0);
    match guard.registry.unregister(id_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Set provider preferences on the router.
///
/// `preferences_json` is a JSON `ProviderPreferences`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `router` must be valid. `preferences_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_router_set_preferences(
    router: *const AdvisorRouter,
    preferences_json: *const c_char,
) -> i32 {
    clear_last_error();

    if router.is_null() {
        set_last_error("divi_advisor_router_set_preferences: null router");
        return -1;
    }

    let Some(pj) = c_str_to_str(preferences_json) else {
        set_last_error("divi_advisor_router_set_preferences: invalid preferences_json");
        return -1;
    };

    let prefs: ProviderPreferences = match serde_json::from_str(pj) {
        Ok(p) => p,
        Err(e) => {
            set_last_error(format!(
                "divi_advisor_router_set_preferences: {e}"
            ));
            return -1;
        }
    };

    let router = unsafe { &*router };
    let mut guard = lock_or_recover(&router.0);
    guard.preferences = prefs;
    0
}

/// Set the security tier on the router.
///
/// `tier_json` is a JSON `SecurityTier`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `router` must be valid. `tier_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_router_set_security_tier(
    router: *const AdvisorRouter,
    tier_json: *const c_char,
) -> i32 {
    clear_last_error();

    if router.is_null() {
        set_last_error("divi_advisor_router_set_security_tier: null router");
        return -1;
    }

    let Some(tj) = c_str_to_str(tier_json) else {
        set_last_error("divi_advisor_router_set_security_tier: invalid tier_json");
        return -1;
    };

    let tier: SecurityTier = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!(
                "divi_advisor_router_set_security_tier: {e}"
            ));
            return -1;
        }
    };

    let router = unsafe { &*router };
    let mut guard = lock_or_recover(&router.0);
    guard.security_tier = tier;
    0
}

/// Select the best provider for a request with the given capability requirements.
///
/// `required_caps` is the raw u32 value of required `ProviderCapabilities`.
/// Returns JSON `ProviderInfo`, or null if no provider matches.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `router` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_router_select(
    router: *const AdvisorRouter,
    required_caps: u32,
) -> *mut c_char {
    clear_last_error();

    if router.is_null() {
        set_last_error("divi_advisor_router_select: null router");
        return std::ptr::null_mut();
    }

    let required = ProviderCapabilities::from_bits_truncate(required_caps);

    let router = unsafe { &*router };
    let guard = lock_or_recover(&router.0);
    match guard.select(required) {
        Ok(provider) => {
            let info = ProviderInfo::from_provider(provider);
            json_to_c(&info)
        }
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Get info for all registered providers.
///
/// Returns JSON array of `ProviderInfo`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `router` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_router_provider_info(
    router: *const AdvisorRouter,
) -> *mut c_char {
    if router.is_null() {
        set_last_error("divi_advisor_router_provider_info: null router");
        return std::ptr::null_mut();
    }

    let router = unsafe { &*router };
    let guard = lock_or_recover(&router.0);
    let info = guard.registry.provider_info();
    json_to_c(&info)
}

// ===================================================================
// SkillRegistry — opaque pointer (tool/function registry)
// ===================================================================

/// Thread-safe wrapper around `SkillRegistry` for FFI.
pub struct AdvisorSkills(pub(crate) Mutex<SkillRegistry>);

/// Create a new empty skill registry.
/// Free with `divi_advisor_skills_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_advisor_skills_new() -> *mut AdvisorSkills {
    Box::into_raw(Box::new(AdvisorSkills(Mutex::new(SkillRegistry::new()))))
}

/// Free a skill registry.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_skills_free(ptr: *mut AdvisorSkills) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Register a skill.
///
/// `skill_json` is a JSON `SkillDefinition`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `skills` must be valid. `skill_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_skills_register(
    skills: *const AdvisorSkills,
    skill_json: *const c_char,
) -> i32 {
    clear_last_error();

    if skills.is_null() {
        set_last_error("divi_advisor_skills_register: null pointer");
        return -1;
    }

    let Some(sj) = c_str_to_str(skill_json) else {
        set_last_error("divi_advisor_skills_register: invalid skill_json");
        return -1;
    };

    let skill: SkillDefinition = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_advisor_skills_register: {e}"));
            return -1;
        }
    };

    let skills = unsafe { &*skills };
    let mut guard = lock_or_recover(&skills.0);
    guard.register(skill);
    0
}

/// Unregister a skill by ID.
///
/// Returns 0 on success, -1 on error (skill not found).
///
/// # Safety
/// `skills` must be valid. `id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_skills_unregister(
    skills: *const AdvisorSkills,
    id: *const c_char,
) -> i32 {
    clear_last_error();

    if skills.is_null() {
        set_last_error("divi_advisor_skills_unregister: null pointer");
        return -1;
    }

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_advisor_skills_unregister: invalid id");
        return -1;
    };

    let skills = unsafe { &*skills };
    let mut guard = lock_or_recover(&skills.0);
    match guard.unregister(id_str) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get a skill by ID.
///
/// Returns JSON `SkillDefinition`, or null if not found.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// `skills` must be valid. `id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_skills_get(
    skills: *const AdvisorSkills,
    id: *const c_char,
) -> *mut c_char {
    if skills.is_null() {
        set_last_error("divi_advisor_skills_get: null pointer");
        return std::ptr::null_mut();
    }

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_advisor_skills_get: invalid id");
        return std::ptr::null_mut();
    };

    let skills = unsafe { &*skills };
    let guard = lock_or_recover(&skills.0);
    match guard.get(id_str) {
        Some(skill) => json_to_c(skill),
        None => std::ptr::null_mut(),
    }
}

/// Search skills by name/description.
///
/// Returns JSON array of `SkillDefinition`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `skills` must be valid. `query` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_skills_search(
    skills: *const AdvisorSkills,
    query: *const c_char,
) -> *mut c_char {
    if skills.is_null() {
        set_last_error("divi_advisor_skills_search: null pointer");
        return std::ptr::null_mut();
    }

    let Some(q) = c_str_to_str(query) else {
        set_last_error("divi_advisor_skills_search: invalid query");
        return std::ptr::null_mut();
    };

    let skills = unsafe { &*skills };
    let guard = lock_or_recover(&skills.0);
    let results: Vec<&SkillDefinition> = guard.search(q);
    json_to_c(&results)
}

/// Get all registered skills.
///
/// Returns JSON array of `SkillDefinition`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `skills` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_skills_all(
    skills: *const AdvisorSkills,
) -> *mut c_char {
    if skills.is_null() {
        set_last_error("divi_advisor_skills_all: null pointer");
        return std::ptr::null_mut();
    }

    let skills = unsafe { &*skills };
    let guard = lock_or_recover(&skills.0);
    let all: Vec<&SkillDefinition> = guard.all();
    json_to_c(&all)
}

/// Get the number of registered skills.
///
/// # Safety
/// `skills` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_skills_count(
    skills: *const AdvisorSkills,
) -> usize {
    if skills.is_null() {
        return 0;
    }
    let skills = unsafe { &*skills };
    let guard = lock_or_recover(&skills.0);
    guard.len()
}

// ===================================================================
// Stateless JSON — config presets and factory functions
// ===================================================================

/// Get the default `AdvisorConfig` as JSON.
///
/// Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_advisor_config_default() -> *mut c_char {
    json_to_c(&AdvisorConfig::default())
}

/// Get the contemplative `AdvisorConfig` preset as JSON.
///
/// Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_advisor_config_contemplative() -> *mut c_char {
    json_to_c(&AdvisorConfig::contemplative())
}

/// Get the responsive `AdvisorConfig` preset as JSON.
///
/// Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_advisor_config_responsive() -> *mut c_char {
    json_to_c(&AdvisorConfig::responsive())
}

/// Create a new `Thought` as JSON.
///
/// `session_id` is a UUID string.
/// `content` is the thought content.
/// `source_json` is a JSON `ThoughtSource`.
/// Returns JSON `Thought`. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_thought_new(
    session_id: *const c_char,
    content: *const c_char,
    source_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(sid) = c_str_to_str(session_id) else {
        set_last_error("divi_advisor_thought_new: invalid session_id");
        return std::ptr::null_mut();
    };

    let uuid = match uuid::Uuid::parse_str(sid) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_advisor_thought_new: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let Some(c) = c_str_to_str(content) else {
        set_last_error("divi_advisor_thought_new: invalid content");
        return std::ptr::null_mut();
    };

    let Some(sj) = c_str_to_str(source_json) else {
        set_last_error("divi_advisor_thought_new: invalid source_json");
        return std::ptr::null_mut();
    };

    let source: ThoughtSource = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_advisor_thought_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let thought = Thought::new(uuid, c, source);
    json_to_c(&thought)
}

/// Create a home session as JSON.
///
/// Returns JSON `Session`. Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_advisor_session_home() -> *mut c_char {
    json_to_c(&Session::home())
}

/// Create a user session as JSON.
///
/// `title` is a C string for the session title.
/// Returns JSON `Session`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `title` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_session_user(
    title: *const c_char,
) -> *mut c_char {
    let Some(t) = c_str_to_str(title) else {
        set_last_error("divi_advisor_session_user: invalid title");
        return std::ptr::null_mut();
    };
    json_to_c(&Session::user(t))
}

/// Create a new sponsorship bond as JSON.
///
/// `sponsor` is the sponsor's public key (crown_id).
/// `companion` is a UUID string for the companion identity.
/// Returns JSON `SponsorshipBond`. Caller must free via `divi_free_string`.
///
/// # Safety
/// `sponsor` and `companion` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_advisor_sponsorship_bond_new(
    sponsor: *const c_char,
    companion: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(s) = c_str_to_str(sponsor) else {
        set_last_error("divi_advisor_sponsorship_bond_new: invalid sponsor");
        return std::ptr::null_mut();
    };

    let Some(c) = c_str_to_str(companion) else {
        set_last_error("divi_advisor_sponsorship_bond_new: invalid companion");
        return std::ptr::null_mut();
    };

    let uuid = match uuid::Uuid::parse_str(c) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!(
                "divi_advisor_sponsorship_bond_new: invalid UUID: {e}"
            ));
            return std::ptr::null_mut();
        }
    };

    let bond = SponsorshipBond::new(s, uuid);
    json_to_c(&bond)
}

/// Get the default `ExpressionConsent` as JSON.
///
/// Returns JSON `ExpressionConsent`. Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_advisor_expression_consent_default() -> *mut c_char {
    json_to_c(&ExpressionConsent::default())
}

// ===================================================================
// Tests
// ===================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use advisor::CognitiveMode;
    use std::ffi::CStr;

    /// A simple status callback that returns `Available`.
    unsafe extern "C" fn available_status(_context: *mut c_void) -> *mut c_char {
        let status = ProviderStatus::Available;
        let json = serde_json::to_string(&status).unwrap();
        CString::new(json).unwrap().into_raw()
    }

    #[test]
    fn loop_lifecycle() {
        let sid = uuid::Uuid::new_v4().to_string();
        let c_sid = CString::new(sid).unwrap();

        let lp = unsafe {
            divi_advisor_loop_new(std::ptr::null(), c_sid.as_ptr())
        };
        assert!(!lp.is_null());

        // Tick
        let actions = unsafe { divi_advisor_loop_tick(lp, 2000) };
        assert!(!actions.is_null());
        unsafe { crate::helpers::divi_free_string(actions) };

        // Mode
        let mode = unsafe { divi_advisor_loop_mode(lp) };
        assert!(!mode.is_null());
        let mode_str = unsafe { CStr::from_ptr(mode) }.to_str().unwrap();
        assert!(mode_str.contains("Assistant"));
        unsafe { crate::helpers::divi_free_string(mode) };

        // Consent
        let consent = unsafe { divi_advisor_loop_consent(lp) };
        assert!(!consent.is_null());
        unsafe { crate::helpers::divi_free_string(consent) };

        // Pressure snapshot
        let pressure = unsafe { divi_advisor_loop_pressure_snapshot(lp) };
        assert!(!pressure.is_null());
        unsafe { crate::helpers::divi_free_string(pressure) };

        // Energy and novelty
        unsafe { divi_advisor_loop_set_energy(lp, 0.8) };
        unsafe { divi_advisor_loop_set_novelty(lp, 0.5) };

        // Conversation lifecycle
        unsafe { divi_advisor_loop_begin_conversation(lp) };
        unsafe { divi_advisor_loop_end_conversation(lp) };

        unsafe { divi_advisor_loop_free(lp) };
    }

    #[test]
    fn loop_apply_command() {
        let sid = uuid::Uuid::new_v4().to_string();
        let c_sid = CString::new(sid).unwrap();

        let lp = unsafe {
            divi_advisor_loop_new(std::ptr::null(), c_sid.as_ptr())
        };

        let cmd = StateCommand::SetMode(CognitiveMode::Autonomous);
        let cmd_json = CString::new(serde_json::to_string(&cmd).unwrap()).unwrap();
        let actions = unsafe {
            divi_advisor_loop_apply_command(lp, cmd_json.as_ptr())
        };
        assert!(!actions.is_null());
        unsafe { crate::helpers::divi_free_string(actions) };

        // Verify mode changed
        let mode = unsafe { divi_advisor_loop_mode(lp) };
        let mode_str = unsafe { CStr::from_ptr(mode) }.to_str().unwrap();
        assert!(mode_str.contains("Autonomous"));
        unsafe { crate::helpers::divi_free_string(mode) };

        unsafe { divi_advisor_loop_free(lp) };
    }

    #[test]
    fn loop_set_consent() {
        let sid = uuid::Uuid::new_v4().to_string();
        let c_sid = CString::new(sid).unwrap();

        let lp = unsafe {
            divi_advisor_loop_new(std::ptr::null(), c_sid.as_ptr())
        };

        let consent = ExpressionConsent {
            granted: false,
            level: advisor::ConsentLevel::Silent,
        };
        let consent_json = CString::new(serde_json::to_string(&consent).unwrap()).unwrap();
        let result = unsafe {
            divi_advisor_loop_set_consent(lp, consent_json.as_ptr())
        };
        assert_eq!(result, 0);

        unsafe { divi_advisor_loop_free(lp) };
    }

    #[test]
    fn store_lifecycle() {
        let store = divi_advisor_store_new(100);
        assert!(!store.is_null());

        // Create and save a thought
        let session_id = uuid::Uuid::new_v4();
        let thought = Thought::new(session_id, "test thought", ThoughtSource::Autonomous);
        let thought_id = thought.id;
        let thought_json = CString::new(serde_json::to_string(&thought).unwrap()).unwrap();

        let result = unsafe {
            divi_advisor_store_save_thought(store, thought_json.as_ptr())
        };
        assert_eq!(result, 0);
        assert_eq!(unsafe { divi_advisor_store_thought_count(store) }, 1);

        // Get thought
        let id_str = CString::new(thought_id.to_string()).unwrap();
        let got = unsafe { divi_advisor_store_get_thought(store, id_str.as_ptr()) };
        assert!(!got.is_null());
        unsafe { crate::helpers::divi_free_string(got) };

        // Thoughts for session
        let sid_str = CString::new(session_id.to_string()).unwrap();
        let thoughts = unsafe {
            divi_advisor_store_thoughts_for_session(store, sid_str.as_ptr())
        };
        assert!(!thoughts.is_null());
        unsafe { crate::helpers::divi_free_string(thoughts) };

        // Delete thought
        assert!(unsafe { divi_advisor_store_delete_thought(store, id_str.as_ptr()) });
        assert_eq!(unsafe { divi_advisor_store_thought_count(store) }, 0);

        // Save session
        let session = Session::user("test session");
        let session_json = CString::new(serde_json::to_string(&session).unwrap()).unwrap();
        let result = unsafe {
            divi_advisor_store_save_session(store, session_json.as_ptr())
        };
        assert_eq!(result, 0);
        assert_eq!(unsafe { divi_advisor_store_session_count(store) }, 1);

        // Active sessions
        let active = unsafe { divi_advisor_store_active_sessions(store) };
        assert!(!active.is_null());
        unsafe { crate::helpers::divi_free_string(active) };

        // Save memory
        let memory = Memory::new("test memory");
        let memory_json = CString::new(serde_json::to_string(&memory).unwrap()).unwrap();
        let result = unsafe {
            divi_advisor_store_save_memory(store, memory_json.as_ptr())
        };
        assert_eq!(result, 0);
        assert_eq!(unsafe { divi_advisor_store_memory_count(store) }, 1);

        // Search memories
        let query = CString::new("test").unwrap();
        let results = unsafe {
            divi_advisor_store_search_memories(store, query.as_ptr(), 10)
        };
        assert!(!results.is_null());
        unsafe { crate::helpers::divi_free_string(results) };

        // Save synapse
        let synapse = Synapse::thought_relates(uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), 0.5);
        let synapse_json = CString::new(serde_json::to_string(&synapse).unwrap()).unwrap();
        let result = unsafe {
            divi_advisor_store_save_synapse(store, synapse_json.as_ptr())
        };
        assert_eq!(result, 0);
        assert_eq!(unsafe { divi_advisor_store_synapse_count(store) }, 1);

        // Prune weak synapses (none should be pruned at 0.1)
        let pruned = unsafe { divi_advisor_store_prune_weak_synapses(store, 0.1) };
        assert_eq!(pruned, 0);

        unsafe { divi_advisor_store_free(store) };
    }

    #[test]
    fn router_lifecycle() {
        let router = divi_advisor_router_new();
        assert!(!router.is_null());

        // Register a provider
        let id = CString::new("test-local").unwrap();
        let name = CString::new("Test Local Model").unwrap();
        let caps = ProviderCapabilities::STREAMING | ProviderCapabilities::OFFLINE_CAPABLE;

        let result = unsafe {
            divi_advisor_router_register_provider(
                router,
                id.as_ptr(),
                name.as_ptr(),
                caps.bits(),
                false,
                available_status,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(result, 0);

        // Provider info
        let info = unsafe { divi_advisor_router_provider_info(router) };
        assert!(!info.is_null());
        let info_str = unsafe { CStr::from_ptr(info) }.to_str().unwrap();
        assert!(info_str.contains("test-local"));
        unsafe { crate::helpers::divi_free_string(info) };

        // Select
        let selected = unsafe {
            divi_advisor_router_select(router, ProviderCapabilities::STREAMING.bits())
        };
        assert!(!selected.is_null());
        unsafe { crate::helpers::divi_free_string(selected) };

        // Set preferences
        let prefs = ProviderPreferences::default();
        let prefs_json = CString::new(serde_json::to_string(&prefs).unwrap()).unwrap();
        let result = unsafe {
            divi_advisor_router_set_preferences(router, prefs_json.as_ptr())
        };
        assert_eq!(result, 0);

        // Set security tier
        let tier_json = CString::new(serde_json::to_string(&SecurityTier::Ultimate).unwrap()).unwrap();
        let result = unsafe {
            divi_advisor_router_set_security_tier(router, tier_json.as_ptr())
        };
        assert_eq!(result, 0);

        // Unregister
        let result = unsafe { divi_advisor_router_unregister(router, id.as_ptr()) };
        assert_eq!(result, 0);

        unsafe { divi_advisor_router_free(router) };
    }

    #[test]
    fn skills_lifecycle() {
        let skills = divi_advisor_skills_new();
        assert!(!skills.is_null());

        // Register
        let skill = SkillDefinition::new("web.search", "Web Search", "Search the web");
        let skill_json = CString::new(serde_json::to_string(&skill).unwrap()).unwrap();
        let result = unsafe { divi_advisor_skills_register(skills, skill_json.as_ptr()) };
        assert_eq!(result, 0);
        assert_eq!(unsafe { divi_advisor_skills_count(skills) }, 1);

        // Get
        let id = CString::new("web.search").unwrap();
        let got = unsafe { divi_advisor_skills_get(skills, id.as_ptr()) };
        assert!(!got.is_null());
        unsafe { crate::helpers::divi_free_string(got) };

        // Search
        let query = CString::new("search").unwrap();
        let results = unsafe { divi_advisor_skills_search(skills, query.as_ptr()) };
        assert!(!results.is_null());
        unsafe { crate::helpers::divi_free_string(results) };

        // All
        let all = unsafe { divi_advisor_skills_all(skills) };
        assert!(!all.is_null());
        unsafe { crate::helpers::divi_free_string(all) };

        // Unregister
        let result = unsafe { divi_advisor_skills_unregister(skills, id.as_ptr()) };
        assert_eq!(result, 0);
        assert_eq!(unsafe { divi_advisor_skills_count(skills) }, 0);

        unsafe { divi_advisor_skills_free(skills) };
    }

    #[test]
    fn stateless_config_presets() {
        let default = divi_advisor_config_default();
        assert!(!default.is_null());
        unsafe { crate::helpers::divi_free_string(default) };

        let contemplative = divi_advisor_config_contemplative();
        assert!(!contemplative.is_null());
        unsafe { crate::helpers::divi_free_string(contemplative) };

        let responsive = divi_advisor_config_responsive();
        assert!(!responsive.is_null());
        unsafe { crate::helpers::divi_free_string(responsive) };
    }

    #[test]
    fn stateless_thought_new() {
        let sid = CString::new(uuid::Uuid::new_v4().to_string()).unwrap();
        let content = CString::new("a new insight").unwrap();
        let source = CString::new(
            serde_json::to_string(&ThoughtSource::Autonomous).unwrap(),
        ).unwrap();

        let result = unsafe {
            divi_advisor_thought_new(sid.as_ptr(), content.as_ptr(), source.as_ptr())
        };
        assert!(!result.is_null());
        let result_str = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert!(result_str.contains("a new insight"));
        unsafe { crate::helpers::divi_free_string(result) };
    }

    #[test]
    fn stateless_session_factories() {
        let home = divi_advisor_session_home();
        assert!(!home.is_null());
        let home_str = unsafe { CStr::from_ptr(home) }.to_str().unwrap();
        assert!(home_str.contains("Home"));
        unsafe { crate::helpers::divi_free_string(home) };

        let title = CString::new("Chat about design").unwrap();
        let user = unsafe { divi_advisor_session_user(title.as_ptr()) };
        assert!(!user.is_null());
        let user_str = unsafe { CStr::from_ptr(user) }.to_str().unwrap();
        assert!(user_str.contains("Chat about design"));
        unsafe { crate::helpers::divi_free_string(user) };
    }

    #[test]
    fn stateless_sponsorship_bond() {
        let sponsor = CString::new("cpub1abc").unwrap();
        let companion = CString::new(uuid::Uuid::new_v4().to_string()).unwrap();

        let bond = unsafe {
            divi_advisor_sponsorship_bond_new(sponsor.as_ptr(), companion.as_ptr())
        };
        assert!(!bond.is_null());
        let bond_str = unsafe { CStr::from_ptr(bond) }.to_str().unwrap();
        assert!(bond_str.contains("cpub1abc"));
        unsafe { crate::helpers::divi_free_string(bond) };
    }

    #[test]
    fn stateless_expression_consent() {
        let consent = divi_advisor_expression_consent_default();
        assert!(!consent.is_null());
        let consent_str = unsafe { CStr::from_ptr(consent) }.to_str().unwrap();
        assert!(consent_str.contains("Normal"));
        unsafe { crate::helpers::divi_free_string(consent) };
    }

    #[test]
    fn null_pointers_are_safe() {
        unsafe {
            divi_advisor_loop_free(std::ptr::null_mut());
            divi_advisor_store_free(std::ptr::null_mut());
            divi_advisor_router_free(std::ptr::null_mut());
            divi_advisor_skills_free(std::ptr::null_mut());
        }

        // Null loop operations
        assert!(unsafe { divi_advisor_loop_tick(std::ptr::null(), 100) }.is_null());
        assert!(unsafe { divi_advisor_loop_mode(std::ptr::null()) }.is_null());
        assert!(unsafe { divi_advisor_loop_consent(std::ptr::null()) }.is_null());
        assert!(unsafe {
            divi_advisor_loop_pressure_snapshot(std::ptr::null())
        }.is_null());

        // Null store operations
        assert_eq!(unsafe { divi_advisor_store_thought_count(std::ptr::null()) }, 0);
        assert_eq!(unsafe { divi_advisor_store_session_count(std::ptr::null()) }, 0);
        assert_eq!(unsafe { divi_advisor_store_memory_count(std::ptr::null()) }, 0);
        assert_eq!(unsafe { divi_advisor_store_synapse_count(std::ptr::null()) }, 0);

        // Null skills operations
        assert_eq!(unsafe { divi_advisor_skills_count(std::ptr::null()) }, 0);
    }
}
