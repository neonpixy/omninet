use std::ffi::c_char;

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use yoke::{
    ActivityAction, ActivityRecord, CeremonyRecord, CeremonyType, GraphSnapshot, Milestone,
    MilestoneSignificance, ParticipantRole, RelationshipGraph, RelationType, TargetType, Timeline,
    TimelineConfig, TraversalNode, VersionChain, VersionTag, YokeLink,
};

use crate::helpers::{c_str_to_str, json_to_c};
use crate::{clear_last_error, set_last_error};

// ===================================================================
// TraversalNodeJson — local serializable wrapper for TraversalNode
// ===================================================================

/// Serializable representation of [`TraversalNode`] for FFI transport.
///
/// TraversalNode in the Yoke crate derives Debug + Clone but not Serialize,
/// so we map into this local type before crossing the FFI boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TraversalNodeJson {
    entity_id: String,
    depth: usize,
    path: Vec<String>,
}

impl From<TraversalNode> for TraversalNodeJson {
    fn from(node: TraversalNode) -> Self {
        Self {
            entity_id: node.entity_id,
            depth: node.depth,
            path: node.path,
        }
    }
}

// ===================================================================
// Helper: deserialize a GraphSnapshot, build a RelationshipGraph
// ===================================================================

/// Deserialize a JSON GraphSnapshot into a live RelationshipGraph.
fn graph_from_json(json_str: &str, fn_name: &str) -> Option<RelationshipGraph> {
    let snapshot: GraphSnapshot = match serde_json::from_str(json_str) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("{fn_name}: {e}"));
            return None;
        }
    };
    Some(RelationshipGraph::from_snapshot(snapshot))
}

/// Snapshot a RelationshipGraph back to JSON.
fn graph_to_json(graph: &RelationshipGraph) -> *mut c_char {
    json_to_c(&graph.snapshot())
}

// ===================================================================
// YokeLink (2)
// ===================================================================

/// Create a new YokeLink.
///
/// `relation_type_json` is a JSON RelationType (e.g. `"DerivedFrom"` or `{"Custom":"blocks"}`).
/// Returns JSON (YokeLink). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_link_new(
    source: *const c_char,
    target: *const c_char,
    relation_type_json: *const c_char,
    author: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(source_str) = c_str_to_str(source) else {
        set_last_error("divi_yoke_link_new: invalid source");
        return std::ptr::null_mut();
    };
    let Some(target_str) = c_str_to_str(target) else {
        set_last_error("divi_yoke_link_new: invalid target");
        return std::ptr::null_mut();
    };
    let Some(rel_str) = c_str_to_str(relation_type_json) else {
        set_last_error("divi_yoke_link_new: invalid relation_type_json");
        return std::ptr::null_mut();
    };
    let Some(author_str) = c_str_to_str(author) else {
        set_last_error("divi_yoke_link_new: invalid author");
        return std::ptr::null_mut();
    };

    let rel: RelationType = match serde_json::from_str(rel_str) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_yoke_link_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let link = YokeLink::new(source_str, target_str, rel, author_str);
    json_to_c(&link)
}

/// Add metadata to a YokeLink.
///
/// `value_json` is a JSON x::Value (e.g. `{"String":"2.0"}` or `{"Bool":true}`).
/// Returns modified YokeLink JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_link_with_metadata(
    link_json: *const c_char,
    key: *const c_char,
    value_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(lj) = c_str_to_str(link_json) else {
        set_last_error("divi_yoke_link_with_metadata: invalid link_json");
        return std::ptr::null_mut();
    };
    let Some(key_str) = c_str_to_str(key) else {
        set_last_error("divi_yoke_link_with_metadata: invalid key");
        return std::ptr::null_mut();
    };
    let Some(val_str) = c_str_to_str(value_json) else {
        set_last_error("divi_yoke_link_with_metadata: invalid value_json");
        return std::ptr::null_mut();
    };

    let link: YokeLink = match serde_json::from_str(lj) {
        Ok(l) => l,
        Err(e) => {
            set_last_error(format!("divi_yoke_link_with_metadata: {e}"));
            return std::ptr::null_mut();
        }
    };

    let value: x::Value = match serde_json::from_str(val_str) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_yoke_link_with_metadata: {e}"));
            return std::ptr::null_mut();
        }
    };

    let link = link.with_metadata(key_str, value);
    json_to_c(&link)
}

// ===================================================================
// VersionTag (3)
// ===================================================================

/// Create a new VersionTag on the "main" branch.
///
/// `idea_id` is a UUID string. `snapshot_clock_json` is a JSON VectorClock.
/// Returns JSON (VersionTag). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_version_tag_new(
    idea_id: *const c_char,
    name: *const c_char,
    snapshot_clock_json: *const c_char,
    author: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(id_str) = c_str_to_str(idea_id) else {
        set_last_error("divi_yoke_version_tag_new: invalid idea_id");
        return std::ptr::null_mut();
    };
    let Some(name_str) = c_str_to_str(name) else {
        set_last_error("divi_yoke_version_tag_new: invalid name");
        return std::ptr::null_mut();
    };
    let Some(clock_str) = c_str_to_str(snapshot_clock_json) else {
        set_last_error("divi_yoke_version_tag_new: invalid snapshot_clock_json");
        return std::ptr::null_mut();
    };
    let Some(author_str) = c_str_to_str(author) else {
        set_last_error("divi_yoke_version_tag_new: invalid author");
        return std::ptr::null_mut();
    };

    let uid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_tag_new: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let clock: x::VectorClock = match serde_json::from_str(clock_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_tag_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let tag = VersionTag::new(uid, name_str, clock, author_str);
    json_to_c(&tag)
}

/// Add a message to a VersionTag.
///
/// Returns modified VersionTag JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_version_tag_with_message(
    tag_json: *const c_char,
    message: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(tag_json) else {
        set_last_error("divi_yoke_version_tag_with_message: invalid tag_json");
        return std::ptr::null_mut();
    };
    let Some(msg) = c_str_to_str(message) else {
        set_last_error("divi_yoke_version_tag_with_message: invalid message");
        return std::ptr::null_mut();
    };

    let tag: VersionTag = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_tag_with_message: {e}"));
            return std::ptr::null_mut();
        }
    };

    let tag = tag.with_message(msg);
    json_to_c(&tag)
}

/// Set the branch of a VersionTag.
///
/// Returns modified VersionTag JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_version_tag_on_branch(
    tag_json: *const c_char,
    branch: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(tag_json) else {
        set_last_error("divi_yoke_version_tag_on_branch: invalid tag_json");
        return std::ptr::null_mut();
    };
    let Some(br) = c_str_to_str(branch) else {
        set_last_error("divi_yoke_version_tag_on_branch: invalid branch");
        return std::ptr::null_mut();
    };

    let tag: VersionTag = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_tag_on_branch: {e}"));
            return std::ptr::null_mut();
        }
    };

    let tag = tag.on_branch(br);
    json_to_c(&tag)
}

// ===================================================================
// VersionChain (11)
// ===================================================================

/// Create a new empty VersionChain for an idea.
///
/// `idea_id` is a UUID string.
/// Returns JSON (VersionChain). Caller must free via `divi_free_string`.
///
/// # Safety
/// `idea_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_version_chain_new(
    idea_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(id_str) = c_str_to_str(idea_id) else {
        set_last_error("divi_yoke_version_chain_new: invalid idea_id");
        return std::ptr::null_mut();
    };

    let uid = match uuid::Uuid::parse_str(id_str) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_chain_new: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let chain = VersionChain::new(uid);
    json_to_c(&chain)
}

/// Tag a version on a VersionChain.
///
/// `chain_json` is the existing chain, `tag_json` is the VersionTag to add.
/// Returns modified VersionChain JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_version_chain_tag_version(
    chain_json: *const c_char,
    tag_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(chain_json) else {
        set_last_error("divi_yoke_version_chain_tag_version: invalid chain_json");
        return std::ptr::null_mut();
    };
    let Some(tj) = c_str_to_str(tag_json) else {
        set_last_error("divi_yoke_version_chain_tag_version: invalid tag_json");
        return std::ptr::null_mut();
    };

    let mut chain: VersionChain = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_chain_tag_version: {e}"));
            return std::ptr::null_mut();
        }
    };

    let tag: VersionTag = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_chain_tag_version: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = chain.tag_version(tag) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&chain)
}

/// Create a branch on a VersionChain.
///
/// `from_version_id` is the UUID of the version to branch from.
/// Returns modified VersionChain JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_version_chain_create_branch(
    chain_json: *const c_char,
    branch_name: *const c_char,
    from_version_id: *const c_char,
    author: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(chain_json) else {
        set_last_error("divi_yoke_version_chain_create_branch: invalid chain_json");
        return std::ptr::null_mut();
    };
    let Some(bn) = c_str_to_str(branch_name) else {
        set_last_error("divi_yoke_version_chain_create_branch: invalid branch_name");
        return std::ptr::null_mut();
    };
    let Some(fv) = c_str_to_str(from_version_id) else {
        set_last_error("divi_yoke_version_chain_create_branch: invalid from_version_id");
        return std::ptr::null_mut();
    };
    let Some(auth) = c_str_to_str(author) else {
        set_last_error("divi_yoke_version_chain_create_branch: invalid author");
        return std::ptr::null_mut();
    };

    let fv_uuid = match uuid::Uuid::parse_str(fv) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_chain_create_branch: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut chain: VersionChain = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_chain_create_branch: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = chain.create_branch(bn, fv_uuid, auth) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&chain)
}

/// Merge a branch back into another branch.
///
/// `merge_version_id` is a UUID for the merge version record.
/// Returns modified VersionChain JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_version_chain_merge_branch(
    chain_json: *const c_char,
    source_branch: *const c_char,
    target_branch: *const c_char,
    merge_version_id: *const c_char,
    author: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(chain_json) else {
        set_last_error("divi_yoke_version_chain_merge_branch: invalid chain_json");
        return std::ptr::null_mut();
    };
    let Some(sb) = c_str_to_str(source_branch) else {
        set_last_error("divi_yoke_version_chain_merge_branch: invalid source_branch");
        return std::ptr::null_mut();
    };
    let Some(tb) = c_str_to_str(target_branch) else {
        set_last_error("divi_yoke_version_chain_merge_branch: invalid target_branch");
        return std::ptr::null_mut();
    };
    let Some(mv) = c_str_to_str(merge_version_id) else {
        set_last_error("divi_yoke_version_chain_merge_branch: invalid merge_version_id");
        return std::ptr::null_mut();
    };
    let Some(auth) = c_str_to_str(author) else {
        set_last_error("divi_yoke_version_chain_merge_branch: invalid author");
        return std::ptr::null_mut();
    };

    let mv_uuid = match uuid::Uuid::parse_str(mv) {
        Ok(u) => u,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_chain_merge_branch: invalid UUID: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut chain: VersionChain = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_chain_merge_branch: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Err(e) = chain.merge_branch(sb, tb, mv_uuid, auth) {
        set_last_error(e.to_string());
        return std::ptr::null_mut();
    }

    json_to_c(&chain)
}

/// Get all versions on a specific branch, chronologically.
///
/// Returns JSON array of VersionTag. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_version_chain_versions_on_branch(
    chain_json: *const c_char,
    branch: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(chain_json) else {
        set_last_error("divi_yoke_version_chain_versions_on_branch: invalid chain_json");
        return std::ptr::null_mut();
    };
    let Some(br) = c_str_to_str(branch) else {
        set_last_error("divi_yoke_version_chain_versions_on_branch: invalid branch");
        return std::ptr::null_mut();
    };

    let chain: VersionChain = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_chain_versions_on_branch: {e}"));
            return std::ptr::null_mut();
        }
    };

    let versions: Vec<&VersionTag> = chain.versions_on_branch(br);
    json_to_c(&versions)
}

/// Get the latest version on a branch, or null if none.
///
/// Returns JSON (VersionTag) or null. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_version_chain_latest_version(
    chain_json: *const c_char,
    branch: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(chain_json) else {
        set_last_error("divi_yoke_version_chain_latest_version: invalid chain_json");
        return std::ptr::null_mut();
    };
    let Some(br) = c_str_to_str(branch) else {
        set_last_error("divi_yoke_version_chain_latest_version: invalid branch");
        return std::ptr::null_mut();
    };

    let chain: VersionChain = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_chain_latest_version: {e}"));
            return std::ptr::null_mut();
        }
    };

    match chain.latest_version(br) {
        Some(v) => json_to_c(v),
        None => std::ptr::null_mut(),
    }
}

/// Get a version by name (first match across all branches), or null if not found.
///
/// Returns JSON (VersionTag) or null. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_version_chain_version_by_name(
    chain_json: *const c_char,
    name: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(chain_json) else {
        set_last_error("divi_yoke_version_chain_version_by_name: invalid chain_json");
        return std::ptr::null_mut();
    };
    let Some(nm) = c_str_to_str(name) else {
        set_last_error("divi_yoke_version_chain_version_by_name: invalid name");
        return std::ptr::null_mut();
    };

    let chain: VersionChain = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_chain_version_by_name: {e}"));
            return std::ptr::null_mut();
        }
    };

    match chain.version_by_name(nm) {
        Some(v) => json_to_c(v),
        None => std::ptr::null_mut(),
    }
}

/// Get all branch names (including "main").
///
/// Returns JSON array of strings. Caller must free via `divi_free_string`.
///
/// # Safety
/// `chain_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_version_chain_branch_names(
    chain_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(chain_json) else {
        set_last_error("divi_yoke_version_chain_branch_names: invalid chain_json");
        return std::ptr::null_mut();
    };

    let chain: VersionChain = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_chain_branch_names: {e}"));
            return std::ptr::null_mut();
        }
    };

    let names: Vec<&str> = chain.branch_names();
    json_to_c(&names)
}

/// Check if a branch has been merged.
///
/// Returns 1 if merged, 0 if not merged, -1 on error.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_version_chain_is_branch_merged(
    chain_json: *const c_char,
    branch_name: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(cj) = c_str_to_str(chain_json) else {
        set_last_error("divi_yoke_version_chain_is_branch_merged: invalid chain_json");
        return -1;
    };
    let Some(bn) = c_str_to_str(branch_name) else {
        set_last_error("divi_yoke_version_chain_is_branch_merged: invalid branch_name");
        return -1;
    };

    let chain: VersionChain = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_chain_is_branch_merged: {e}"));
            return -1;
        }
    };

    if chain.is_branch_merged(bn) { 1 } else { 0 }
}

/// Get the total number of versions in a chain.
///
/// Returns count on success, -1 on error.
///
/// # Safety
/// `chain_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_version_chain_version_count(
    chain_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(cj) = c_str_to_str(chain_json) else {
        set_last_error("divi_yoke_version_chain_version_count: invalid chain_json");
        return -1;
    };

    let chain: VersionChain = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_chain_version_count: {e}"));
            return -1;
        }
    };

    chain.version_count() as i32
}

/// Get the total number of branches in a chain (including "main").
///
/// Returns count on success, -1 on error.
///
/// # Safety
/// `chain_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_version_chain_branch_count(
    chain_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(cj) = c_str_to_str(chain_json) else {
        set_last_error("divi_yoke_version_chain_branch_count: invalid chain_json");
        return -1;
    };

    let chain: VersionChain = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_version_chain_branch_count: {e}"));
            return -1;
        }
    };

    chain.branch_count() as i32
}

// ===================================================================
// ActivityRecord (3)
// ===================================================================

/// Create a new ActivityRecord.
///
/// `action_json` is a JSON ActivityAction (e.g. `"Created"` or `{"Custom":"archived"}`).
/// `target_type_json` is a JSON TargetType (e.g. `"Idea"` or `{"Custom":"project"}`).
/// Returns JSON (ActivityRecord). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_activity_record_new(
    actor: *const c_char,
    action_json: *const c_char,
    target_id: *const c_char,
    target_type_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(actor_str) = c_str_to_str(actor) else {
        set_last_error("divi_yoke_activity_record_new: invalid actor");
        return std::ptr::null_mut();
    };
    let Some(action_str) = c_str_to_str(action_json) else {
        set_last_error("divi_yoke_activity_record_new: invalid action_json");
        return std::ptr::null_mut();
    };
    let Some(tid) = c_str_to_str(target_id) else {
        set_last_error("divi_yoke_activity_record_new: invalid target_id");
        return std::ptr::null_mut();
    };
    let Some(tt_str) = c_str_to_str(target_type_json) else {
        set_last_error("divi_yoke_activity_record_new: invalid target_type_json");
        return std::ptr::null_mut();
    };

    let action: ActivityAction = match serde_json::from_str(action_str) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_yoke_activity_record_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let target_type: TargetType = match serde_json::from_str(tt_str) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_activity_record_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let record = ActivityRecord::new(actor_str, action, tid, target_type);
    json_to_c(&record)
}

/// Add context to an ActivityRecord.
///
/// Returns modified ActivityRecord JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_activity_record_with_context(
    record_json: *const c_char,
    context: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(rj) = c_str_to_str(record_json) else {
        set_last_error("divi_yoke_activity_record_with_context: invalid record_json");
        return std::ptr::null_mut();
    };
    let Some(ctx) = c_str_to_str(context) else {
        set_last_error("divi_yoke_activity_record_with_context: invalid context");
        return std::ptr::null_mut();
    };

    let record: ActivityRecord = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_yoke_activity_record_with_context: {e}"));
            return std::ptr::null_mut();
        }
    };

    let record = record.with_context(ctx);
    json_to_c(&record)
}

/// Set the community on an ActivityRecord.
///
/// Returns modified ActivityRecord JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_activity_record_in_community(
    record_json: *const c_char,
    community_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(rj) = c_str_to_str(record_json) else {
        set_last_error("divi_yoke_activity_record_in_community: invalid record_json");
        return std::ptr::null_mut();
    };
    let Some(cid) = c_str_to_str(community_id) else {
        set_last_error("divi_yoke_activity_record_in_community: invalid community_id");
        return std::ptr::null_mut();
    };

    let record: ActivityRecord = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_yoke_activity_record_in_community: {e}"));
            return std::ptr::null_mut();
        }
    };

    let record = record.in_community(cid);
    json_to_c(&record)
}

// ===================================================================
// Milestone (4)
// ===================================================================

/// Create a new Milestone.
///
/// `significance_json` is a JSON MilestoneSignificance (e.g. `"Major"`).
/// Returns JSON (Milestone). Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_milestone_new(
    name: *const c_char,
    significance_json: *const c_char,
    author: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(nm) = c_str_to_str(name) else {
        set_last_error("divi_yoke_milestone_new: invalid name");
        return std::ptr::null_mut();
    };
    let Some(sig_str) = c_str_to_str(significance_json) else {
        set_last_error("divi_yoke_milestone_new: invalid significance_json");
        return std::ptr::null_mut();
    };
    let Some(auth) = c_str_to_str(author) else {
        set_last_error("divi_yoke_milestone_new: invalid author");
        return std::ptr::null_mut();
    };

    let significance: MilestoneSignificance = match serde_json::from_str(sig_str) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_yoke_milestone_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let milestone = Milestone::new(nm, significance, auth);
    json_to_c(&milestone)
}

/// Add a description to a Milestone.
///
/// Returns modified Milestone JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_milestone_with_description(
    milestone_json: *const c_char,
    description: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(mj) = c_str_to_str(milestone_json) else {
        set_last_error("divi_yoke_milestone_with_description: invalid milestone_json");
        return std::ptr::null_mut();
    };
    let Some(desc) = c_str_to_str(description) else {
        set_last_error("divi_yoke_milestone_with_description: invalid description");
        return std::ptr::null_mut();
    };

    let milestone: Milestone = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_yoke_milestone_with_description: {e}"));
            return std::ptr::null_mut();
        }
    };

    let milestone = milestone.with_description(desc);
    json_to_c(&milestone)
}

/// Set the community on a Milestone.
///
/// Returns modified Milestone JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_milestone_in_community(
    milestone_json: *const c_char,
    community_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(mj) = c_str_to_str(milestone_json) else {
        set_last_error("divi_yoke_milestone_in_community: invalid milestone_json");
        return std::ptr::null_mut();
    };
    let Some(cid) = c_str_to_str(community_id) else {
        set_last_error("divi_yoke_milestone_in_community: invalid community_id");
        return std::ptr::null_mut();
    };

    let milestone: Milestone = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_yoke_milestone_in_community: {e}"));
            return std::ptr::null_mut();
        }
    };

    let milestone = milestone.in_community(cid);
    json_to_c(&milestone)
}

/// Add a related event to a Milestone.
///
/// Returns modified Milestone JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_milestone_with_related_event(
    milestone_json: *const c_char,
    event_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(mj) = c_str_to_str(milestone_json) else {
        set_last_error("divi_yoke_milestone_with_related_event: invalid milestone_json");
        return std::ptr::null_mut();
    };
    let Some(eid) = c_str_to_str(event_id) else {
        set_last_error("divi_yoke_milestone_with_related_event: invalid event_id");
        return std::ptr::null_mut();
    };

    let milestone: Milestone = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_yoke_milestone_with_related_event: {e}"));
            return std::ptr::null_mut();
        }
    };

    let milestone = milestone.with_related_event(eid);
    json_to_c(&milestone)
}

// ===================================================================
// Timeline (13)
// ===================================================================

/// Create a new empty Timeline.
///
/// Returns JSON (Timeline). Caller must free via `divi_free_string`.
///
/// # Safety
/// `owner_id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_timeline_new(
    owner_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(oid) = c_str_to_str(owner_id) else {
        set_last_error("divi_yoke_timeline_new: invalid owner_id");
        return std::ptr::null_mut();
    };

    let timeline = Timeline::new(oid);
    json_to_c(&timeline)
}

/// Set the config on a Timeline.
///
/// `config_json` is a JSON TimelineConfig.
/// Returns modified Timeline JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_timeline_with_config(
    timeline_json: *const c_char,
    config_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(timeline_json) else {
        set_last_error("divi_yoke_timeline_with_config: invalid timeline_json");
        return std::ptr::null_mut();
    };
    let Some(cj) = c_str_to_str(config_json) else {
        set_last_error("divi_yoke_timeline_with_config: invalid config_json");
        return std::ptr::null_mut();
    };

    let timeline: Timeline = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_with_config: {e}"));
            return std::ptr::null_mut();
        }
    };

    let config: TimelineConfig = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_with_config: {e}"));
            return std::ptr::null_mut();
        }
    };

    let timeline = timeline.with_config(config);
    json_to_c(&timeline)
}

/// Record an activity on a Timeline.
///
/// `activity_json` is a JSON ActivityRecord.
/// Returns modified Timeline JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_timeline_record(
    timeline_json: *const c_char,
    activity_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(timeline_json) else {
        set_last_error("divi_yoke_timeline_record: invalid timeline_json");
        return std::ptr::null_mut();
    };
    let Some(aj) = c_str_to_str(activity_json) else {
        set_last_error("divi_yoke_timeline_record: invalid activity_json");
        return std::ptr::null_mut();
    };

    let mut timeline: Timeline = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_record: {e}"));
            return std::ptr::null_mut();
        }
    };

    let activity: ActivityRecord = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_record: {e}"));
            return std::ptr::null_mut();
        }
    };

    timeline.record(activity);
    json_to_c(&timeline)
}

/// Mark a milestone on a Timeline.
///
/// `milestone_json` is a JSON Milestone.
/// Returns modified Timeline JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_timeline_mark_milestone(
    timeline_json: *const c_char,
    milestone_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(timeline_json) else {
        set_last_error("divi_yoke_timeline_mark_milestone: invalid timeline_json");
        return std::ptr::null_mut();
    };
    let Some(mj) = c_str_to_str(milestone_json) else {
        set_last_error("divi_yoke_timeline_mark_milestone: invalid milestone_json");
        return std::ptr::null_mut();
    };

    let mut timeline: Timeline = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_mark_milestone: {e}"));
            return std::ptr::null_mut();
        }
    };

    let milestone: Milestone = match serde_json::from_str(mj) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_mark_milestone: {e}"));
            return std::ptr::null_mut();
        }
    };

    timeline.mark_milestone(milestone);
    json_to_c(&timeline)
}

/// Prune activities older than a cutoff timestamp.
///
/// `cutoff` is a Unix timestamp (seconds since epoch).
/// Returns modified Timeline JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// `timeline_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_timeline_prune_before(
    timeline_json: *const c_char,
    cutoff: i64,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(timeline_json) else {
        set_last_error("divi_yoke_timeline_prune_before: invalid timeline_json");
        return std::ptr::null_mut();
    };

    let mut timeline: Timeline = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_prune_before: {e}"));
            return std::ptr::null_mut();
        }
    };

    let cutoff_dt = DateTime::from_timestamp(cutoff, 0).unwrap_or_default();
    timeline.prune_before(cutoff_dt);
    json_to_c(&timeline)
}

/// Query activities by actor.
///
/// Returns JSON array of ActivityRecord. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_timeline_by_actor(
    timeline_json: *const c_char,
    actor: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(timeline_json) else {
        set_last_error("divi_yoke_timeline_by_actor: invalid timeline_json");
        return std::ptr::null_mut();
    };
    let Some(act) = c_str_to_str(actor) else {
        set_last_error("divi_yoke_timeline_by_actor: invalid actor");
        return std::ptr::null_mut();
    };

    let timeline: Timeline = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_by_actor: {e}"));
            return std::ptr::null_mut();
        }
    };

    let results: Vec<&ActivityRecord> = timeline.by_actor(act);
    json_to_c(&results)
}

/// Query activities by action type.
///
/// `action_json` is a JSON ActivityAction (e.g. `"Created"`).
/// Returns JSON array of ActivityRecord. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_timeline_by_action(
    timeline_json: *const c_char,
    action_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(timeline_json) else {
        set_last_error("divi_yoke_timeline_by_action: invalid timeline_json");
        return std::ptr::null_mut();
    };
    let Some(aj) = c_str_to_str(action_json) else {
        set_last_error("divi_yoke_timeline_by_action: invalid action_json");
        return std::ptr::null_mut();
    };

    let timeline: Timeline = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_by_action: {e}"));
            return std::ptr::null_mut();
        }
    };

    let action: ActivityAction = match serde_json::from_str(aj) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_by_action: {e}"));
            return std::ptr::null_mut();
        }
    };

    let results: Vec<&ActivityRecord> = timeline.by_action(&action);
    json_to_c(&results)
}

/// Query activities targeting a specific entity.
///
/// Returns JSON array of ActivityRecord. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_timeline_for_target(
    timeline_json: *const c_char,
    target_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(timeline_json) else {
        set_last_error("divi_yoke_timeline_for_target: invalid timeline_json");
        return std::ptr::null_mut();
    };
    let Some(tid) = c_str_to_str(target_id) else {
        set_last_error("divi_yoke_timeline_for_target: invalid target_id");
        return std::ptr::null_mut();
    };

    let timeline: Timeline = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_for_target: {e}"));
            return std::ptr::null_mut();
        }
    };

    let results: Vec<&ActivityRecord> = timeline.for_target(tid);
    json_to_c(&results)
}

/// Query activities in a specific community.
///
/// Returns JSON array of ActivityRecord. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_timeline_in_community(
    timeline_json: *const c_char,
    community_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(timeline_json) else {
        set_last_error("divi_yoke_timeline_in_community: invalid timeline_json");
        return std::ptr::null_mut();
    };
    let Some(cid) = c_str_to_str(community_id) else {
        set_last_error("divi_yoke_timeline_in_community: invalid community_id");
        return std::ptr::null_mut();
    };

    let timeline: Timeline = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_in_community: {e}"));
            return std::ptr::null_mut();
        }
    };

    let results: Vec<&ActivityRecord> = timeline.in_community(cid);
    json_to_c(&results)
}

/// Query activities within a time range.
///
/// `since` and `until` are Unix timestamps (seconds since epoch).
/// Returns JSON array of ActivityRecord. Caller must free via `divi_free_string`.
///
/// # Safety
/// `timeline_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_timeline_between(
    timeline_json: *const c_char,
    since: i64,
    until: i64,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(timeline_json) else {
        set_last_error("divi_yoke_timeline_between: invalid timeline_json");
        return std::ptr::null_mut();
    };

    let timeline: Timeline = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_between: {e}"));
            return std::ptr::null_mut();
        }
    };

    let since_dt = DateTime::from_timestamp(since, 0).unwrap_or_default();
    let until_dt = DateTime::from_timestamp(until, 0).unwrap_or_default();
    let results: Vec<&ActivityRecord> = timeline.between(since_dt, until_dt);
    json_to_c(&results)
}

/// Query milestones at or above a given significance.
///
/// `significance_json` is a JSON MilestoneSignificance (e.g. `"Major"`).
/// Returns JSON array of Milestone. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_timeline_milestones_at_least(
    timeline_json: *const c_char,
    significance_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(tj) = c_str_to_str(timeline_json) else {
        set_last_error("divi_yoke_timeline_milestones_at_least: invalid timeline_json");
        return std::ptr::null_mut();
    };
    let Some(sj) = c_str_to_str(significance_json) else {
        set_last_error("divi_yoke_timeline_milestones_at_least: invalid significance_json");
        return std::ptr::null_mut();
    };

    let timeline: Timeline = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_milestones_at_least: {e}"));
            return std::ptr::null_mut();
        }
    };

    let significance: MilestoneSignificance = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_milestones_at_least: {e}"));
            return std::ptr::null_mut();
        }
    };

    let results: Vec<&Milestone> = timeline.milestones_at_least(significance);
    json_to_c(&results)
}

/// Get the total number of activities in a Timeline.
///
/// Returns count on success, -1 on error.
///
/// # Safety
/// `timeline_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_timeline_activity_count(
    timeline_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(tj) = c_str_to_str(timeline_json) else {
        set_last_error("divi_yoke_timeline_activity_count: invalid timeline_json");
        return -1;
    };

    let timeline: Timeline = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_activity_count: {e}"));
            return -1;
        }
    };

    timeline.activity_count() as i32
}

/// Get the total number of milestones in a Timeline.
///
/// Returns count on success, -1 on error.
///
/// # Safety
/// `timeline_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_timeline_milestone_count(
    timeline_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(tj) = c_str_to_str(timeline_json) else {
        set_last_error("divi_yoke_timeline_milestone_count: invalid timeline_json");
        return -1;
    };

    let timeline: Timeline = match serde_json::from_str(tj) {
        Ok(t) => t,
        Err(e) => {
            set_last_error(format!("divi_yoke_timeline_milestone_count: {e}"));
            return -1;
        }
    };

    timeline.milestone_count() as i32
}

// ===================================================================
// CeremonyRecord (13)
// ===================================================================

/// Create a new CeremonyRecord.
///
/// `ceremony_type_json` is a JSON CeremonyType (e.g. `"CovenantOath"` or
/// `{"Custom":"graduation"}`).
/// Returns JSON (CeremonyRecord). Caller must free via `divi_free_string`.
///
/// # Safety
/// `ceremony_type_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_ceremony_new(
    ceremony_type_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(ct_str) = c_str_to_str(ceremony_type_json) else {
        set_last_error("divi_yoke_ceremony_new: invalid ceremony_type_json");
        return std::ptr::null_mut();
    };

    let ceremony_type: CeremonyType = match serde_json::from_str(ct_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_ceremony_new: {e}"));
            return std::ptr::null_mut();
        }
    };

    let ceremony = CeremonyRecord::new(ceremony_type);
    json_to_c(&ceremony)
}

/// Add a principal to a CeremonyRecord.
///
/// Returns modified CeremonyRecord JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_ceremony_with_principal(
    ceremony_json: *const c_char,
    crown_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(ceremony_json) else {
        set_last_error("divi_yoke_ceremony_with_principal: invalid ceremony_json");
        return std::ptr::null_mut();
    };
    let Some(crown_id_str) = c_str_to_str(crown_id) else {
        set_last_error("divi_yoke_ceremony_with_principal: invalid crown_id");
        return std::ptr::null_mut();
    };

    let ceremony: CeremonyRecord = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_ceremony_with_principal: {e}"));
            return std::ptr::null_mut();
        }
    };

    let ceremony = ceremony.with_principal(crown_id_str);
    json_to_c(&ceremony)
}

/// Add a witness to a CeremonyRecord.
///
/// Returns modified CeremonyRecord JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_ceremony_with_witness(
    ceremony_json: *const c_char,
    crown_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(ceremony_json) else {
        set_last_error("divi_yoke_ceremony_with_witness: invalid ceremony_json");
        return std::ptr::null_mut();
    };
    let Some(crown_id_str) = c_str_to_str(crown_id) else {
        set_last_error("divi_yoke_ceremony_with_witness: invalid crown_id");
        return std::ptr::null_mut();
    };

    let ceremony: CeremonyRecord = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_ceremony_with_witness: {e}"));
            return std::ptr::null_mut();
        }
    };

    let ceremony = ceremony.with_witness(crown_id_str);
    json_to_c(&ceremony)
}

/// Add an officiant to a CeremonyRecord.
///
/// Returns modified CeremonyRecord JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_ceremony_with_officiant(
    ceremony_json: *const c_char,
    crown_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(ceremony_json) else {
        set_last_error("divi_yoke_ceremony_with_officiant: invalid ceremony_json");
        return std::ptr::null_mut();
    };
    let Some(crown_id_str) = c_str_to_str(crown_id) else {
        set_last_error("divi_yoke_ceremony_with_officiant: invalid crown_id");
        return std::ptr::null_mut();
    };

    let ceremony: CeremonyRecord = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_ceremony_with_officiant: {e}"));
            return std::ptr::null_mut();
        }
    };

    let ceremony = ceremony.with_officiant(crown_id_str);
    json_to_c(&ceremony)
}

/// Add a participant with a custom role to a CeremonyRecord.
///
/// `role_json` is a JSON ParticipantRole (e.g. `"Witness"` or `{"Custom":"mentor"}`).
/// Returns modified CeremonyRecord JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_ceremony_with_participant(
    ceremony_json: *const c_char,
    crown_id: *const c_char,
    role_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(ceremony_json) else {
        set_last_error("divi_yoke_ceremony_with_participant: invalid ceremony_json");
        return std::ptr::null_mut();
    };
    let Some(crown_id_str) = c_str_to_str(crown_id) else {
        set_last_error("divi_yoke_ceremony_with_participant: invalid crown_id");
        return std::ptr::null_mut();
    };
    let Some(role_str) = c_str_to_str(role_json) else {
        set_last_error("divi_yoke_ceremony_with_participant: invalid role_json");
        return std::ptr::null_mut();
    };

    let ceremony: CeremonyRecord = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_ceremony_with_participant: {e}"));
            return std::ptr::null_mut();
        }
    };

    let role: ParticipantRole = match serde_json::from_str(role_str) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_yoke_ceremony_with_participant: {e}"));
            return std::ptr::null_mut();
        }
    };

    let ceremony = ceremony.with_participant(crown_id_str, role);
    json_to_c(&ceremony)
}

/// Set the community on a CeremonyRecord.
///
/// Returns modified CeremonyRecord JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_ceremony_in_community(
    ceremony_json: *const c_char,
    community_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(ceremony_json) else {
        set_last_error("divi_yoke_ceremony_in_community: invalid ceremony_json");
        return std::ptr::null_mut();
    };
    let Some(cid) = c_str_to_str(community_id) else {
        set_last_error("divi_yoke_ceremony_in_community: invalid community_id");
        return std::ptr::null_mut();
    };

    let ceremony: CeremonyRecord = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_ceremony_in_community: {e}"));
            return std::ptr::null_mut();
        }
    };

    let ceremony = ceremony.in_community(cid);
    json_to_c(&ceremony)
}

/// Set the content on a CeremonyRecord (oath text, charter, etc.).
///
/// Returns modified CeremonyRecord JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_ceremony_with_content(
    ceremony_json: *const c_char,
    content: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(ceremony_json) else {
        set_last_error("divi_yoke_ceremony_with_content: invalid ceremony_json");
        return std::ptr::null_mut();
    };
    let Some(cnt) = c_str_to_str(content) else {
        set_last_error("divi_yoke_ceremony_with_content: invalid content");
        return std::ptr::null_mut();
    };

    let ceremony: CeremonyRecord = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_ceremony_with_content: {e}"));
            return std::ptr::null_mut();
        }
    };

    let ceremony = ceremony.with_content(cnt);
    json_to_c(&ceremony)
}

/// Add a related event to a CeremonyRecord.
///
/// Returns modified CeremonyRecord JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_ceremony_with_related_event(
    ceremony_json: *const c_char,
    event_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(ceremony_json) else {
        set_last_error("divi_yoke_ceremony_with_related_event: invalid ceremony_json");
        return std::ptr::null_mut();
    };
    let Some(eid) = c_str_to_str(event_id) else {
        set_last_error("divi_yoke_ceremony_with_related_event: invalid event_id");
        return std::ptr::null_mut();
    };

    let ceremony: CeremonyRecord = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_ceremony_with_related_event: {e}"));
            return std::ptr::null_mut();
        }
    };

    let ceremony = ceremony.with_related_event(eid);
    json_to_c(&ceremony)
}

/// Validate a CeremonyRecord against its structural rules.
///
/// Returns 0 on success, -1 on validation failure (check `divi_last_error`).
///
/// # Safety
/// `ceremony_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_ceremony_validate(
    ceremony_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(cj) = c_str_to_str(ceremony_json) else {
        set_last_error("divi_yoke_ceremony_validate: invalid ceremony_json");
        return -1;
    };

    let ceremony: CeremonyRecord = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_ceremony_validate: {e}"));
            return -1;
        }
    };

    match ceremony.validate() {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Get all principals in a CeremonyRecord.
///
/// Returns JSON array of strings. Caller must free via `divi_free_string`.
///
/// # Safety
/// `ceremony_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_ceremony_principals(
    ceremony_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(ceremony_json) else {
        set_last_error("divi_yoke_ceremony_principals: invalid ceremony_json");
        return std::ptr::null_mut();
    };

    let ceremony: CeremonyRecord = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_ceremony_principals: {e}"));
            return std::ptr::null_mut();
        }
    };

    let principals: Vec<&str> = ceremony.principals();
    json_to_c(&principals)
}

/// Get all witnesses in a CeremonyRecord.
///
/// Returns JSON array of strings. Caller must free via `divi_free_string`.
///
/// # Safety
/// `ceremony_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_ceremony_witnesses(
    ceremony_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(ceremony_json) else {
        set_last_error("divi_yoke_ceremony_witnesses: invalid ceremony_json");
        return std::ptr::null_mut();
    };

    let ceremony: CeremonyRecord = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_ceremony_witnesses: {e}"));
            return std::ptr::null_mut();
        }
    };

    let witnesses: Vec<&str> = ceremony.witnesses();
    json_to_c(&witnesses)
}

/// Get all officiants in a CeremonyRecord.
///
/// Returns JSON array of strings. Caller must free via `divi_free_string`.
///
/// # Safety
/// `ceremony_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_ceremony_officiants(
    ceremony_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(cj) = c_str_to_str(ceremony_json) else {
        set_last_error("divi_yoke_ceremony_officiants: invalid ceremony_json");
        return std::ptr::null_mut();
    };

    let ceremony: CeremonyRecord = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_ceremony_officiants: {e}"));
            return std::ptr::null_mut();
        }
    };

    let officiants: Vec<&str> = ceremony.officiants();
    json_to_c(&officiants)
}

/// Get the total participant count of a CeremonyRecord.
///
/// Returns count on success, -1 on error.
///
/// # Safety
/// `ceremony_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_ceremony_participant_count(
    ceremony_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(cj) = c_str_to_str(ceremony_json) else {
        set_last_error("divi_yoke_ceremony_participant_count: invalid ceremony_json");
        return -1;
    };

    let ceremony: CeremonyRecord = match serde_json::from_str(cj) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_yoke_ceremony_participant_count: {e}"));
            return -1;
        }
    };

    ceremony.participant_count() as i32
}

// ===================================================================
// RelationshipGraph via GraphSnapshot (13)
// ===================================================================

/// Create a new empty RelationshipGraph (as a GraphSnapshot).
///
/// Returns JSON (GraphSnapshot). Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_yoke_graph_new() -> *mut c_char {
    clear_last_error();
    let graph = RelationshipGraph::new();
    graph_to_json(&graph)
}

/// Add a link to a RelationshipGraph.
///
/// `graph_json` is a JSON GraphSnapshot. `link_json` is a JSON YokeLink.
/// Returns modified GraphSnapshot JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_graph_add_link(
    graph_json: *const c_char,
    link_json: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(gj) = c_str_to_str(graph_json) else {
        set_last_error("divi_yoke_graph_add_link: invalid graph_json");
        return std::ptr::null_mut();
    };
    let Some(lj) = c_str_to_str(link_json) else {
        set_last_error("divi_yoke_graph_add_link: invalid link_json");
        return std::ptr::null_mut();
    };

    let Some(mut graph) = graph_from_json(gj, "divi_yoke_graph_add_link") else {
        return std::ptr::null_mut();
    };

    let link: YokeLink = match serde_json::from_str(lj) {
        Ok(l) => l,
        Err(e) => {
            set_last_error(format!("divi_yoke_graph_add_link: {e}"));
            return std::ptr::null_mut();
        }
    };

    graph.add_link(link);
    graph_to_json(&graph)
}

/// Remove all links involving an entity from the graph.
///
/// Returns modified GraphSnapshot JSON. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_graph_remove_entity(
    graph_json: *const c_char,
    entity_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(gj) = c_str_to_str(graph_json) else {
        set_last_error("divi_yoke_graph_remove_entity: invalid graph_json");
        return std::ptr::null_mut();
    };
    let Some(eid) = c_str_to_str(entity_id) else {
        set_last_error("divi_yoke_graph_remove_entity: invalid entity_id");
        return std::ptr::null_mut();
    };

    let Some(mut graph) = graph_from_json(gj, "divi_yoke_graph_remove_entity") else {
        return std::ptr::null_mut();
    };

    graph.remove_entity(eid);
    graph_to_json(&graph)
}

/// Get all links originating from an entity.
///
/// Returns JSON array of YokeLink. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_graph_links_from(
    graph_json: *const c_char,
    source: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(gj) = c_str_to_str(graph_json) else {
        set_last_error("divi_yoke_graph_links_from: invalid graph_json");
        return std::ptr::null_mut();
    };
    let Some(src) = c_str_to_str(source) else {
        set_last_error("divi_yoke_graph_links_from: invalid source");
        return std::ptr::null_mut();
    };

    let Some(graph) = graph_from_json(gj, "divi_yoke_graph_links_from") else {
        return std::ptr::null_mut();
    };

    let links: Vec<&YokeLink> = graph.links_from(src);
    json_to_c(&links)
}

/// Get all links pointing to an entity.
///
/// Returns JSON array of YokeLink. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_graph_links_to(
    graph_json: *const c_char,
    target: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(gj) = c_str_to_str(graph_json) else {
        set_last_error("divi_yoke_graph_links_to: invalid graph_json");
        return std::ptr::null_mut();
    };
    let Some(tgt) = c_str_to_str(target) else {
        set_last_error("divi_yoke_graph_links_to: invalid target");
        return std::ptr::null_mut();
    };

    let Some(graph) = graph_from_json(gj, "divi_yoke_graph_links_to") else {
        return std::ptr::null_mut();
    };

    let links: Vec<&YokeLink> = graph.links_to(tgt);
    json_to_c(&links)
}

/// Find all ancestors via provenance links (DerivedFrom, VersionOf, etc.).
///
/// Returns JSON array of TraversalNodeJson. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_graph_ancestors(
    graph_json: *const c_char,
    entity_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(gj) = c_str_to_str(graph_json) else {
        set_last_error("divi_yoke_graph_ancestors: invalid graph_json");
        return std::ptr::null_mut();
    };
    let Some(eid) = c_str_to_str(entity_id) else {
        set_last_error("divi_yoke_graph_ancestors: invalid entity_id");
        return std::ptr::null_mut();
    };

    let Some(graph) = graph_from_json(gj, "divi_yoke_graph_ancestors") else {
        return std::ptr::null_mut();
    };

    let nodes: Vec<TraversalNodeJson> =
        graph.ancestors(eid).into_iter().map(Into::into).collect();
    json_to_c(&nodes)
}

/// Find all descendants via provenance links.
///
/// Returns JSON array of TraversalNodeJson. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_graph_descendants(
    graph_json: *const c_char,
    entity_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(gj) = c_str_to_str(graph_json) else {
        set_last_error("divi_yoke_graph_descendants: invalid graph_json");
        return std::ptr::null_mut();
    };
    let Some(eid) = c_str_to_str(entity_id) else {
        set_last_error("divi_yoke_graph_descendants: invalid entity_id");
        return std::ptr::null_mut();
    };

    let Some(graph) = graph_from_json(gj, "divi_yoke_graph_descendants") else {
        return std::ptr::null_mut();
    };

    let nodes: Vec<TraversalNodeJson> =
        graph.descendants(eid).into_iter().map(Into::into).collect();
    json_to_c(&nodes)
}

/// Get all comments on an entity.
///
/// Returns JSON array of YokeLink. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_graph_comments_on(
    graph_json: *const c_char,
    entity_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(gj) = c_str_to_str(graph_json) else {
        set_last_error("divi_yoke_graph_comments_on: invalid graph_json");
        return std::ptr::null_mut();
    };
    let Some(eid) = c_str_to_str(entity_id) else {
        set_last_error("divi_yoke_graph_comments_on: invalid entity_id");
        return std::ptr::null_mut();
    };

    let Some(graph) = graph_from_json(gj, "divi_yoke_graph_comments_on") else {
        return std::ptr::null_mut();
    };

    let links: Vec<&YokeLink> = graph.comments_on(eid);
    json_to_c(&links)
}

/// Get all version-of links pointing to an entity.
///
/// Returns JSON array of YokeLink. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_graph_versions_of(
    graph_json: *const c_char,
    entity_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(gj) = c_str_to_str(graph_json) else {
        set_last_error("divi_yoke_graph_versions_of: invalid graph_json");
        return std::ptr::null_mut();
    };
    let Some(eid) = c_str_to_str(entity_id) else {
        set_last_error("divi_yoke_graph_versions_of: invalid entity_id");
        return std::ptr::null_mut();
    };

    let Some(graph) = graph_from_json(gj, "divi_yoke_graph_versions_of") else {
        return std::ptr::null_mut();
    };

    let links: Vec<&YokeLink> = graph.versions_of(eid);
    json_to_c(&links)
}

/// Get all endorsements of an entity.
///
/// Returns JSON array of YokeLink. Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_graph_endorsements_of(
    graph_json: *const c_char,
    entity_id: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(gj) = c_str_to_str(graph_json) else {
        set_last_error("divi_yoke_graph_endorsements_of: invalid graph_json");
        return std::ptr::null_mut();
    };
    let Some(eid) = c_str_to_str(entity_id) else {
        set_last_error("divi_yoke_graph_endorsements_of: invalid entity_id");
        return std::ptr::null_mut();
    };

    let Some(graph) = graph_from_json(gj, "divi_yoke_graph_endorsements_of") else {
        return std::ptr::null_mut();
    };

    let links: Vec<&YokeLink> = graph.endorsements_of(eid);
    json_to_c(&links)
}

/// Find the shortest path between two entities in the graph.
///
/// Returns JSON array of strings (entity IDs) or null if no path exists.
/// Caller must free via `divi_free_string`.
///
/// # Safety
/// All C strings must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_graph_path_between(
    graph_json: *const c_char,
    from: *const c_char,
    to: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(gj) = c_str_to_str(graph_json) else {
        set_last_error("divi_yoke_graph_path_between: invalid graph_json");
        return std::ptr::null_mut();
    };
    let Some(from_str) = c_str_to_str(from) else {
        set_last_error("divi_yoke_graph_path_between: invalid from");
        return std::ptr::null_mut();
    };
    let Some(to_str) = c_str_to_str(to) else {
        set_last_error("divi_yoke_graph_path_between: invalid to");
        return std::ptr::null_mut();
    };

    let Some(graph) = graph_from_json(gj, "divi_yoke_graph_path_between") else {
        return std::ptr::null_mut();
    };

    match graph.path_between(from_str, to_str) {
        Some(path) => json_to_c(&path),
        None => std::ptr::null_mut(),
    }
}

/// Get the total number of links in the graph.
///
/// Returns count on success, -1 on error.
///
/// # Safety
/// `graph_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_graph_link_count(
    graph_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(gj) = c_str_to_str(graph_json) else {
        set_last_error("divi_yoke_graph_link_count: invalid graph_json");
        return -1;
    };

    let Some(graph) = graph_from_json(gj, "divi_yoke_graph_link_count") else {
        return -1;
    };

    graph.link_count() as i32
}

/// Get the total number of unique entities in the graph.
///
/// Returns count on success, -1 on error.
///
/// # Safety
/// `graph_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_yoke_graph_entity_count(
    graph_json: *const c_char,
) -> i32 {
    clear_last_error();

    let Some(gj) = c_str_to_str(graph_json) else {
        set_last_error("divi_yoke_graph_entity_count: invalid graph_json");
        return -1;
    };

    let Some(graph) = graph_from_json(gj, "divi_yoke_graph_entity_count") else {
        return -1;
    };

    graph.entity_count() as i32
}
