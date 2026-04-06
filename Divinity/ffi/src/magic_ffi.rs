//! FFI bindings for the Magic crate — rendering, canvas, tools, history,
//! projection, and document state.
//!
//! Exposes Magic's Triple-I architecture (Ideation, Imagination, Initiation)
//! plus Projection, Canvas, and Tool subsystems to any language that speaks
//! C ABI. All functions use the `divi_magic_` prefix.

use std::ffi::c_char;
use std::sync::Mutex;

use ideas::Digit;
use uuid::Uuid;

use magic::{
    // Ideation
    AccessibilityRole, AccessibilitySpec, CanvasState, DocumentHistory, DocumentState,
    DigitTypeRegistry, SlideSequenceState,
    // Imagination
    RenderCache, RenderContext, RenderMode, RendererRegistry, register_all_renderers,
    // Initiation
    Action, HistoryEntry,
    // Projection
    CodeProjection, HtmlProjection, ProjectionContext, ReactProjection, SwiftUIProjection,
    // Tool
    ModifierKeys, ToolRegistry, default_tool_registry,
    // Canvas
    compute_handles, hit_test_handles,
};
use regalia::Reign;
use x::geometry::{Point, Rect};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover, string_to_c};
use crate::{clear_last_error, set_last_error};

// ===========================================================================
// Thread-safe opaque pointer wrappers
// ===========================================================================

pub struct MagicDocumentState(pub(crate) Mutex<DocumentState>);
pub struct MagicCanvasState(pub(crate) Mutex<CanvasState>);
pub struct MagicRendererRegistry(pub(crate) Mutex<RendererRegistry>);
pub struct MagicToolRegistry(pub(crate) Mutex<ToolRegistry>);
pub struct MagicDocumentHistory(pub(crate) Mutex<DocumentHistory>);
pub struct MagicRenderCache(pub(crate) Mutex<RenderCache>);

// ===========================================================================
// DocumentState — Ideation (single source of truth)
// ===========================================================================

/// Create a new empty document state with the given author identifier.
///
/// Free with `divi_magic_document_free`.
///
/// # Safety
/// `author` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_new(author: *const c_char) -> *mut MagicDocumentState {
    let author_str = c_str_to_str(author).unwrap_or("unknown");
    Box::into_raw(Box::new(MagicDocumentState(Mutex::new(
        DocumentState::new(author_str),
    ))))
}

/// Free a document state.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_magic_document_new`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_free(ptr: *mut MagicDocumentState) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Returns the number of digits in the document.
///
/// # Safety
/// `doc` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_digit_count(
    doc: *const MagicDocumentState,
) -> usize {
    let doc = unsafe { &*doc };
    let guard = lock_or_recover(&doc.0);
    guard.digit_count()
}

/// Returns the root digit ID as a UUID string, or null if none.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `doc` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_root_id(
    doc: *const MagicDocumentState,
) -> *mut c_char {
    let doc = unsafe { &*doc };
    let guard = lock_or_recover(&doc.0);
    match guard.root_digit_id() {
        Some(id) => string_to_c(id.to_string()),
        None => std::ptr::null_mut(),
    }
}

/// Returns a single digit as JSON, or null if not found.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `doc` must be a valid pointer. `id` must be a valid C string (UUID).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_digit(
    doc: *const MagicDocumentState,
    id: *const c_char,
) -> *mut c_char {
    let doc = unsafe { &*doc };
    let Some(id_str) = c_str_to_str(id) else {
        return std::ptr::null_mut();
    };
    let Ok(uuid) = Uuid::parse_str(id_str) else {
        return std::ptr::null_mut();
    };
    let guard = lock_or_recover(&doc.0);
    match guard.digit(uuid) {
        Some(digit) => json_to_c(digit),
        None => std::ptr::null_mut(),
    }
}

/// Returns all digits as a JSON array.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `doc` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_all_digits(
    doc: *const MagicDocumentState,
) -> *mut c_char {
    let doc = unsafe { &*doc };
    let guard = lock_or_recover(&doc.0);
    let digits: Vec<&Digit> = guard.digits().collect();
    json_to_c(&digits)
}

/// Returns children of a digit as a JSON array.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `doc` must be a valid pointer. `parent_id` must be a valid C string (UUID).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_children_of(
    doc: *const MagicDocumentState,
    parent_id: *const c_char,
) -> *mut c_char {
    let doc = unsafe { &*doc };
    let Some(id_str) = c_str_to_str(parent_id) else {
        return json_to_c(&Vec::<Digit>::new());
    };
    let Ok(uuid) = Uuid::parse_str(id_str) else {
        return json_to_c(&Vec::<Digit>::new());
    };
    let guard = lock_or_recover(&doc.0);
    let children = guard.children_of(uuid);
    json_to_c(&children)
}

/// Returns the document layout as JSON.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `doc` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_layout(
    doc: *const MagicDocumentState,
) -> *mut c_char {
    let doc = unsafe { &*doc };
    let guard = lock_or_recover(&doc.0);
    json_to_c(&guard.layout)
}

/// Returns the selection state as JSON.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `doc` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_selection(
    doc: *const MagicDocumentState,
) -> *mut c_char {
    let doc = unsafe { &*doc };
    let guard = lock_or_recover(&doc.0);
    json_to_c(&guard.selection)
}

/// Returns the current author identifier as a string.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `doc` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_author(
    doc: *const MagicDocumentState,
) -> *mut c_char {
    let doc = unsafe { &*doc };
    let guard = lock_or_recover(&doc.0);
    string_to_c(guard.author().to_string())
}

/// Insert a digit into the document.
///
/// Returns the generated operation as JSON, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `doc` must be a valid pointer. `digit_json` and (optionally) `parent_id`
/// must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_insert(
    doc: *const MagicDocumentState,
    digit_json: *const c_char,
    parent_id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let doc = unsafe { &*doc };
    let Some(json_str) = c_str_to_str(digit_json) else {
        set_last_error("divi_magic_document_insert: invalid digit_json");
        return std::ptr::null_mut();
    };
    let digit: Digit = match serde_json::from_str(json_str) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_magic_document_insert: {e}"));
            return std::ptr::null_mut();
        }
    };
    let pid = c_str_to_str(parent_id).and_then(|s| Uuid::parse_str(s).ok());

    let mut guard = lock_or_recover(&doc.0);
    match guard.insert_digit(digit, pid) {
        Ok(op) => json_to_c(&op),
        Err(e) => {
            set_last_error(format!("divi_magic_document_insert: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Update a digit field.
///
/// Returns the generated operation as JSON, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `doc` must be a valid pointer. `id`, `field`, `old_json`, `new_json`
/// must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_update(
    doc: *const MagicDocumentState,
    id: *const c_char,
    field: *const c_char,
    old_json: *const c_char,
    new_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let doc = unsafe { &*doc };

    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_magic_document_update: invalid id");
        return std::ptr::null_mut();
    };
    let Ok(uuid) = Uuid::parse_str(id_str) else {
        set_last_error("divi_magic_document_update: invalid UUID");
        return std::ptr::null_mut();
    };
    let Some(field_str) = c_str_to_str(field) else {
        set_last_error("divi_magic_document_update: invalid field");
        return std::ptr::null_mut();
    };
    let Some(old_str) = c_str_to_str(old_json) else {
        set_last_error("divi_magic_document_update: invalid old_json");
        return std::ptr::null_mut();
    };
    let Some(new_str) = c_str_to_str(new_json) else {
        set_last_error("divi_magic_document_update: invalid new_json");
        return std::ptr::null_mut();
    };

    let old_value: x::Value = match serde_json::from_str(old_str) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_magic_document_update: old_json parse: {e}"));
            return std::ptr::null_mut();
        }
    };
    let new_value: x::Value = match serde_json::from_str(new_str) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(format!("divi_magic_document_update: new_json parse: {e}"));
            return std::ptr::null_mut();
        }
    };

    let mut guard = lock_or_recover(&doc.0);
    match guard.update_digit(uuid, field_str.to_string(), old_value, new_value) {
        Ok(op) => json_to_c(&op),
        Err(e) => {
            set_last_error(format!("divi_magic_document_update: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Delete a digit (tombstone soft-delete).
///
/// Returns the generated operation as JSON, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `doc` must be a valid pointer. `id` must be a valid C string (UUID).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_delete(
    doc: *const MagicDocumentState,
    id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let doc = unsafe { &*doc };
    let Some(id_str) = c_str_to_str(id) else {
        set_last_error("divi_magic_document_delete: invalid id");
        return std::ptr::null_mut();
    };
    let Ok(uuid) = Uuid::parse_str(id_str) else {
        set_last_error("divi_magic_document_delete: invalid UUID");
        return std::ptr::null_mut();
    };

    let mut guard = lock_or_recover(&doc.0);
    match guard.delete_digit(uuid) {
        Ok(op) => json_to_c(&op),
        Err(e) => {
            set_last_error(format!("divi_magic_document_delete: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Apply a DigitOperation (from JSON) to the document.
///
/// Returns 0 on success (applied), 1 if duplicate (already applied), -1 on error.
///
/// # Safety
/// `doc` must be a valid pointer. `operation_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_apply(
    doc: *const MagicDocumentState,
    operation_json: *const c_char,
) -> i32 {
    clear_last_error();
    let doc = unsafe { &*doc };
    let Some(json_str) = c_str_to_str(operation_json) else {
        set_last_error("divi_magic_document_apply: invalid operation_json");
        return -1;
    };
    let op = match serde_json::from_str(json_str) {
        Ok(o) => o,
        Err(e) => {
            set_last_error(format!("divi_magic_document_apply: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&doc.0);
    match guard.apply(&op) {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(e) => {
            set_last_error(format!("divi_magic_document_apply: {e}"));
            -1
        }
    }
}

/// Load a set of digits wholesale (replaces existing state).
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `doc` must be a valid pointer. `digits_json` must be a valid C string.
/// `root_id` may be null (no root).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_load_digits(
    doc: *const MagicDocumentState,
    digits_json: *const c_char,
    root_id: *const c_char,
) -> i32 {
    clear_last_error();
    let doc = unsafe { &*doc };
    let Some(json_str) = c_str_to_str(digits_json) else {
        set_last_error("divi_magic_document_load_digits: invalid digits_json");
        return -1;
    };
    let digits: Vec<Digit> = match serde_json::from_str(json_str) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_magic_document_load_digits: {e}"));
            return -1;
        }
    };
    let rid = c_str_to_str(root_id).and_then(|s| Uuid::parse_str(s).ok());

    let mut guard = lock_or_recover(&doc.0);
    guard.load_digits(digits, rid);
    0
}

/// Set the document layout from JSON.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `doc` must be a valid pointer. `layout_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_set_layout(
    doc: *const MagicDocumentState,
    layout_json: *const c_char,
) -> i32 {
    clear_last_error();
    let doc = unsafe { &*doc };
    let Some(json_str) = c_str_to_str(layout_json) else {
        set_last_error("divi_magic_document_set_layout: invalid layout_json");
        return -1;
    };
    let layout = match serde_json::from_str(json_str) {
        Ok(l) => l,
        Err(e) => {
            set_last_error(format!("divi_magic_document_set_layout: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&doc.0);
    guard.layout = layout;
    0
}

/// Select a digit in the document's selection state.
///
/// # Safety
/// `doc` must be a valid pointer. `id` must be a valid C string (UUID).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_select(
    doc: *const MagicDocumentState,
    id: *const c_char,
) {
    let doc = unsafe { &*doc };
    if let Some(id_str) = c_str_to_str(id) {
        if let Ok(uuid) = Uuid::parse_str(id_str) {
            let mut guard = lock_or_recover(&doc.0);
            guard.selection.select(uuid);
        }
    }
}

/// Deselect a digit from the document's selection state.
///
/// # Safety
/// `doc` must be a valid pointer. `id` must be a valid C string (UUID).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_deselect_digit(
    doc: *const MagicDocumentState,
    id: *const c_char,
) {
    let doc = unsafe { &*doc };
    if let Some(id_str) = c_str_to_str(id) {
        if let Ok(uuid) = Uuid::parse_str(id_str) {
            let mut guard = lock_or_recover(&doc.0);
            guard.selection.deselect(uuid);
        }
    }
}

/// Clear all selection in the document.
///
/// # Safety
/// `doc` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_document_clear_selection(
    doc: *const MagicDocumentState,
) {
    let doc = unsafe { &*doc };
    let mut guard = lock_or_recover(&doc.0);
    guard.selection.clear();
}

// ===========================================================================
// CanvasState — viewport, zoom, selection, snapping
// ===========================================================================

/// Create a new canvas state with default viewport (1024x768).
///
/// Free with `divi_magic_canvas_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_magic_canvas_new() -> *mut MagicCanvasState {
    Box::into_raw(Box::new(MagicCanvasState(Mutex::new(CanvasState::new()))))
}

/// Create a new canvas state with a specific viewport size.
///
/// Free with `divi_magic_canvas_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_magic_canvas_with_viewport(
    width: f64,
    height: f64,
) -> *mut MagicCanvasState {
    Box::into_raw(Box::new(MagicCanvasState(Mutex::new(
        CanvasState::with_viewport(width, height),
    ))))
}

/// Free a canvas state.
///
/// # Safety
/// `ptr` must be a valid pointer from a `divi_magic_canvas_*` constructor, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_free(ptr: *mut MagicCanvasState) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Convert a screen-space point to canvas-space.
///
/// Writes the result to `out_cx` and `out_cy`.
///
/// # Safety
/// `canvas` must be a valid pointer. `out_cx` and `out_cy` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_screen_to_canvas(
    canvas: *const MagicCanvasState,
    sx: f64,
    sy: f64,
    out_cx: *mut f64,
    out_cy: *mut f64,
) {
    let canvas = unsafe { &*canvas };
    let guard = lock_or_recover(&canvas.0);
    let result = guard.screen_to_canvas(Point::new(sx, sy));
    unsafe {
        *out_cx = result.x;
        *out_cy = result.y;
    }
}

/// Convert a canvas-space point to screen-space.
///
/// Writes the result to `out_sx` and `out_sy`.
///
/// # Safety
/// `canvas` must be a valid pointer. `out_sx` and `out_sy` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_canvas_to_screen(
    canvas: *const MagicCanvasState,
    cx: f64,
    cy: f64,
    out_sx: *mut f64,
    out_sy: *mut f64,
) {
    let canvas = unsafe { &*canvas };
    let guard = lock_or_recover(&canvas.0);
    let result = guard.canvas_to_screen(Point::new(cx, cy));
    unsafe {
        *out_sx = result.x;
        *out_sy = result.y;
    }
}

/// Returns the current zoom level.
///
/// # Safety
/// `canvas` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_zoom_level(
    canvas: *const MagicCanvasState,
) -> f64 {
    let canvas = unsafe { &*canvas };
    let guard = lock_or_recover(&canvas.0);
    guard.zoom_level()
}

/// Set the zoom level (clamped to valid range).
///
/// # Safety
/// `canvas` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_set_zoom(
    canvas: *const MagicCanvasState,
    level: f64,
) {
    let canvas = unsafe { &*canvas };
    let mut guard = lock_or_recover(&canvas.0);
    guard.set_zoom(level);
}

/// Zoom by a multiplicative factor around a center point (canvas coords).
///
/// # Safety
/// `canvas` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_zoom_by(
    canvas: *const MagicCanvasState,
    factor: f64,
    cx: f64,
    cy: f64,
) {
    let canvas = unsafe { &*canvas };
    let mut guard = lock_or_recover(&canvas.0);
    guard.zoom_by(factor, Point::new(cx, cy));
}

/// Zoom to fit a rectangle in the viewport.
///
/// # Safety
/// `canvas` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_zoom_to_fit(
    canvas: *const MagicCanvasState,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    let canvas = unsafe { &*canvas };
    let mut guard = lock_or_recover(&canvas.0);
    guard.zoom_to_fit(Rect::new(x, y, w, h));
}

/// Set zoom to 100% (actual size).
///
/// # Safety
/// `canvas` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_zoom_actual_size(
    canvas: *const MagicCanvasState,
) {
    let canvas = unsafe { &*canvas };
    let mut guard = lock_or_recover(&canvas.0);
    guard.zoom_actual_size();
}

/// Set zoom to a specific percentage (e.g. 200.0 for 200%).
///
/// # Safety
/// `canvas` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_zoom_percent(
    canvas: *const MagicCanvasState,
    percent: f64,
) {
    let canvas = unsafe { &*canvas };
    let mut guard = lock_or_recover(&canvas.0);
    guard.zoom_percent(percent);
}

/// Returns the current canvas selection as a JSON array of UUID strings.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `canvas` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_selection(
    canvas: *const MagicCanvasState,
) -> *mut c_char {
    let canvas = unsafe { &*canvas };
    let guard = lock_or_recover(&canvas.0);
    json_to_c(&guard.selection())
}

/// Select a single digit on the canvas (replaces current selection).
///
/// # Safety
/// `canvas` must be a valid pointer. `id` must be a valid C string (UUID).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_select(
    canvas: *const MagicCanvasState,
    id: *const c_char,
) {
    let canvas = unsafe { &*canvas };
    if let Some(id_str) = c_str_to_str(id) {
        if let Ok(uuid) = Uuid::parse_str(id_str) {
            let mut guard = lock_or_recover(&canvas.0);
            guard.select(uuid);
        }
    }
}

/// Select multiple digits on the canvas (adds to current selection).
///
/// # Safety
/// `canvas` must be a valid pointer. `ids_json` must be a valid C string (JSON array of UUIDs).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_select_multiple(
    canvas: *const MagicCanvasState,
    ids_json: *const c_char,
) {
    let canvas = unsafe { &*canvas };
    if let Some(json_str) = c_str_to_str(ids_json) {
        if let Ok(ids) = serde_json::from_str::<Vec<Uuid>>(json_str) {
            let mut guard = lock_or_recover(&canvas.0);
            guard.select_multiple(&ids);
        }
    }
}

/// Deselect a digit on the canvas.
///
/// # Safety
/// `canvas` must be a valid pointer. `id` must be a valid C string (UUID).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_deselect(
    canvas: *const MagicCanvasState,
    id: *const c_char,
) {
    let canvas = unsafe { &*canvas };
    if let Some(id_str) = c_str_to_str(id) {
        if let Ok(uuid) = Uuid::parse_str(id_str) {
            let mut guard = lock_or_recover(&canvas.0);
            guard.deselect(uuid);
        }
    }
}

/// Clear the canvas selection entirely.
///
/// # Safety
/// `canvas` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_clear_selection(
    canvas: *const MagicCanvasState,
) {
    let canvas = unsafe { &*canvas };
    let mut guard = lock_or_recover(&canvas.0);
    guard.clear_selection();
}

/// Check whether a digit is selected on the canvas.
///
/// # Safety
/// `canvas` must be a valid pointer. `id` must be a valid C string (UUID).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_is_selected(
    canvas: *const MagicCanvasState,
    id: *const c_char,
) -> bool {
    let canvas = unsafe { &*canvas };
    let Some(id_str) = c_str_to_str(id) else {
        return false;
    };
    let Ok(uuid) = Uuid::parse_str(id_str) else {
        return false;
    };
    let guard = lock_or_recover(&canvas.0);
    guard.is_selected(uuid)
}

/// Returns the number of selected items on the canvas.
///
/// # Safety
/// `canvas` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_selection_count(
    canvas: *const MagicCanvasState,
) -> usize {
    let canvas = unsafe { &*canvas };
    let guard = lock_or_recover(&canvas.0);
    guard.selection_count()
}

/// Snap a point to the grid (if enabled). Writes result to `out_x`, `out_y`.
///
/// # Safety
/// `canvas` must be a valid pointer. `out_x` and `out_y` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_snap_point(
    canvas: *const MagicCanvasState,
    x: f64,
    y: f64,
    out_x: *mut f64,
    out_y: *mut f64,
) {
    let canvas = unsafe { &*canvas };
    let guard = lock_or_recover(&canvas.0);
    let result = guard.snap_point(Point::new(x, y));
    unsafe {
        *out_x = result.x;
        *out_y = result.y;
    }
}

/// Set grid size and snap-to-grid. Pass 0 for `size` to disable grid.
///
/// # Safety
/// `canvas` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_set_grid(
    canvas: *const MagicCanvasState,
    size: f64,
    snap: bool,
) {
    let canvas = unsafe { &*canvas };
    let mut guard = lock_or_recover(&canvas.0);
    guard.grid_size = if size > 0.0 { Some(size) } else { None };
    guard.snap_to_grid = snap;
}

/// Set whether alignment guides are visible.
///
/// # Safety
/// `canvas` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_set_guides(
    canvas: *const MagicCanvasState,
    visible: bool,
) {
    let canvas = unsafe { &*canvas };
    let mut guard = lock_or_recover(&canvas.0);
    guard.show_guides = visible;
}

/// Compute drag handles for a selection bounding rect at the given zoom.
///
/// Returns a JSON array of DragHandle objects, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// All parameters must be valid.
#[unsafe(no_mangle)]
pub extern "C" fn divi_magic_canvas_compute_handles(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    zoom: f64,
) -> *mut c_char {
    let rect = Rect::new(x, y, w, h);
    let handles = compute_handles(rect, zoom);
    json_to_c(&handles)
}

/// Hit-test a point against a set of handles (from JSON).
///
/// Returns the matching DragHandle as JSON, or null if no hit.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `handles_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_canvas_hit_test_handles(
    px: f64,
    py: f64,
    handles_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let Some(json_str) = c_str_to_str(handles_json) else {
        set_last_error("divi_magic_canvas_hit_test_handles: invalid handles_json");
        return std::ptr::null_mut();
    };
    let handles: Vec<magic::DragHandle> = match serde_json::from_str(json_str) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(format!("divi_magic_canvas_hit_test_handles: {e}"));
            return std::ptr::null_mut();
        }
    };
    match hit_test_handles(Point::new(px, py), &handles) {
        Some(handle) => json_to_c(handle),
        None => std::ptr::null_mut(),
    }
}

// ===========================================================================
// RendererRegistry — Imagination
// ===========================================================================

/// Create a new empty renderer registry (with fallback renderer).
///
/// Free with `divi_magic_renderer_registry_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_magic_renderer_registry_new() -> *mut MagicRendererRegistry {
    Box::into_raw(Box::new(MagicRendererRegistry(Mutex::new(
        RendererRegistry::new(),
    ))))
}

/// Create a renderer registry pre-loaded with all built-in renderers.
///
/// Free with `divi_magic_renderer_registry_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_magic_renderer_registry_new_with_all() -> *mut MagicRendererRegistry {
    let mut registry = RendererRegistry::new();
    register_all_renderers(&mut registry);
    Box::into_raw(Box::new(MagicRendererRegistry(Mutex::new(registry))))
}

/// Free a renderer registry.
///
/// # Safety
/// `ptr` must be a valid pointer from a `divi_magic_renderer_registry_*` constructor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_renderer_registry_free(ptr: *mut MagicRendererRegistry) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Returns the number of registered renderers.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_renderer_registry_count(
    registry: *const MagicRendererRegistry,
) -> usize {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    guard.count()
}

/// Check whether a renderer is registered for the given digit type.
///
/// # Safety
/// `registry` must be a valid pointer. `digit_type` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_renderer_registry_has(
    registry: *const MagicRendererRegistry,
    digit_type: *const c_char,
) -> bool {
    let registry = unsafe { &*registry };
    let Some(dtype) = c_str_to_str(digit_type) else {
        return false;
    };
    let guard = lock_or_recover(&registry.0);
    guard.has_renderer(dtype)
}

/// Returns the registered digit types as a JSON array of strings.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_renderer_registry_types(
    registry: *const MagicRendererRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    let types: Vec<&String> = guard.registered_types().collect();
    json_to_c(&types)
}

/// Render a digit using the appropriate renderer.
///
/// Returns the RenderSpec as JSON, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer. `digit_json`, `mode_json`, `context_json`
/// must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_renderer_render(
    registry: *const MagicRendererRegistry,
    digit_json: *const c_char,
    mode_json: *const c_char,
    context_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let registry = unsafe { &*registry };
    let Some(d_str) = c_str_to_str(digit_json) else {
        set_last_error("divi_magic_renderer_render: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(m_str) = c_str_to_str(mode_json) else {
        set_last_error("divi_magic_renderer_render: invalid mode_json");
        return std::ptr::null_mut();
    };
    let Some(c_str) = c_str_to_str(context_json) else {
        set_last_error("divi_magic_renderer_render: invalid context_json");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(d_str) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_magic_renderer_render: digit parse: {e}"));
            return std::ptr::null_mut();
        }
    };
    let mode: RenderMode = match serde_json::from_str(m_str) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("divi_magic_renderer_render: mode parse: {e}"));
            return std::ptr::null_mut();
        }
    };
    let context: RenderContext = match serde_json::from_str(c_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_magic_renderer_render: context parse: {e}"));
            return std::ptr::null_mut();
        }
    };

    let guard = lock_or_recover(&registry.0);
    let spec = guard.render(&digit, mode, &context);
    json_to_c(&spec)
}

/// Get the estimated size for a digit without full rendering.
///
/// Writes width and height to `out_w` and `out_h`.
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `registry` must be a valid pointer. `digit_json`, `context_json`
/// must be valid C strings. `out_w`, `out_h` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_renderer_estimated_size(
    registry: *const MagicRendererRegistry,
    digit_json: *const c_char,
    context_json: *const c_char,
    out_w: *mut f64,
    out_h: *mut f64,
) -> i32 {
    clear_last_error();
    let registry = unsafe { &*registry };
    let Some(d_str) = c_str_to_str(digit_json) else {
        set_last_error("divi_magic_renderer_estimated_size: invalid digit_json");
        return -1;
    };
    let Some(c_str) = c_str_to_str(context_json) else {
        set_last_error("divi_magic_renderer_estimated_size: invalid context_json");
        return -1;
    };

    let digit: Digit = match serde_json::from_str(d_str) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_magic_renderer_estimated_size: digit parse: {e}"));
            return -1;
        }
    };
    let context: RenderContext = match serde_json::from_str(c_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_magic_renderer_estimated_size: context parse: {e}"));
            return -1;
        }
    };

    let guard = lock_or_recover(&registry.0);
    let renderer = guard.get(digit.digit_type());
    let (w, h) = renderer.estimated_size(&digit, &context);
    unsafe {
        *out_w = w;
        *out_h = h;
    }
    0
}

// ===========================================================================
// RenderCache — LRU cache for RenderSpecs
// ===========================================================================

/// Create a new render cache with default max size (200).
///
/// Free with `divi_magic_cache_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_magic_cache_new() -> *mut MagicRenderCache {
    Box::into_raw(Box::new(MagicRenderCache(Mutex::new(RenderCache::new()))))
}

/// Create a new render cache with a specific max size.
///
/// Free with `divi_magic_cache_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_magic_cache_with_max_size(max_size: usize) -> *mut MagicRenderCache {
    Box::into_raw(Box::new(MagicRenderCache(Mutex::new(
        RenderCache::with_max_size(max_size),
    ))))
}

/// Free a render cache.
///
/// # Safety
/// `ptr` must be a valid pointer from a `divi_magic_cache_*` constructor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_cache_free(ptr: *mut MagicRenderCache) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Get a cached RenderSpec as JSON. Returns null if not cached (miss).
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `cache` must be a valid pointer. `digit_id` must be a valid C string (UUID).
/// `mode_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_cache_get(
    cache: *const MagicRenderCache,
    digit_id: *const c_char,
    mode_json: *const c_char,
) -> *mut c_char {
    let cache = unsafe { &*cache };
    let Some(id_str) = c_str_to_str(digit_id) else {
        return std::ptr::null_mut();
    };
    let Ok(uuid) = Uuid::parse_str(id_str) else {
        return std::ptr::null_mut();
    };
    let Some(m_str) = c_str_to_str(mode_json) else {
        return std::ptr::null_mut();
    };
    let Ok(mode) = serde_json::from_str::<RenderMode>(m_str) else {
        return std::ptr::null_mut();
    };

    let mut guard = lock_or_recover(&cache.0);
    match guard.get(uuid, mode) {
        Some(spec) => json_to_c(spec),
        None => std::ptr::null_mut(),
    }
}

/// Insert a RenderSpec (from JSON) into the cache.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `cache` must be a valid pointer. `spec_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_cache_insert(
    cache: *const MagicRenderCache,
    spec_json: *const c_char,
) -> i32 {
    clear_last_error();
    let cache = unsafe { &*cache };
    let Some(json_str) = c_str_to_str(spec_json) else {
        set_last_error("divi_magic_cache_insert: invalid spec_json");
        return -1;
    };
    let spec = match serde_json::from_str(json_str) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_magic_cache_insert: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&cache.0);
    guard.insert(spec);
    0
}

/// Invalidate all cached specs for a digit.
///
/// # Safety
/// `cache` must be a valid pointer. `digit_id` must be a valid C string (UUID).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_cache_invalidate(
    cache: *const MagicRenderCache,
    digit_id: *const c_char,
) {
    let cache = unsafe { &*cache };
    if let Some(id_str) = c_str_to_str(digit_id) {
        if let Ok(uuid) = Uuid::parse_str(id_str) {
            let mut guard = lock_or_recover(&cache.0);
            guard.invalidate(uuid);
        }
    }
}

/// Clear the entire render cache.
///
/// # Safety
/// `cache` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_cache_invalidate_all(
    cache: *const MagicRenderCache,
) {
    let cache = unsafe { &*cache };
    let mut guard = lock_or_recover(&cache.0);
    guard.invalidate_all();
}

/// Returns the number of cached entries.
///
/// # Safety
/// `cache` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_cache_size(
    cache: *const MagicRenderCache,
) -> usize {
    let cache = unsafe { &*cache };
    let guard = lock_or_recover(&cache.0);
    guard.size()
}

/// Returns the cache hit rate (0.0 to 1.0).
///
/// # Safety
/// `cache` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_cache_hit_rate(
    cache: *const MagicRenderCache,
) -> f64 {
    let cache = unsafe { &*cache };
    let guard = lock_or_recover(&cache.0);
    guard.hit_rate()
}

// ===========================================================================
// ToolRegistry — canvas tool management
// ===========================================================================

/// Create a new empty tool registry.
///
/// Free with `divi_magic_tool_registry_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_magic_tool_registry_new() -> *mut MagicToolRegistry {
    Box::into_raw(Box::new(MagicToolRegistry(Mutex::new(ToolRegistry::new()))))
}

/// Create a tool registry pre-loaded with all default tools.
///
/// Free with `divi_magic_tool_registry_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_magic_tool_registry_new_default() -> *mut MagicToolRegistry {
    Box::into_raw(Box::new(MagicToolRegistry(Mutex::new(
        default_tool_registry(),
    ))))
}

/// Free a tool registry.
///
/// # Safety
/// `ptr` must be a valid pointer from a `divi_magic_tool_registry_*` constructor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_tool_registry_free(ptr: *mut MagicToolRegistry) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Returns the number of registered tools.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_tool_registry_count(
    registry: *const MagicToolRegistry,
) -> usize {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    guard.len()
}

/// Returns the IDs of all registered tools as a JSON array of strings.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_tool_registry_list(
    registry: *const MagicToolRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    let list = guard.list();
    json_to_c(&list)
}

/// Select (activate) a tool by ID. Returns `true` if found.
///
/// # Safety
/// `registry` must be a valid pointer. `id` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_tool_registry_select(
    registry: *const MagicToolRegistry,
    id: *const c_char,
) -> bool {
    let registry = unsafe { &*registry };
    let Some(id_str) = c_str_to_str(id) else {
        return false;
    };
    let mut guard = lock_or_recover(&registry.0);
    guard.select(id_str)
}

/// Returns the active tool's ID as a string, or null if none active.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_tool_registry_active_id(
    registry: *const MagicToolRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    match guard.active() {
        Some(tool) => string_to_c(tool.id().to_string()),
        None => std::ptr::null_mut(),
    }
}

/// Returns the active tool's cursor style as JSON, or null if none active.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `registry` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_tool_registry_active_cursor(
    registry: *const MagicToolRegistry,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    match guard.active() {
        Some(tool) => json_to_c(&tool.cursor()),
        None => std::ptr::null_mut(),
    }
}

/// Handle a press event on the active tool.
///
/// Lock order: tool registry first, then document (prevents deadlock).
///
/// Returns the resulting ToolAction as JSON, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `registry` and `doc` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_tool_on_press(
    registry: *const MagicToolRegistry,
    x: f64,
    y: f64,
    shift: bool,
    alt: bool,
    command: bool,
    doc: *const MagicDocumentState,
) -> *mut c_char {
    clear_last_error();
    let registry = unsafe { &*registry };
    let doc = unsafe { &*doc };
    let point = Point::new(x, y);
    let modifiers = ModifierKeys { shift, alt, command };

    // Lock order: registry first, then document
    let mut reg_guard = lock_or_recover(&registry.0);
    let doc_guard = lock_or_recover(&doc.0);

    match reg_guard.active_mut() {
        Some(tool) => {
            let action = tool.on_press(point, modifiers, &doc_guard);
            json_to_c(&action)
        }
        None => {
            set_last_error("divi_magic_tool_on_press: no active tool");
            std::ptr::null_mut()
        }
    }
}

/// Handle a drag event on the active tool.
///
/// Lock order: tool registry first, then document (prevents deadlock).
///
/// Returns the resulting ToolAction as JSON, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `registry` and `doc` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_tool_on_drag(
    registry: *const MagicToolRegistry,
    x: f64,
    y: f64,
    shift: bool,
    alt: bool,
    command: bool,
    doc: *const MagicDocumentState,
) -> *mut c_char {
    clear_last_error();
    let registry = unsafe { &*registry };
    let doc = unsafe { &*doc };
    let point = Point::new(x, y);
    let modifiers = ModifierKeys { shift, alt, command };

    let mut reg_guard = lock_or_recover(&registry.0);
    let doc_guard = lock_or_recover(&doc.0);

    match reg_guard.active_mut() {
        Some(tool) => {
            let action = tool.on_drag(point, modifiers, &doc_guard);
            json_to_c(&action)
        }
        None => {
            set_last_error("divi_magic_tool_on_drag: no active tool");
            std::ptr::null_mut()
        }
    }
}

/// Handle a release event on the active tool.
///
/// Lock order: tool registry first, then document (prevents deadlock).
///
/// Returns the resulting ToolAction as JSON, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `registry` and `doc` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_tool_on_release(
    registry: *const MagicToolRegistry,
    x: f64,
    y: f64,
    shift: bool,
    alt: bool,
    command: bool,
    doc: *const MagicDocumentState,
) -> *mut c_char {
    clear_last_error();
    let registry = unsafe { &*registry };
    let doc = unsafe { &*doc };
    let point = Point::new(x, y);
    let modifiers = ModifierKeys { shift, alt, command };

    let mut reg_guard = lock_or_recover(&registry.0);
    let doc_guard = lock_or_recover(&doc.0);

    match reg_guard.active_mut() {
        Some(tool) => {
            let action = tool.on_release(point, modifiers, &doc_guard);
            json_to_c(&action)
        }
        None => {
            set_last_error("divi_magic_tool_on_release: no active tool");
            std::ptr::null_mut()
        }
    }
}

/// Handle a hover event on the active tool.
///
/// Returns the cursor style as JSON, or null if no override / no active tool.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `registry` and `doc` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_tool_on_hover(
    registry: *const MagicToolRegistry,
    x: f64,
    y: f64,
    doc: *const MagicDocumentState,
) -> *mut c_char {
    let registry = unsafe { &*registry };
    let doc = unsafe { &*doc };
    let point = Point::new(x, y);

    let reg_guard = lock_or_recover(&registry.0);
    let doc_guard = lock_or_recover(&doc.0);

    match reg_guard.active() {
        Some(tool) => match tool.on_hover(point, &doc_guard) {
            Some(cursor) => json_to_c(&cursor),
            None => std::ptr::null_mut(),
        },
        None => std::ptr::null_mut(),
    }
}

// ===========================================================================
// DocumentHistory — undo/redo
// ===========================================================================

/// Create a new empty document history.
///
/// Free with `divi_magic_history_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_magic_history_new() -> *mut MagicDocumentHistory {
    Box::into_raw(Box::new(MagicDocumentHistory(Mutex::new(
        DocumentHistory::new(),
    ))))
}

/// Free a document history.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_magic_history_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_history_free(ptr: *mut MagicDocumentHistory) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Whether undo is available.
///
/// # Safety
/// `history` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_history_can_undo(
    history: *const MagicDocumentHistory,
) -> bool {
    let history = unsafe { &*history };
    let guard = lock_or_recover(&history.0);
    guard.can_undo()
}

/// Whether redo is available.
///
/// # Safety
/// `history` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_history_can_redo(
    history: *const MagicDocumentHistory,
) -> bool {
    let history = unsafe { &*history };
    let guard = lock_or_recover(&history.0);
    guard.can_redo()
}

/// Returns the number of undo entries.
///
/// # Safety
/// `history` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_history_undo_count(
    history: *const MagicDocumentHistory,
) -> usize {
    let history = unsafe { &*history };
    let guard = lock_or_recover(&history.0);
    guard.undo_count()
}

/// Returns the number of redo entries.
///
/// # Safety
/// `history` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_history_redo_count(
    history: *const MagicDocumentHistory,
) -> usize {
    let history = unsafe { &*history };
    let guard = lock_or_recover(&history.0);
    guard.redo_count()
}

/// Clear all undo/redo history.
///
/// # Safety
/// `history` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_history_clear(
    history: *const MagicDocumentHistory,
) {
    let history = unsafe { &*history };
    let mut guard = lock_or_recover(&history.0);
    guard.clear();
}

/// Execute an action: apply it to the document and record undo history.
///
/// Lock order: history first, then document (prevents deadlock).
///
/// Returns the resulting operation as JSON, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `history` and `doc` must be valid pointers. `action_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_history_execute(
    history: *const MagicDocumentHistory,
    doc: *const MagicDocumentState,
    action_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let history = unsafe { &*history };
    let doc = unsafe { &*doc };
    let Some(json_str) = c_str_to_str(action_json) else {
        set_last_error("divi_magic_history_execute: invalid action_json");
        return std::ptr::null_mut();
    };
    let action: Action = match serde_json::from_str(json_str) {
        Ok(a) => a,
        Err(e) => {
            set_last_error(format!("divi_magic_history_execute: {e}"));
            return std::ptr::null_mut();
        }
    };

    // Lock order: history first, then document
    let mut hist_guard = lock_or_recover(&history.0);
    let mut doc_guard = lock_or_recover(&doc.0);

    match action.execute(&mut doc_guard) {
        Ok((op, inverse)) => {
            hist_guard.record(HistoryEntry {
                operation: op.clone(),
                inverse,
            });
            json_to_c(&op)
        }
        Err(e) => {
            set_last_error(format!("divi_magic_history_execute: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Undo the last action.
///
/// Lock order: history first, then document (prevents deadlock).
///
/// Returns the undo operation as JSON, or null if nothing to undo / error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `history` and `doc` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_history_undo(
    history: *const MagicDocumentHistory,
    doc: *const MagicDocumentState,
) -> *mut c_char {
    clear_last_error();
    let history = unsafe { &*history };
    let doc = unsafe { &*doc };

    // Lock order: history first, then document
    let mut hist_guard = lock_or_recover(&history.0);
    let mut doc_guard = lock_or_recover(&doc.0);

    let entry = match hist_guard.pop_undo() {
        Some(e) => e,
        None => {
            set_last_error("divi_magic_history_undo: nothing to undo");
            return std::ptr::null_mut();
        }
    };

    // Execute the inverse action
    match entry.inverse.execute(&mut doc_guard) {
        Ok((op, re_inverse)) => {
            hist_guard.push_redo(HistoryEntry {
                operation: op.clone(),
                inverse: re_inverse,
            });
            json_to_c(&op)
        }
        Err(e) => {
            set_last_error(format!("divi_magic_history_undo: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Redo the last undone action.
///
/// Lock order: history first, then document (prevents deadlock).
///
/// Returns the redo operation as JSON, or null if nothing to redo / error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `history` and `doc` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_history_redo(
    history: *const MagicDocumentHistory,
    doc: *const MagicDocumentState,
) -> *mut c_char {
    clear_last_error();
    let history = unsafe { &*history };
    let doc = unsafe { &*doc };

    // Lock order: history first, then document
    let mut hist_guard = lock_or_recover(&history.0);
    let mut doc_guard = lock_or_recover(&doc.0);

    let entry = match hist_guard.pop_redo() {
        Some(e) => e,
        None => {
            set_last_error("divi_magic_history_redo: nothing to redo");
            return std::ptr::null_mut();
        }
    };

    // Execute the redo action. Uses `record()` which pushes onto the undo
    // stack. Note: `record()` also clears remaining redo entries, which
    // matches the "branch on new action" convention used by most editors.
    match entry.inverse.execute(&mut doc_guard) {
        Ok((op, re_inverse)) => {
            hist_guard.record(HistoryEntry {
                operation: op.clone(),
                inverse: re_inverse,
            });
            json_to_c(&op)
        }
        Err(e) => {
            set_last_error(format!("divi_magic_history_redo: {e}"));
            std::ptr::null_mut()
        }
    }
}

// ===========================================================================
// Projection — code generation from digits
// ===========================================================================

/// Build a ProjectionContext from digits and produce it as JSON.
///
/// Returns the ProjectionContext as JSON, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `digits_json` and `reign_json` must be valid C strings. `root_id` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_projection_build_context(
    digits_json: *const c_char,
    root_id: *const c_char,
    reign_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let Some(d_str) = c_str_to_str(digits_json) else {
        set_last_error("divi_magic_projection_build_context: invalid digits_json");
        return std::ptr::null_mut();
    };
    let Some(r_str) = c_str_to_str(reign_json) else {
        set_last_error("divi_magic_projection_build_context: invalid reign_json");
        return std::ptr::null_mut();
    };

    let digits: Vec<Digit> = match serde_json::from_str(d_str) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_magic_projection_build_context: digits parse: {e}"));
            return std::ptr::null_mut();
        }
    };
    let reign: Reign = match serde_json::from_str(r_str) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_magic_projection_build_context: reign parse: {e}"));
            return std::ptr::null_mut();
        }
    };
    let rid = c_str_to_str(root_id).and_then(|s| Uuid::parse_str(s).ok());

    let context = ProjectionContext::build(&digits, rid, reign);
    json_to_c(&context)
}

/// Project SwiftUI code from a ProjectionContext (JSON).
///
/// Returns a JSON array of GeneratedFile, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `context_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_projection_swiftui(
    context_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let Some(json_str) = c_str_to_str(context_json) else {
        set_last_error("divi_magic_projection_swiftui: invalid context_json");
        return std::ptr::null_mut();
    };
    let context: ProjectionContext = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_magic_projection_swiftui: context parse: {e}"));
            return std::ptr::null_mut();
        }
    };

    let proj = SwiftUIProjection;
    match proj.project(&context) {
        Ok(files) => json_to_c(&files),
        Err(e) => {
            set_last_error(format!("divi_magic_projection_swiftui: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Project React (TSX) code from a ProjectionContext (JSON).
///
/// Returns a JSON array of GeneratedFile, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `context_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_projection_react(
    context_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let Some(json_str) = c_str_to_str(context_json) else {
        set_last_error("divi_magic_projection_react: invalid context_json");
        return std::ptr::null_mut();
    };
    let context: ProjectionContext = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_magic_projection_react: context parse: {e}"));
            return std::ptr::null_mut();
        }
    };

    let proj = ReactProjection;
    match proj.project(&context) {
        Ok(files) => json_to_c(&files),
        Err(e) => {
            set_last_error(format!("divi_magic_projection_react: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Project HTML/CSS code from a ProjectionContext (JSON).
///
/// Returns a JSON array of GeneratedFile, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `context_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_projection_html(
    context_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let Some(json_str) = c_str_to_str(context_json) else {
        set_last_error("divi_magic_projection_html: invalid context_json");
        return std::ptr::null_mut();
    };
    let context: ProjectionContext = match serde_json::from_str(json_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("divi_magic_projection_html: context parse: {e}"));
            return std::ptr::null_mut();
        }
    };

    let proj = HtmlProjection;
    match proj.project(&context) {
        Ok(files) => json_to_c(&files),
        Err(e) => {
            set_last_error(format!("divi_magic_projection_html: {e}"));
            std::ptr::null_mut()
        }
    }
}

// ===========================================================================
// Utilities — accessibility, slide sequence, type registry
// ===========================================================================

/// Build an AccessibilitySpec from a digit (JSON), role (JSON), and label.
///
/// Returns the AccessibilitySpec as JSON, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `digit_json`, `role_json`, `label` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_accessibility_from_digit(
    digit_json: *const c_char,
    role_json: *const c_char,
    label: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let Some(d_str) = c_str_to_str(digit_json) else {
        set_last_error("divi_magic_accessibility_from_digit: invalid digit_json");
        return std::ptr::null_mut();
    };
    let Some(r_str) = c_str_to_str(role_json) else {
        set_last_error("divi_magic_accessibility_from_digit: invalid role_json");
        return std::ptr::null_mut();
    };
    let Some(label_str) = c_str_to_str(label) else {
        set_last_error("divi_magic_accessibility_from_digit: invalid label");
        return std::ptr::null_mut();
    };

    let digit: Digit = match serde_json::from_str(d_str) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_magic_accessibility_from_digit: digit parse: {e}"));
            return std::ptr::null_mut();
        }
    };
    let role: AccessibilityRole = match serde_json::from_str(r_str) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_magic_accessibility_from_digit: role parse: {e}"));
            return std::ptr::null_mut();
        }
    };

    let spec = AccessibilitySpec::from_digit(&digit, role, label_str);
    json_to_c(&spec)
}

/// Create a new default SlideSequenceState as JSON.
///
/// The returned pointer must be freed via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_magic_slide_sequence_new() -> *mut c_char {
    let state = SlideSequenceState::default();
    json_to_c(&state)
}

/// Advance to the next slide. Returns the updated state as JSON.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `state_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_slide_sequence_next(
    state_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let Some(json_str) = c_str_to_str(state_json) else {
        set_last_error("divi_magic_slide_sequence_next: invalid state_json");
        return std::ptr::null_mut();
    };
    let mut state: SlideSequenceState = match serde_json::from_str(json_str) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_magic_slide_sequence_next: {e}"));
            return std::ptr::null_mut();
        }
    };
    state.next();
    json_to_c(&state)
}

/// Go to the previous slide. Returns the updated state as JSON.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `state_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_slide_sequence_previous(
    state_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let Some(json_str) = c_str_to_str(state_json) else {
        set_last_error("divi_magic_slide_sequence_previous: invalid state_json");
        return std::ptr::null_mut();
    };
    let mut state: SlideSequenceState = match serde_json::from_str(json_str) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_magic_slide_sequence_previous: {e}"));
            return std::ptr::null_mut();
        }
    };
    state.previous();
    json_to_c(&state)
}

/// Jump to a specific slide index. Returns the updated state as JSON.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `state_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_slide_sequence_go_to(
    state_json: *const c_char,
    index: usize,
) -> *mut c_char {
    clear_last_error();
    let Some(json_str) = c_str_to_str(state_json) else {
        set_last_error("divi_magic_slide_sequence_go_to: invalid state_json");
        return std::ptr::null_mut();
    };
    let mut state: SlideSequenceState = match serde_json::from_str(json_str) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_magic_slide_sequence_go_to: {e}"));
            return std::ptr::null_mut();
        }
    };
    state.go_to(index);
    json_to_c(&state)
}

/// Insert a slide at the given index. Returns the updated state as JSON.
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `state_json` and `slide_id` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_slide_sequence_add_slide(
    state_json: *const c_char,
    slide_id: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let Some(json_str) = c_str_to_str(state_json) else {
        set_last_error("divi_magic_slide_sequence_add_slide: invalid state_json");
        return std::ptr::null_mut();
    };
    let Some(id_str) = c_str_to_str(slide_id) else {
        set_last_error("divi_magic_slide_sequence_add_slide: invalid slide_id");
        return std::ptr::null_mut();
    };
    let Ok(uuid) = Uuid::parse_str(id_str) else {
        set_last_error("divi_magic_slide_sequence_add_slide: invalid UUID");
        return std::ptr::null_mut();
    };
    let mut state: SlideSequenceState = match serde_json::from_str(json_str) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_magic_slide_sequence_add_slide: {e}"));
            return std::ptr::null_mut();
        }
    };
    let len = state.slide_count();
    state.insert_slide(len, uuid);
    json_to_c(&state)
}

/// Returns the core type registry (9 built-in types) as JSON.
///
/// The returned pointer must be freed via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_magic_type_registry_core() -> *mut c_char {
    let registry = DigitTypeRegistry::with_core_types();
    json_to_c(&registry)
}

/// Get a specific type definition from a registry (JSON).
///
/// Returns the DigitTypeDefinition as JSON, or null if not found.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `registry_json` and `digit_type` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_magic_type_registry_get(
    registry_json: *const c_char,
    digit_type: *const c_char,
) -> *mut c_char {
    let Some(r_str) = c_str_to_str(registry_json) else {
        return std::ptr::null_mut();
    };
    let Some(t_str) = c_str_to_str(digit_type) else {
        return std::ptr::null_mut();
    };
    let Ok(registry) = serde_json::from_str::<DigitTypeRegistry>(r_str) else {
        return std::ptr::null_mut();
    };
    match registry.get(t_str) {
        Some(def) => json_to_c(def),
        None => std::ptr::null_mut(),
    }
}
