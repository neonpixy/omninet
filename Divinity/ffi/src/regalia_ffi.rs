//! Regalia FFI — C bindings for Omnidea's design language.
//!
//! Exposes Regalia's theming, layout, gradients, animation curves,
//! material presets, and accessibility helpers to any C ABI consumer.

use std::collections::HashMap;
use std::ffi::c_char;
use std::sync::Mutex;

use regalia::aura::Aura;
use regalia::component_style::ComponentStyle;
use regalia::crown_jewels::{contrast_ratio, relative_luminance, Material};
use regalia::domain::{Arbiter, Clansman, MockClansman};
use regalia::formation::{ColumnAlignment, ColumnJustification, FormationKind};
use regalia::insignia::{BorderInsets, SanctumID};
use regalia::reign::{Aspect, Reign};
use regalia::{FacetStyle, FacetStyleDelta, Gradient, Sanctum, Shift, ThemeCollection};

use crate::helpers::{c_str_to_str, json_to_c, lock_or_recover};
use crate::{clear_last_error, set_last_error};

/// Get the default Reign theme as JSON.
///
/// Returns a JSON string (caller must free via `divi_free_string`),
/// or null on error (check `divi_last_error`).
#[unsafe(no_mangle)]
pub extern "C" fn divi_regalia_default_reign() -> *mut c_char {
    let reign = Reign::default();
    json_to_c(&reign)
}

/// Resolve the Crest (color palette) for a given aspect ("light" or "dark").
///
/// `reign_json` — JSON-encoded Reign. If null, uses default.
/// `aspect` — "light", "dark", or a custom string.
///
/// Returns a JSON-encoded Crest, or null on error.
#[unsafe(no_mangle)]
pub extern "C" fn divi_regalia_resolve_crest(
    reign_json: *const c_char,
    aspect: *const c_char,
) -> *mut c_char {
    let aspect_str = match c_str_to_str(aspect) {
        Some(s) => s,
        None => {
            set_last_error("aspect is null or invalid UTF-8");
            return std::ptr::null_mut();
        }
    };

    let reign: Reign = if reign_json.is_null() {
        Reign::default()
    } else {
        match c_str_to_str(reign_json) {
            Some(s) => match serde_json::from_str(s) {
                Ok(r) => r,
                Err(e) => {
                    set_last_error(format!("Failed to parse Reign JSON: {e}"));
                    return std::ptr::null_mut();
                }
            },
            None => {
                set_last_error("reign_json is invalid UTF-8");
                return std::ptr::null_mut();
            }
        }
    };

    let asp = match aspect_str {
        "light" => Aspect::light(),
        "dark" => Aspect::dark(),
        other => Aspect::custom(other),
    };

    let crest = reign.aura.crest(&asp);
    json_to_c(crest)
}

/// Run the Arbiter layout solver.
///
/// `x`, `y`, `w`, `h` — viewport bounds.
/// `sanctums_json` — JSON array of Sanctum objects.
///
/// Returns a JSON-encoded Domain (resolved layout), or null on error.
#[unsafe(no_mangle)]
pub extern "C" fn divi_regalia_resolve_layout(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    sanctums_json: *const c_char,
) -> *mut c_char {
    let sanctums_str = match c_str_to_str(sanctums_json) {
        Some(s) => s,
        None => {
            set_last_error("sanctums_json is null or invalid UTF-8");
            return std::ptr::null_mut();
        }
    };

    let sanctums: Vec<Sanctum> = match serde_json::from_str(sanctums_str) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("Failed to parse sanctums JSON: {e}"));
            return std::ptr::null_mut();
        }
    };

    // Create a mock clansman for each sanctum so the Arbiter has something to resolve.
    let mut vassals: HashMap<SanctumID, Vec<&dyn Clansman>> = HashMap::new();
    let mock = MockClansman::named("pulse-content", None);
    for sanctum in &sanctums {
        if sanctum.border.is_none() {
            // Content sanctum gets a child
            vassals.insert(sanctum.id.clone(), vec![&mock as &dyn Clansman]);
        }
    }

    let insets: HashMap<SanctumID, BorderInsets> = HashMap::new();
    let bounds = (x, y, w, h);

    match Arbiter::resolve(bounds, &sanctums, &vassals, &insets, None) {
        Ok(domain) => json_to_c(&domain),
        Err(e) => {
            set_last_error(format!("Arbiter resolve failed: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Get the Pulse dashboard sanctum definitions as JSON.
///
/// Returns a JSON array of Sanctum objects (toolbar + sidebar + content).
/// Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_regalia_pulse_sanctums() -> *mut c_char {
    let sanctums = vec![
        Sanctum::toolbar(Some(52.0), None),
        Sanctum::sidebar(Some(220.0), Some(FormationKind::Column {
            spacing: 4.0,
            alignment: ColumnAlignment::Leading,
            justification: ColumnJustification::Top,
        })),
        Sanctum::content(Some(FormationKind::Column {
            spacing: 16.0,
            alignment: ColumnAlignment::Leading,
            justification: ColumnJustification::Top,
        })),
    ];
    json_to_c(&sanctums)
}

// ===================================================================
// ThemeCollection — opaque pointer (multi-theme management)
// ===================================================================

pub struct RegaliaThemeCollection(pub(crate) Mutex<ThemeCollection>);

/// Create a new theme collection with an initial Reign.
///
/// `reign_json` — JSON-encoded Reign. If null, uses `Reign::default()`.
/// Free with `divi_regalia_theme_collection_free`.
///
/// # Safety
/// `reign_json` may be null (uses default).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_regalia_theme_collection_new(
    reign_json: *const c_char,
) -> *mut RegaliaThemeCollection {
    let reign: Reign = if reign_json.is_null() {
        Reign::default()
    } else if let Some(rj) = c_str_to_str(reign_json) {
        serde_json::from_str(rj).unwrap_or_default()
    } else {
        Reign::default()
    };

    Box::into_raw(Box::new(RegaliaThemeCollection(Mutex::new(
        ThemeCollection::new(reign),
    ))))
}

/// Free a theme collection.
///
/// # Safety
/// `ptr` must be valid, called exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_regalia_theme_collection_free(
    ptr: *mut RegaliaThemeCollection,
) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}

/// Get the active Reign as JSON.
///
/// Returns JSON (Reign). Caller must free via `divi_free_string`.
///
/// # Safety
/// `tc` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_regalia_theme_collection_active(
    tc: *const RegaliaThemeCollection,
) -> *mut c_char {
    let tc = unsafe { &*tc };
    let guard = lock_or_recover(&tc.0);
    json_to_c(guard.active())
}

/// Switch to a different theme by name.
///
/// Returns 0 on success, -1 on error (theme not found).
///
/// # Safety
/// `tc` must be a valid pointer. `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_regalia_theme_collection_switch(
    tc: *const RegaliaThemeCollection,
    name: *const c_char,
) -> i32 {
    clear_last_error();

    let tc = unsafe { &*tc };
    let Some(n) = c_str_to_str(name) else {
        set_last_error("divi_regalia_theme_collection_switch: invalid name");
        return -1;
    };

    let mut guard = lock_or_recover(&tc.0);
    match guard.switch(n) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Add a new theme to the collection.
///
/// `reign_json` — JSON-encoded Reign.
/// Returns 0 on success, -1 on error (duplicate name).
///
/// # Safety
/// `tc` must be a valid pointer. `reign_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_regalia_theme_collection_add(
    tc: *const RegaliaThemeCollection,
    reign_json: *const c_char,
) -> i32 {
    clear_last_error();

    let tc = unsafe { &*tc };
    let Some(rj) = c_str_to_str(reign_json) else {
        set_last_error("divi_regalia_theme_collection_add: invalid reign_json");
        return -1;
    };

    let reign: Reign = match serde_json::from_str(rj) {
        Ok(r) => r,
        Err(e) => {
            set_last_error(format!("divi_regalia_theme_collection_add: {e}"));
            return -1;
        }
    };

    let mut guard = lock_or_recover(&tc.0);
    match guard.add(reign) {
        Ok(()) => 0,
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Remove a theme by name.
///
/// Returns JSON (the removed Reign). Caller must free via `divi_free_string`.
/// Returns null on error (cannot remove active, last, or nonexistent).
///
/// # Safety
/// `tc` must be a valid pointer. `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_regalia_theme_collection_remove(
    tc: *const RegaliaThemeCollection,
    name: *const c_char,
) -> *mut c_char {
    clear_last_error();

    let tc = unsafe { &*tc };
    let Some(n) = c_str_to_str(name) else {
        set_last_error("divi_regalia_theme_collection_remove: invalid name");
        return std::ptr::null_mut();
    };

    let mut guard = lock_or_recover(&tc.0);
    match guard.remove(n) {
        Ok(reign) => json_to_c(&reign),
        Err(e) => {
            set_last_error(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// List all theme names in the collection.
///
/// Returns JSON array of strings. Caller must free via `divi_free_string`.
///
/// # Safety
/// `tc` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_regalia_theme_collection_list(
    tc: *const RegaliaThemeCollection,
) -> *mut c_char {
    let tc = unsafe { &*tc };
    let guard = lock_or_recover(&tc.0);
    let names = guard.list();
    json_to_c(&names)
}

// ===================================================================
// Stateless helpers — component styles, gradients, surge, materials
// ===================================================================

/// Look up a ComponentStyle preset by name.
///
/// Supported names: "primary_button", "card", "input_field", "text_body".
/// Returns JSON (ComponentStyle). Caller must free via `divi_free_string`.
/// Returns null if the name is unknown.
///
/// # Safety
/// `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_regalia_component_style_preset(
    name: *const c_char,
) -> *mut c_char {
    let Some(n) = c_str_to_str(name) else {
        set_last_error("divi_regalia_component_style_preset: invalid name");
        return std::ptr::null_mut();
    };

    let style = match n {
        "primary_button" => ComponentStyle::primary_button(),
        "card" => ComponentStyle::card(),
        "input_field" => ComponentStyle::input_field(),
        "text_body" => ComponentStyle::text_body(),
        other => {
            set_last_error(format!(
                "divi_regalia_component_style_preset: unknown preset '{other}'"
            ));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&style)
}

/// Look up a Gradient preset by name.
///
/// Supported names: "sunset", "ocean", "forest".
/// Returns JSON (Gradient). Caller must free via `divi_free_string`.
/// Returns null if the name is unknown.
///
/// # Safety
/// `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_regalia_gradient_preset(
    name: *const c_char,
) -> *mut c_char {
    let Some(n) = c_str_to_str(name) else {
        set_last_error("divi_regalia_gradient_preset: invalid name");
        return std::ptr::null_mut();
    };

    let gradient = match n {
        "sunset" => Gradient::sunset(),
        "ocean" => Gradient::ocean(),
        "forest" => Gradient::forest(),
        other => {
            set_last_error(format!(
                "divi_regalia_gradient_preset: unknown preset '{other}'"
            ));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&gradient)
}

/// Interpolate the color at a given position along a gradient.
///
/// `gradient_json` — JSON-encoded Gradient.
/// `position` — 0.0 to 1.0 along the gradient.
///
/// Returns JSON (Ember). Caller must free via `divi_free_string`.
/// Returns null on error.
///
/// # Safety
/// `gradient_json` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_regalia_gradient_color_at(
    gradient_json: *const c_char,
    position: f64,
) -> *mut c_char {
    let Some(gj) = c_str_to_str(gradient_json) else {
        set_last_error("divi_regalia_gradient_color_at: invalid gradient_json");
        return std::ptr::null_mut();
    };

    let gradient: Gradient = match serde_json::from_str(gj) {
        Ok(g) => g,
        Err(e) => {
            set_last_error(format!("divi_regalia_gradient_color_at: {e}"));
            return std::ptr::null_mut();
        }
    };

    let color = gradient.color_at(position);
    json_to_c(&color)
}

/// Evaluate a Surge animation curve at time `t`.
///
/// Supported preset names: "snap", "smooth", "bouncy".
/// Returns the curve value at `t` (typically 0.0 at start, 1.0 at end).
/// Returns NaN on error (unknown preset or invalid name).
///
/// # Safety
/// `preset_name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_regalia_surge_evaluate(
    preset_name: *const c_char,
    t: f64,
) -> f64 {
    let Some(n) = c_str_to_str(preset_name) else {
        set_last_error("divi_regalia_surge_evaluate: invalid preset_name");
        return f64::NAN;
    };

    let shift = match n {
        "snap" => Shift::snap(),
        "smooth" => Shift::smooth(),
        "bouncy" => Shift::bouncy(),
        other => {
            set_last_error(format!(
                "divi_regalia_surge_evaluate: unknown preset '{other}'"
            ));
            return f64::NAN;
        }
    };

    shift.value(t)
}

/// Look up a FacetStyle preset by name.
///
/// Supported names: "regular", "clear", "subtle", "frosted".
/// Returns JSON (FacetStyle). Caller must free via `divi_free_string`.
/// Returns null if the name is unknown.
///
/// # Safety
/// `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_regalia_facet_style_preset(
    name: *const c_char,
) -> *mut c_char {
    let Some(n) = c_str_to_str(name) else {
        set_last_error("divi_regalia_facet_style_preset: invalid name");
        return std::ptr::null_mut();
    };

    let style = match n {
        "regular" => FacetStyle::regular(),
        "clear" => FacetStyle::clear(),
        "subtle" => FacetStyle::subtle(),
        "frosted" => FacetStyle::frosted(),
        other => {
            set_last_error(format!(
                "divi_regalia_facet_style_preset: unknown preset '{other}'"
            ));
            return std::ptr::null_mut();
        }
    };

    json_to_c(&style)
}

/// Apply a delta to a FacetStyle, returning the modified style.
///
/// `style_json` — JSON-encoded FacetStyle.
/// `delta_json` — JSON-encoded FacetStyleDelta.
///
/// Returns JSON (FacetStyle). Caller must free via `divi_free_string`.
/// Returns null on error.
///
/// # Safety
/// Both parameters must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_regalia_facet_style_apply_delta(
    style_json: *const c_char,
    delta_json: *const c_char,
) -> *mut c_char {
    let Some(sj) = c_str_to_str(style_json) else {
        set_last_error("divi_regalia_facet_style_apply_delta: invalid style_json");
        return std::ptr::null_mut();
    };

    let Some(dj) = c_str_to_str(delta_json) else {
        set_last_error("divi_regalia_facet_style_apply_delta: invalid delta_json");
        return std::ptr::null_mut();
    };

    let style: FacetStyle = match serde_json::from_str(sj) {
        Ok(s) => s,
        Err(e) => {
            set_last_error(format!("divi_regalia_facet_style_apply_delta: style: {e}"));
            return std::ptr::null_mut();
        }
    };

    let delta: FacetStyleDelta = match serde_json::from_str(dj) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("divi_regalia_facet_style_apply_delta: delta: {e}"));
            return std::ptr::null_mut();
        }
    };

    let result = style.applying(&delta);
    json_to_c(&result)
}

/// Compute the WCAG contrast ratio between two sRGB colors.
///
/// All channel values are in the 0.0–1.0 range.
/// Returns a ratio >= 1.0 (e.g. 21.0 for black vs white).
#[unsafe(no_mangle)]
pub extern "C" fn divi_regalia_contrast_ratio(
    r1: f64,
    g1: f64,
    b1: f64,
    r2: f64,
    g2: f64,
    b2: f64,
) -> f64 {
    let lum1 = relative_luminance(r1, g1, b1);
    let lum2 = relative_luminance(r2, g2, b2);
    contrast_ratio(lum1, lum2)
}

/// Get the default Aura (design token container) as JSON.
///
/// Returns JSON (Aura). Caller must free via `divi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_regalia_default_aura() -> *mut c_char {
    json_to_c(&Aura::default())
}
