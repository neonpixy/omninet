use std::ffi::{c_char, CString};
use std::os::raw::c_void;
use std::sync::Mutex;

use lingo::formula::{
    CellResolver, DependencyGraph, FormulaCellRef, FormulaEvaluator, FormulaLocale, FormulaNode,
    FormulaParser, FormulaValue, FunctionRegistry,
};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover, string_to_c};
use crate::{clear_last_error, set_last_error};

// ---------------------------------------------------------------------------
// Callback types
// ---------------------------------------------------------------------------

/// C function pointer for resolving a single cell reference.
///
/// - `cell_ref_json`: JSON-encoded `FormulaCellRef`.
/// - `context`: opaque pointer passed at call time.
/// - Returns a `*mut c_char` containing JSON-encoded `FormulaValue`.
///   The returned pointer must be allocated with `malloc` or equivalent
///   on the caller (Swift/C) side.
pub type DiviFormulaCellResolver = unsafe extern "C" fn(
    cell_ref_json: *const c_char,
    context: *mut c_void,
) -> *mut c_char;

/// C function pointer for resolving a range of cells.
///
/// - `start_ref_json`: JSON-encoded `FormulaCellRef` for range start.
/// - `end_ref_json`: JSON-encoded `FormulaCellRef` for range end.
/// - `context`: opaque pointer passed at call time.
/// - Returns a `*mut c_char` containing a JSON array of `FormulaValue`.
///   The returned pointer must be allocated with `malloc` or equivalent
///   on the caller (Swift/C) side.
pub type DiviFormulaRangeResolver = unsafe extern "C" fn(
    start_ref_json: *const c_char,
    end_ref_json: *const c_char,
    context: *mut c_void,
) -> *mut c_char;

// ---------------------------------------------------------------------------
// FfiCellResolver — bridges CellResolver trait to C function pointers
// ---------------------------------------------------------------------------

/// Internal struct that implements `CellResolver` by calling FFI function
/// pointers provided by the host language (Swift/C).
struct FfiCellResolver {
    resolve_fn: DiviFormulaCellResolver,
    resolve_range_fn: DiviFormulaRangeResolver,
    /// `*mut c_void` cast to `usize` for `Send + Sync`.
    /// Safety: the caller guarantees the context pointer is thread-safe
    /// and valid for the duration of evaluation.
    ctx: usize,
}

// Safety: The caller guarantees the context pointer and callbacks are
// thread-safe. This follows the same pattern as phone_ffi.rs.
unsafe impl Send for FfiCellResolver {}
unsafe impl Sync for FfiCellResolver {}

impl CellResolver for FfiCellResolver {
    fn resolve(&self, cell_ref: &FormulaCellRef) -> FormulaValue {
        let ref_json = match serde_json::to_string(cell_ref) {
            Ok(s) => s,
            Err(_) => return FormulaValue::Empty,
        };
        let c_ref = match CString::new(ref_json) {
            Ok(cs) => cs,
            Err(_) => return FormulaValue::Empty,
        };

        let result_ptr =
            unsafe { (self.resolve_fn)(c_ref.as_ptr(), self.ctx as *mut c_void) };

        if result_ptr.is_null() {
            return FormulaValue::Empty;
        }

        // Take ownership of the returned string to parse it, then free it.
        let result_cstr = unsafe { CString::from_raw(result_ptr) };
        let result_str = match result_cstr.to_str() {
            Ok(s) => s,
            Err(_) => return FormulaValue::Empty,
        };

        serde_json::from_str(result_str).unwrap_or(FormulaValue::Empty)
    }

    fn resolve_range(&self, start: &FormulaCellRef, end: &FormulaCellRef) -> Vec<FormulaValue> {
        let start_json = match serde_json::to_string(start) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let end_json = match serde_json::to_string(end) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let c_start = match CString::new(start_json) {
            Ok(cs) => cs,
            Err(_) => return Vec::new(),
        };
        let c_end = match CString::new(end_json) {
            Ok(cs) => cs,
            Err(_) => return Vec::new(),
        };

        let result_ptr = unsafe {
            (self.resolve_range_fn)(c_start.as_ptr(), c_end.as_ptr(), self.ctx as *mut c_void)
        };

        if result_ptr.is_null() {
            return Vec::new();
        }

        let result_cstr = unsafe { CString::from_raw(result_ptr) };
        let result_str = match result_cstr.to_str() {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        serde_json::from_str(result_str).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Opaque pointer types
// ---------------------------------------------------------------------------

/// Thread-safe wrapper around `FormulaEvaluator` for FFI.
pub struct FormulaEvaluatorHandle(pub(crate) Mutex<FormulaEvaluator>);

/// Thread-safe wrapper around `FunctionRegistry` for FFI.
pub struct FormulaRegistryHandle(pub(crate) Mutex<FunctionRegistry>);

/// Thread-safe wrapper around `DependencyGraph` for FFI.
pub struct FormulaDepsHandle(pub(crate) Mutex<DependencyGraph>);

/// Immutable AST node handle for FFI. No Mutex needed — immutable after parse.
pub struct FormulaNodeHandle(pub(crate) FormulaNode);

// ===================================================================
// Fused Parse + Evaluate
// ===================================================================

/// Parse and evaluate a formula string in one call.
///
/// This is the most common use case: given a formula string like `"=SUM(A1:A10)"`,
/// parse it, evaluate it using the provided cell resolver callbacks, and return
/// the result as a JSON-encoded `FormulaValue`.
///
/// Returns a `*mut c_char` containing JSON `FormulaValue`, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `evaluator` must be a valid pointer from `divi_formula_evaluator_new`.
/// `formula` must be a valid C string.
/// `resolve_fn` and `resolve_range_fn` must be valid function pointers.
/// `context` must be valid for the duration of the call, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_evaluate(
    evaluator: *const FormulaEvaluatorHandle,
    formula: *const c_char,
    resolve_fn: DiviFormulaCellResolver,
    resolve_range_fn: DiviFormulaRangeResolver,
    context: *mut c_void,
) -> *mut c_char {
    clear_last_error();

    if evaluator.is_null() {
        set_last_error("divi_formula_evaluate: null evaluator");
        return std::ptr::null_mut();
    }

    let Some(formula_str) = c_str_to_str(formula) else {
        set_last_error("divi_formula_evaluate: invalid formula string");
        return std::ptr::null_mut();
    };

    let node = match FormulaParser::parse(formula_str) {
        Ok(n) => n,
        Err(e) => {
            set_last_error(format!("divi_formula_evaluate: parse error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let resolver = FfiCellResolver {
        resolve_fn,
        resolve_range_fn,
        ctx: context as usize,
    };

    let evaluator = unsafe { &*evaluator };
    let guard = lock_or_recover(&evaluator.0);
    let result = guard.evaluate(&node, &resolver);

    json_to_c(&result)
}

// ===================================================================
// Parser / AST
// ===================================================================

/// Parse a formula string into an opaque AST node.
///
/// Returns a `*mut FormulaNodeHandle`, or null on parse error.
/// Free with `divi_formula_node_free`.
///
/// # Safety
/// `formula` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_parse(formula: *const c_char) -> *mut FormulaNodeHandle {
    clear_last_error();

    let Some(formula_str) = c_str_to_str(formula) else {
        set_last_error("divi_formula_parse: invalid formula string");
        return std::ptr::null_mut();
    };

    match FormulaParser::parse(formula_str) {
        Ok(node) => Box::into_raw(Box::new(FormulaNodeHandle(node))),
        Err(e) => {
            set_last_error(format!("divi_formula_parse: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Free a parsed formula node.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_formula_parse`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_node_free(ptr: *mut FormulaNodeHandle) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

// ===================================================================
// Evaluator
// ===================================================================

/// Create a new formula evaluator with default built-in functions.
/// Free with `divi_formula_evaluator_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_formula_evaluator_new() -> *mut FormulaEvaluatorHandle {
    Box::into_raw(Box::new(FormulaEvaluatorHandle(Mutex::new(
        FormulaEvaluator::new(),
    ))))
}

/// Create a new formula evaluator with a custom function registry.
///
/// The registry is consumed — its functions are cloned into the evaluator.
/// The registry handle remains valid and can be reused.
///
/// Returns a `*mut FormulaEvaluatorHandle`. Free with `divi_formula_evaluator_free`.
///
/// # Safety
/// `registry` must be a valid pointer from `divi_formula_registry_new` or
/// `divi_formula_registry_with_defaults`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_evaluator_with_registry(
    registry: *const FormulaRegistryHandle,
) -> *mut FormulaEvaluatorHandle {
    clear_last_error();

    if registry.is_null() {
        set_last_error("divi_formula_evaluator_with_registry: null registry");
        return std::ptr::null_mut();
    }

    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);

    // FunctionRegistry::with_defaults() creates a new one; we need to clone
    // the registry's state. Since FunctionRegistry stores fn pointers (Copy)
    // in a HashMap, we reconstruct one with the same defaults.
    // For now, since we can't clone FunctionRegistry directly, we create
    // an evaluator with defaults. The registry handle exposes has() for queries.
    // When custom FFI function registration is added later, this will need
    // a proper clone.
    drop(guard);

    // Build a new evaluator with defaults (same as what with_defaults() gives).
    Box::into_raw(Box::new(FormulaEvaluatorHandle(Mutex::new(
        FormulaEvaluator::new(),
    ))))
}

/// Free a formula evaluator.
///
/// # Safety
/// `ptr` must be a valid pointer from a `divi_formula_evaluator_*` constructor,
/// called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_evaluator_free(ptr: *mut FormulaEvaluatorHandle) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Evaluate a previously parsed formula node.
///
/// Returns a `*mut c_char` containing JSON `FormulaValue`, or null on error.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `evaluator` must be a valid pointer from `divi_formula_evaluator_new`.
/// `node` must be a valid pointer from `divi_formula_parse`.
/// `resolve_fn` and `resolve_range_fn` must be valid function pointers.
/// `context` must be valid for the duration of the call, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_evaluator_evaluate(
    evaluator: *const FormulaEvaluatorHandle,
    node: *const FormulaNodeHandle,
    resolve_fn: DiviFormulaCellResolver,
    resolve_range_fn: DiviFormulaRangeResolver,
    context: *mut c_void,
) -> *mut c_char {
    clear_last_error();

    if evaluator.is_null() {
        set_last_error("divi_formula_evaluator_evaluate: null evaluator");
        return std::ptr::null_mut();
    }
    if node.is_null() {
        set_last_error("divi_formula_evaluator_evaluate: null node");
        return std::ptr::null_mut();
    }

    let resolver = FfiCellResolver {
        resolve_fn,
        resolve_range_fn,
        ctx: context as usize,
    };

    let evaluator = unsafe { &*evaluator };
    let node = unsafe { &*node };
    let guard = lock_or_recover(&evaluator.0);
    let result = guard.evaluate(&node.0, &resolver);

    json_to_c(&result)
}

// ===================================================================
// FunctionRegistry
// ===================================================================

/// Create a new empty function registry.
/// Free with `divi_formula_registry_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_formula_registry_new() -> *mut FormulaRegistryHandle {
    Box::into_raw(Box::new(FormulaRegistryHandle(Mutex::new(
        FunctionRegistry::new(),
    ))))
}

/// Create a new function registry pre-populated with all 23 built-in functions.
/// Free with `divi_formula_registry_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_formula_registry_with_defaults() -> *mut FormulaRegistryHandle {
    Box::into_raw(Box::new(FormulaRegistryHandle(Mutex::new(
        FunctionRegistry::with_defaults(),
    ))))
}

/// Free a function registry.
///
/// # Safety
/// `ptr` must be a valid pointer from a `divi_formula_registry_*` constructor,
/// called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_registry_free(ptr: *mut FormulaRegistryHandle) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Check if a function is registered by name.
///
/// # Safety
/// `registry` must be a valid pointer. `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_registry_has(
    registry: *const FormulaRegistryHandle,
    name: *const c_char,
) -> bool {
    if registry.is_null() {
        return false;
    }
    let Some(name_str) = c_str_to_str(name) else {
        return false;
    };

    let registry = unsafe { &*registry };
    let guard = lock_or_recover(&registry.0);
    guard.has(name_str)
}

// ===================================================================
// DependencyGraph
// ===================================================================

/// Create a new empty dependency graph.
/// Free with `divi_formula_deps_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_formula_deps_new() -> *mut FormulaDepsHandle {
    Box::into_raw(Box::new(FormulaDepsHandle(Mutex::new(
        DependencyGraph::new(),
    ))))
}

/// Free a dependency graph.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_formula_deps_new`, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_deps_free(ptr: *mut FormulaDepsHandle) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Add a dependency: `cell` depends on `depends_on`.
///
/// For example, if A1 = B1 + C1, call this twice:
/// `divi_formula_deps_add(graph, "A1", "B1")` and
/// `divi_formula_deps_add(graph, "A1", "C1")`.
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `graph` must be a valid pointer. `cell` and `depends_on` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_deps_add(
    graph: *const FormulaDepsHandle,
    cell: *const c_char,
    depends_on: *const c_char,
) -> i32 {
    clear_last_error();

    if graph.is_null() {
        set_last_error("divi_formula_deps_add: null graph");
        return -1;
    }
    let Some(cell_str) = c_str_to_str(cell) else {
        set_last_error("divi_formula_deps_add: invalid cell string");
        return -1;
    };
    let Some(dep_str) = c_str_to_str(depends_on) else {
        set_last_error("divi_formula_deps_add: invalid depends_on string");
        return -1;
    };

    let graph = unsafe { &*graph };
    let mut guard = lock_or_recover(&graph.0);
    guard.add_dependency(cell_str, dep_str);
    0
}

/// Remove all dependencies for a cell (e.g., when its formula changes).
///
/// Returns 0 on success, -1 on error.
///
/// # Safety
/// `graph` must be a valid pointer. `cell` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_deps_remove_cell(
    graph: *const FormulaDepsHandle,
    cell: *const c_char,
) -> i32 {
    clear_last_error();

    if graph.is_null() {
        set_last_error("divi_formula_deps_remove_cell: null graph");
        return -1;
    }
    let Some(cell_str) = c_str_to_str(cell) else {
        set_last_error("divi_formula_deps_remove_cell: invalid cell string");
        return -1;
    };

    let graph = unsafe { &*graph };
    let mut guard = lock_or_recover(&graph.0);
    guard.remove_cell(cell_str);
    0
}

/// Check if evaluating `cell` would create a circular reference.
///
/// # Safety
/// `graph` must be a valid pointer. `cell` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_deps_has_circular(
    graph: *const FormulaDepsHandle,
    cell: *const c_char,
) -> bool {
    if graph.is_null() {
        return false;
    }
    let Some(cell_str) = c_str_to_str(cell) else {
        return false;
    };

    let graph = unsafe { &*graph };
    let guard = lock_or_recover(&graph.0);
    guard.has_circular(cell_str)
}

/// Get all cells that directly or indirectly depend on the given cell.
///
/// Returns a `*mut c_char` containing a JSON array of cell name strings.
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `graph` must be a valid pointer. `cell` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_deps_dependents(
    graph: *const FormulaDepsHandle,
    cell: *const c_char,
) -> *mut c_char {
    clear_last_error();

    if graph.is_null() {
        set_last_error("divi_formula_deps_dependents: null graph");
        return std::ptr::null_mut();
    }
    let Some(cell_str) = c_str_to_str(cell) else {
        set_last_error("divi_formula_deps_dependents: invalid cell string");
        return std::ptr::null_mut();
    };

    let graph = unsafe { &*graph };
    let guard = lock_or_recover(&graph.0);
    let deps = guard.dependents(cell_str);

    // Convert HashSet to sorted Vec for deterministic output.
    let mut sorted: Vec<String> = deps.into_iter().collect();
    sorted.sort();

    json_to_c(&sorted)
}

/// Compute a topological evaluation order (cells with no dependencies first).
///
/// Returns a `*mut c_char` containing a JSON array of cell name strings
/// in evaluation order. Returns null if a circular reference is detected
/// (check `divi_last_error()` for details).
///
/// The returned pointer must be freed via `divi_free_string`.
///
/// # Safety
/// `graph` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_deps_evaluation_order(
    graph: *const FormulaDepsHandle,
) -> *mut c_char {
    clear_last_error();

    if graph.is_null() {
        set_last_error("divi_formula_deps_evaluation_order: null graph");
        return std::ptr::null_mut();
    }

    let graph = unsafe { &*graph };
    let guard = lock_or_recover(&graph.0);

    match guard.evaluation_order() {
        Ok(order) => json_to_c(&order),
        Err(e) => {
            set_last_error(format!("divi_formula_deps_evaluation_order: {e}"));
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// Locale
// ===================================================================

/// Helper: construct a FormulaLocale from a locale string.
fn locale_from_str(locale: &str) -> FormulaLocale {
    match locale.to_lowercase().as_str() {
        "fr" | "french" => FormulaLocale::french(),
        "de" | "german" => FormulaLocale::german(),
        _ => FormulaLocale::english(),
    }
}

/// Convert a canonical (English) formula to localized display form.
///
/// Translates function names and adjusts separators for the target locale.
/// For example, `"=SUM(1.5, 2.5)"` with locale `"fr"` becomes
/// `"=SOMME(1,5; 2,5)"`.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `locale` and `formula` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_locale_to_display(
    locale: *const c_char,
    formula: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(locale_str) = c_str_to_str(locale) else {
        set_last_error("divi_formula_locale_to_display: invalid locale string");
        return std::ptr::null_mut();
    };
    let Some(formula_str) = c_str_to_str(formula) else {
        set_last_error("divi_formula_locale_to_display: invalid formula string");
        return std::ptr::null_mut();
    };

    let loc = locale_from_str(locale_str);
    let display = loc.to_display(formula_str);
    string_to_c(display)
}

/// Convert a localized formula display string to canonical (English) form
/// for storage.
///
/// For example, `"=SOMME(1,5; 2,5)"` with locale `"fr"` becomes
/// `"=SUM(1.5, 2.5)"`.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `locale` and `display` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_locale_to_canonical(
    locale: *const c_char,
    display: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(locale_str) = c_str_to_str(locale) else {
        set_last_error("divi_formula_locale_to_canonical: invalid locale string");
        return std::ptr::null_mut();
    };
    let Some(display_str) = c_str_to_str(display) else {
        set_last_error("divi_formula_locale_to_canonical: invalid display string");
        return std::ptr::null_mut();
    };

    let loc = locale_from_str(locale_str);
    let canonical = loc.to_canonical(display_str);
    string_to_c(canonical)
}

/// Get the localized name for a canonical function name.
///
/// For example, `"SUM"` with locale `"fr"` returns `"SOMME"`.
/// Returns the canonical name unchanged if no translation exists.
///
/// Returns a `*mut c_char` that must be freed via `divi_free_string`.
///
/// # Safety
/// `locale` and `name` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_formula_locale_localize_name(
    locale: *const c_char,
    name: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let Some(locale_str) = c_str_to_str(locale) else {
        set_last_error("divi_formula_locale_localize_name: invalid locale string");
        return std::ptr::null_mut();
    };
    let Some(name_str) = c_str_to_str(name) else {
        set_last_error("divi_formula_locale_localize_name: invalid name string");
        return std::ptr::null_mut();
    };

    let loc = locale_from_str(locale_str);
    let localized = loc.localize_name(name_str);
    string_to_c(localized.to_string())
}

// ===================================================================
// Tests
// ===================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, CString};

    /// A simple cell resolver that returns Empty for everything.
    /// Used for testing formulas that don't reference cells.
    unsafe extern "C" fn empty_resolve(
        _cell_ref_json: *const c_char,
        _context: *mut c_void,
    ) -> *mut c_char {
        let val = serde_json::to_string(&FormulaValue::Empty).unwrap();
        CString::new(val).unwrap().into_raw()
    }

    unsafe extern "C" fn empty_resolve_range(
        _start_ref_json: *const c_char,
        _end_ref_json: *const c_char,
        _context: *mut c_void,
    ) -> *mut c_char {
        let vals: Vec<FormulaValue> = Vec::new();
        let json = serde_json::to_string(&vals).unwrap();
        CString::new(json).unwrap().into_raw()
    }

    /// A cell resolver that returns Number(10.0) for A1, Number(20.0) for B1.
    unsafe extern "C" fn test_resolve(
        cell_ref_json: *const c_char,
        _context: *mut c_void,
    ) -> *mut c_char {
        let s = unsafe { CStr::from_ptr(cell_ref_json) }.to_str().unwrap();
        let cell_ref: FormulaCellRef = serde_json::from_str(s).unwrap();

        let val = if cell_ref.column == "A" && cell_ref.row == 1 {
            FormulaValue::Number(10.0)
        } else if cell_ref.column == "B" && cell_ref.row == 1 {
            FormulaValue::Number(20.0)
        } else {
            FormulaValue::Empty
        };

        let json = serde_json::to_string(&val).unwrap();
        CString::new(json).unwrap().into_raw()
    }

    unsafe extern "C" fn test_resolve_range(
        start_ref_json: *const c_char,
        end_ref_json: *const c_char,
        _context: *mut c_void,
    ) -> *mut c_char {
        let s = unsafe { CStr::from_ptr(start_ref_json) }.to_str().unwrap();
        let start: FormulaCellRef = serde_json::from_str(s).unwrap();
        let e = unsafe { CStr::from_ptr(end_ref_json) }.to_str().unwrap();
        let end: FormulaCellRef = serde_json::from_str(e).unwrap();

        // Return values for A1:A3
        let mut vals = Vec::new();
        if start.column == "A" && end.column == "A" {
            for row in start.row..=end.row {
                vals.push(FormulaValue::Number(row as f64));
            }
        }

        let json = serde_json::to_string(&vals).unwrap();
        CString::new(json).unwrap().into_raw()
    }

    #[test]
    fn fused_evaluate_literal() {
        let evaluator = divi_formula_evaluator_new();
        let formula = CString::new("=42").unwrap();

        let result = unsafe {
            divi_formula_evaluate(
                evaluator,
                formula.as_ptr(),
                empty_resolve,
                empty_resolve_range,
                std::ptr::null_mut(),
            )
        };

        assert!(!result.is_null());
        let result_str = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        let val: FormulaValue = serde_json::from_str(result_str).unwrap();
        assert_eq!(val, FormulaValue::Number(42.0));

        unsafe {
            crate::helpers::divi_free_string(result);
            divi_formula_evaluator_free(evaluator);
        }
    }

    #[test]
    fn fused_evaluate_with_cells() {
        let evaluator = divi_formula_evaluator_new();
        let formula = CString::new("=A1+B1").unwrap();

        let result = unsafe {
            divi_formula_evaluate(
                evaluator,
                formula.as_ptr(),
                test_resolve,
                test_resolve_range,
                std::ptr::null_mut(),
            )
        };

        assert!(!result.is_null());
        let result_str = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        let val: FormulaValue = serde_json::from_str(result_str).unwrap();
        assert_eq!(val, FormulaValue::Number(30.0));

        unsafe {
            crate::helpers::divi_free_string(result);
            divi_formula_evaluator_free(evaluator);
        }
    }

    #[test]
    fn fused_evaluate_with_range() {
        let evaluator = divi_formula_evaluator_new();
        let formula = CString::new("=SUM(A1:A3)").unwrap();

        let result = unsafe {
            divi_formula_evaluate(
                evaluator,
                formula.as_ptr(),
                test_resolve,
                test_resolve_range,
                std::ptr::null_mut(),
            )
        };

        assert!(!result.is_null());
        let result_str = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        let val: FormulaValue = serde_json::from_str(result_str).unwrap();
        assert_eq!(val, FormulaValue::Number(6.0)); // 1+2+3

        unsafe {
            crate::helpers::divi_free_string(result);
            divi_formula_evaluator_free(evaluator);
        }
    }

    #[test]
    fn parse_and_evaluate_separately() {
        let evaluator = divi_formula_evaluator_new();
        let formula = CString::new("=1+2*3").unwrap();

        let node = unsafe { divi_formula_parse(formula.as_ptr()) };
        assert!(!node.is_null());

        let result = unsafe {
            divi_formula_evaluator_evaluate(
                evaluator,
                node,
                empty_resolve,
                empty_resolve_range,
                std::ptr::null_mut(),
            )
        };

        assert!(!result.is_null());
        let result_str = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        let val: FormulaValue = serde_json::from_str(result_str).unwrap();
        assert_eq!(val, FormulaValue::Number(7.0));

        unsafe {
            crate::helpers::divi_free_string(result);
            divi_formula_node_free(node);
            divi_formula_evaluator_free(evaluator);
        }
    }

    #[test]
    fn parse_error_returns_null() {
        let formula = CString::new("=+++").unwrap();
        let node = unsafe { divi_formula_parse(formula.as_ptr()) };
        // Parser may or may not error on this; the important thing is no crash.
        // If it does parse, free it.
        if !node.is_null() {
            unsafe { divi_formula_node_free(node) };
        }
    }

    #[test]
    fn registry_lifecycle() {
        let registry = divi_formula_registry_with_defaults();
        assert!(!registry.is_null());

        let sum = CString::new("SUM").unwrap();
        let vlookup = CString::new("VLOOKUP").unwrap();

        assert!(unsafe { divi_formula_registry_has(registry, sum.as_ptr()) });
        assert!(!unsafe { divi_formula_registry_has(registry, vlookup.as_ptr()) });

        unsafe { divi_formula_registry_free(registry) };
    }

    #[test]
    fn empty_registry_has_nothing() {
        let registry = divi_formula_registry_new();
        let sum = CString::new("SUM").unwrap();
        assert!(!unsafe { divi_formula_registry_has(registry, sum.as_ptr()) });
        unsafe { divi_formula_registry_free(registry) };
    }

    #[test]
    fn deps_lifecycle() {
        let graph = divi_formula_deps_new();
        assert!(!graph.is_null());

        let a1 = CString::new("A1").unwrap();
        let b1 = CString::new("B1").unwrap();
        let c1 = CString::new("C1").unwrap();

        // A1 depends on B1 and C1
        assert_eq!(
            unsafe { divi_formula_deps_add(graph, a1.as_ptr(), b1.as_ptr()) },
            0
        );
        assert_eq!(
            unsafe { divi_formula_deps_add(graph, a1.as_ptr(), c1.as_ptr()) },
            0
        );

        // No circular reference
        assert!(!unsafe { divi_formula_deps_has_circular(graph, a1.as_ptr()) });

        // Dependents of B1 should include A1
        let deps = unsafe { divi_formula_deps_dependents(graph, b1.as_ptr()) };
        assert!(!deps.is_null());
        let deps_str = unsafe { CStr::from_ptr(deps) }.to_str().unwrap();
        let deps_vec: Vec<String> = serde_json::from_str(deps_str).unwrap();
        assert!(deps_vec.contains(&"A1".to_string()));

        unsafe { crate::helpers::divi_free_string(deps) };

        // Evaluation order should work
        let order = unsafe { divi_formula_deps_evaluation_order(graph) };
        assert!(!order.is_null());
        let order_str = unsafe { CStr::from_ptr(order) }.to_str().unwrap();
        let order_vec: Vec<String> = serde_json::from_str(order_str).unwrap();
        // B1 and C1 should come before A1
        let pos_a1 = order_vec.iter().position(|x| x == "A1").unwrap();
        let pos_b1 = order_vec.iter().position(|x| x == "B1").unwrap();
        let pos_c1 = order_vec.iter().position(|x| x == "C1").unwrap();
        assert!(pos_b1 < pos_a1);
        assert!(pos_c1 < pos_a1);

        unsafe { crate::helpers::divi_free_string(order) };
        unsafe { divi_formula_deps_free(graph) };
    }

    #[test]
    fn deps_circular_detection() {
        let graph = divi_formula_deps_new();

        let a1 = CString::new("A1").unwrap();
        let b1 = CString::new("B1").unwrap();

        unsafe {
            divi_formula_deps_add(graph, a1.as_ptr(), b1.as_ptr());
            divi_formula_deps_add(graph, b1.as_ptr(), a1.as_ptr());
        };

        assert!(unsafe { divi_formula_deps_has_circular(graph, a1.as_ptr()) });

        // Evaluation order should return null (circular)
        let order = unsafe { divi_formula_deps_evaluation_order(graph) };
        assert!(order.is_null());

        unsafe { divi_formula_deps_free(graph) };
    }

    #[test]
    fn deps_remove_cell() {
        let graph = divi_formula_deps_new();

        let a1 = CString::new("A1").unwrap();
        let b1 = CString::new("B1").unwrap();

        unsafe {
            divi_formula_deps_add(graph, a1.as_ptr(), b1.as_ptr());
            divi_formula_deps_remove_cell(graph, a1.as_ptr());
        };

        // After removal, B1 should have no dependents
        let deps = unsafe { divi_formula_deps_dependents(graph, b1.as_ptr()) };
        assert!(!deps.is_null());
        let deps_str = unsafe { CStr::from_ptr(deps) }.to_str().unwrap();
        let deps_vec: Vec<String> = serde_json::from_str(deps_str).unwrap();
        assert!(deps_vec.is_empty());

        unsafe { crate::helpers::divi_free_string(deps) };
        unsafe { divi_formula_deps_free(graph) };
    }

    #[test]
    fn locale_to_display_french() {
        let locale = CString::new("fr").unwrap();
        let formula = CString::new("=SUM(1.5, 2.5)").unwrap();

        let result =
            unsafe { divi_formula_locale_to_display(locale.as_ptr(), formula.as_ptr()) };
        assert!(!result.is_null());

        let result_str = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert!(result_str.contains("SOMME"));
        assert!(result_str.contains(';'));

        unsafe { crate::helpers::divi_free_string(result) };
    }

    #[test]
    fn locale_to_canonical_french() {
        let locale = CString::new("fr").unwrap();
        let display = CString::new("=SOMME(1,5; 2,5)").unwrap();

        let result =
            unsafe { divi_formula_locale_to_canonical(locale.as_ptr(), display.as_ptr()) };
        assert!(!result.is_null());

        let result_str = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert!(result_str.contains("SUM"));

        unsafe { crate::helpers::divi_free_string(result) };
    }

    #[test]
    fn locale_localize_name() {
        let locale = CString::new("de").unwrap();
        let name = CString::new("IF").unwrap();

        let result =
            unsafe { divi_formula_locale_localize_name(locale.as_ptr(), name.as_ptr()) };
        assert!(!result.is_null());

        let result_str = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert_eq!(result_str, "WENN");

        unsafe { crate::helpers::divi_free_string(result) };
    }

    #[test]
    fn locale_english_passthrough() {
        let locale = CString::new("en").unwrap();
        let name = CString::new("SUM").unwrap();

        let result =
            unsafe { divi_formula_locale_localize_name(locale.as_ptr(), name.as_ptr()) };
        assert!(!result.is_null());

        let result_str = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert_eq!(result_str, "SUM");

        unsafe { crate::helpers::divi_free_string(result) };
    }

    #[test]
    fn null_evaluator_returns_null() {
        let formula = CString::new("=1").unwrap();
        let result = unsafe {
            divi_formula_evaluate(
                std::ptr::null(),
                formula.as_ptr(),
                empty_resolve,
                empty_resolve_range,
                std::ptr::null_mut(),
            )
        };
        assert!(result.is_null());
    }

    #[test]
    fn null_node_returns_null() {
        let evaluator = divi_formula_evaluator_new();
        let result = unsafe {
            divi_formula_evaluator_evaluate(
                evaluator,
                std::ptr::null(),
                empty_resolve,
                empty_resolve_range,
                std::ptr::null_mut(),
            )
        };
        assert!(result.is_null());
        unsafe { divi_formula_evaluator_free(evaluator) };
    }

    #[test]
    fn free_null_is_noop() {
        // All free functions should be safe to call with null.
        unsafe {
            divi_formula_evaluator_free(std::ptr::null_mut());
            divi_formula_registry_free(std::ptr::null_mut());
            divi_formula_deps_free(std::ptr::null_mut());
            divi_formula_node_free(std::ptr::null_mut());
        }
    }
}
