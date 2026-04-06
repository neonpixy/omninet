//! Nexus FFI — export, import, and protocol bridge registries.
//!
//! Exposes the three Nexus registries as opaque pointers. Each is created
//! via `_with_defaults()` (all built-in plugins) or `_new()` (empty).
//!
//! ## Export workflow
//! 1. `divi_exporter_registry_with_defaults()` → `*mut ExporterRegistry`
//! 2. `divi_export(registry, digits_json, root_id, config_json)` → bytes + len
//! 3. `divi_free_bytes(ptr, len)` to free the output
//! 4. `divi_exporter_registry_free(registry)` when done
//!
//! ## Import workflow
//! 1. `divi_importer_registry_with_defaults()` → `*mut ImporterRegistry`
//! 2. `divi_import(registry, data, len, mime_type, config_json)` → JSON
//! 3. `divi_free_string(json)` to free the output
//! 4. `divi_importer_registry_free(registry)` when done

use std::ffi::c_char;

use crate::helpers::{bytes_to_owned, c_str_to_str, json_to_c, string_to_c};
use crate::{clear_last_error, set_last_error};

use nexus::{BridgeRegistry, ExporterRegistry, ImporterRegistry};

// ---------------------------------------------------------------------------
// ExporterRegistry — opaque pointer lifecycle
// ---------------------------------------------------------------------------

/// Create an exporter registry with all built-in exporters (15 formats).
#[unsafe(no_mangle)]
pub extern "C" fn divi_exporter_registry_with_defaults() -> *mut ExporterRegistry {
    clear_last_error();
    Box::into_raw(Box::new(ExporterRegistry::with_defaults()))
}

/// Create an empty exporter registry.
#[unsafe(no_mangle)]
pub extern "C" fn divi_exporter_registry_new() -> *mut ExporterRegistry {
    clear_last_error();
    Box::into_raw(Box::new(ExporterRegistry::new()))
}

/// Free an exporter registry.
///
/// # Safety
/// `ptr` must have been returned by `divi_exporter_registry_*` or be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_exporter_registry_free(ptr: *mut ExporterRegistry) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr)) };
    }
}

/// Number of registered exporters.
///
/// # Safety
/// `ptr` must be a valid `ExporterRegistry`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_exporter_registry_count(ptr: *const ExporterRegistry) -> i32 {
    clear_last_error();
    if ptr.is_null() {
        set_last_error("divi_exporter_registry_count: null pointer");
        return -1;
    }
    let reg = unsafe { &*ptr };
    reg.count() as i32
}

/// List all registered exporter IDs as a JSON array of strings.
///
/// # Safety
/// `ptr` must be a valid `ExporterRegistry`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_exporter_registry_list(
    ptr: *const ExporterRegistry,
) -> *mut c_char {
    clear_last_error();
    if ptr.is_null() {
        set_last_error("divi_exporter_registry_list: null pointer");
        return std::ptr::null_mut();
    }
    let reg = unsafe { &*ptr };
    let ids = reg.list();
    json_to_c(&ids)
}

/// List all supported export formats as a JSON array of strings.
///
/// # Safety
/// `ptr` must be a valid `ExporterRegistry`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_exporter_registry_supported_formats(
    ptr: *const ExporterRegistry,
) -> *mut c_char {
    clear_last_error();
    if ptr.is_null() {
        set_last_error("divi_exporter_registry_supported_formats: null pointer");
        return std::ptr::null_mut();
    }
    let reg = unsafe { &*ptr };
    let formats = reg.supported_formats();
    json_to_c(&formats)
}

/// Export digits to the format specified in the config.
///
/// Returns a byte buffer via `out_ptr` and `out_len`. Caller must free with
/// `divi_free_bytes`. Also returns filename and mime_type as JSON via the
/// return value (free with `divi_free_string`).
///
/// `digits_json`: JSON array of serialized Digits.
/// `root_id`: optional UUID string (null for none).
/// `config_json`: JSON-serialized `ExportConfig`.
///
/// Returns JSON `{"filename":"...","mime_type":"..."}` on success, null on error.
///
/// # Safety
/// All pointers must be valid. `out_ptr` and `out_len` must be non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_export(
    registry: *const ExporterRegistry,
    digits_json: *const c_char,
    root_id: *const c_char,
    config_json: *const c_char,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_export";

    if registry.is_null() || out_ptr.is_null() || out_len.is_null() {
        set_last_error(format!("{fn_name}: null pointer argument"));
        return std::ptr::null_mut();
    }

    let reg = unsafe { &*registry };

    let digits_str = match c_str_to_str(digits_json) {
        Some(s) => s,
        None => {
            set_last_error(format!("{fn_name}: invalid digits_json"));
            return std::ptr::null_mut();
        }
    };

    let digits: Vec<ideas::Digit> = match serde_json::from_str(digits_str) {
        Ok(d) => d,
        Err(e) => {
            set_last_error(format!("{fn_name}: failed to parse digits: {e}"));
            return std::ptr::null_mut();
        }
    };

    let root_uuid = c_str_to_str(root_id).and_then(|s| uuid::Uuid::parse_str(s).ok());

    let config_str = match c_str_to_str(config_json) {
        Some(s) => s,
        None => {
            set_last_error(format!("{fn_name}: invalid config_json"));
            return std::ptr::null_mut();
        }
    };

    let config: nexus::ExportConfig = match serde_json::from_str(config_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("{fn_name}: failed to parse config: {e}"));
            return std::ptr::null_mut();
        }
    };

    match reg.export(&digits, root_uuid, &config) {
        Ok(output) => {
            let (ptr, len) = bytes_to_owned(output.data);
            unsafe {
                *out_ptr = ptr;
                *out_len = len;
            }

            #[derive(serde::Serialize)]
            struct ExportMeta {
                filename: String,
                mime_type: String,
            }
            json_to_c(&ExportMeta {
                filename: output.filename,
                mime_type: output.mime_type,
            })
        }
        Err(e) => {
            set_last_error(format!("{fn_name}: {e}"));
            std::ptr::null_mut()
        }
    }
}

// ---------------------------------------------------------------------------
// ImporterRegistry — opaque pointer lifecycle
// ---------------------------------------------------------------------------

/// Create an importer registry with all built-in importers (7 formats).
#[unsafe(no_mangle)]
pub extern "C" fn divi_importer_registry_with_defaults() -> *mut ImporterRegistry {
    clear_last_error();
    Box::into_raw(Box::new(ImporterRegistry::with_defaults()))
}

/// Create an empty importer registry.
#[unsafe(no_mangle)]
pub extern "C" fn divi_importer_registry_new() -> *mut ImporterRegistry {
    clear_last_error();
    Box::into_raw(Box::new(ImporterRegistry::new()))
}

/// Free an importer registry.
///
/// # Safety
/// `ptr` must have been returned by `divi_importer_registry_*` or be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_importer_registry_free(ptr: *mut ImporterRegistry) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr)) };
    }
}

/// Number of registered importers.
///
/// # Safety
/// `ptr` must be a valid `ImporterRegistry`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_importer_registry_count(ptr: *const ImporterRegistry) -> i32 {
    clear_last_error();
    if ptr.is_null() {
        set_last_error("divi_importer_registry_count: null pointer");
        return -1;
    }
    let reg = unsafe { &*ptr };
    reg.count() as i32
}

/// List all registered importer IDs as a JSON array of strings.
///
/// # Safety
/// `ptr` must be a valid `ImporterRegistry`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_importer_registry_list(
    ptr: *const ImporterRegistry,
) -> *mut c_char {
    clear_last_error();
    if ptr.is_null() {
        set_last_error("divi_importer_registry_list: null pointer");
        return std::ptr::null_mut();
    }
    let reg = unsafe { &*ptr };
    let ids = reg.list();
    json_to_c(&ids)
}

/// Import data from a file into Ideas digits.
///
/// `data` + `data_len`: raw file bytes.
/// `mime_type`: MIME type string (e.g., "text/csv", "application/pdf").
/// `config_json`: JSON-serialized `ImportConfig`.
///
/// Returns JSON-serialized `ImportOutput` on success (free with `divi_free_string`).
/// Returns null on error.
///
/// # Safety
/// All pointers must be valid. `data` must point to `data_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_import(
    registry: *const ImporterRegistry,
    data: *const u8,
    data_len: usize,
    mime_type: *const c_char,
    config_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_import";

    if registry.is_null() || data.is_null() {
        set_last_error(format!("{fn_name}: null pointer argument"));
        return std::ptr::null_mut();
    }

    let reg = unsafe { &*registry };
    let bytes = unsafe { std::slice::from_raw_parts(data, data_len) };

    let mime = match c_str_to_str(mime_type) {
        Some(s) => s,
        None => {
            set_last_error(format!("{fn_name}: invalid mime_type"));
            return std::ptr::null_mut();
        }
    };

    let config_str = match c_str_to_str(config_json) {
        Some(s) => s,
        None => {
            set_last_error(format!("{fn_name}: invalid config_json"));
            return std::ptr::null_mut();
        }
    };

    let config: nexus::ImportConfig = match serde_json::from_str(config_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("{fn_name}: failed to parse config: {e}"));
            return std::ptr::null_mut();
        }
    };

    match reg.import(bytes, mime, &config) {
        Ok(output) => json_to_c(&output),
        Err(e) => {
            set_last_error(format!("{fn_name}: {e}"));
            std::ptr::null_mut()
        }
    }
}

// ---------------------------------------------------------------------------
// BridgeRegistry — opaque pointer lifecycle
// ---------------------------------------------------------------------------

/// Create a bridge registry with all built-in bridges (SMTP).
#[unsafe(no_mangle)]
pub extern "C" fn divi_bridge_registry_with_defaults() -> *mut BridgeRegistry {
    clear_last_error();
    Box::into_raw(Box::new(BridgeRegistry::with_defaults()))
}

/// Create an empty bridge registry.
#[unsafe(no_mangle)]
pub extern "C" fn divi_bridge_registry_new() -> *mut BridgeRegistry {
    clear_last_error();
    Box::into_raw(Box::new(BridgeRegistry::new()))
}

/// Free a bridge registry.
///
/// # Safety
/// `ptr` must have been returned by `divi_bridge_registry_*` or be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bridge_registry_free(ptr: *mut BridgeRegistry) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr)) };
    }
}

/// Number of registered bridges.
///
/// # Safety
/// `ptr` must be a valid `BridgeRegistry`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bridge_registry_count(ptr: *const BridgeRegistry) -> i32 {
    clear_last_error();
    if ptr.is_null() {
        set_last_error("divi_bridge_registry_count: null pointer");
        return -1;
    }
    let reg = unsafe { &*ptr };
    reg.count() as i32
}

/// List all registered bridge IDs as a JSON array of strings.
///
/// # Safety
/// `ptr` must be a valid `BridgeRegistry`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bridge_registry_list(
    ptr: *const BridgeRegistry,
) -> *mut c_char {
    clear_last_error();
    if ptr.is_null() {
        set_last_error("divi_bridge_registry_list: null pointer");
        return std::ptr::null_mut();
    }
    let reg = unsafe { &*ptr };
    let ids = reg.list();
    json_to_c(&ids)
}

/// Bridge an Equipment MailMessage to an external protocol.
///
/// `message_json`: JSON-serialized `MailMessage`.
/// `config_json`: JSON-serialized `BridgeConfig`.
///
/// Returns JSON-serialized `BridgeResult` on success (free with `divi_free_string`).
/// Returns null on error.
///
/// # Safety
/// All pointers must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_bridge(
    registry: *const BridgeRegistry,
    message_json: *const c_char,
    config_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let fn_name = "divi_bridge";

    if registry.is_null() {
        set_last_error(format!("{fn_name}: null pointer argument"));
        return std::ptr::null_mut();
    }

    let reg = unsafe { &*registry };

    let msg_str = match c_str_to_str(message_json) {
        Some(s) => s,
        None => {
            set_last_error(format!("{fn_name}: invalid message_json"));
            return std::ptr::null_mut();
        }
    };

    let message: equipment::MailMessage = match serde_json::from_str(msg_str) {
        Ok(m) => m,
        Err(e) => {
            set_last_error(format!("{fn_name}: failed to parse message: {e}"));
            return std::ptr::null_mut();
        }
    };

    let config_str = match c_str_to_str(config_json) {
        Some(s) => s,
        None => {
            set_last_error(format!("{fn_name}: invalid config_json"));
            return std::ptr::null_mut();
        }
    };

    let config: nexus::BridgeConfig = match serde_json::from_str(config_str) {
        Ok(c) => c,
        Err(e) => {
            set_last_error(format!("{fn_name}: failed to parse config: {e}"));
            return std::ptr::null_mut();
        }
    };

    match reg.bridge(&message, &config) {
        Ok(result) => json_to_c(&result),
        Err(e) => {
            set_last_error(format!("{fn_name}: {e}"));
            std::ptr::null_mut()
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience: format info
// ---------------------------------------------------------------------------

/// Get the file extension for an export format (e.g., "pdf", "docx").
///
/// `format_json`: JSON string of the format name (e.g., `"pdf"`).
/// Returns the extension string. Caller must free with `divi_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_export_format_extension(
    format_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let s = match c_str_to_str(format_json) {
        Some(s) => s,
        None => {
            set_last_error("divi_export_format_extension: invalid format");
            return std::ptr::null_mut();
        }
    };
    let format: nexus::ExportFormat = match serde_json::from_str(&format!("\"{s}\"")) {
        Ok(f) => f,
        Err(e) => {
            set_last_error(format!("divi_export_format_extension: unknown format: {e}"));
            return std::ptr::null_mut();
        }
    };
    string_to_c(format.extension().to_string())
}

/// Get the MIME type for an export format (e.g., "application/pdf").
///
/// `format_json`: format name string (e.g., "pdf").
/// Returns the MIME type string. Caller must free with `divi_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_export_format_mime_type(
    format_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let s = match c_str_to_str(format_json) {
        Some(s) => s,
        None => {
            set_last_error("divi_export_format_mime_type: invalid format");
            return std::ptr::null_mut();
        }
    };
    let format: nexus::ExportFormat = match serde_json::from_str(&format!("\"{s}\"")) {
        Ok(f) => f,
        Err(e) => {
            set_last_error(format!("divi_export_format_mime_type: unknown format: {e}"));
            return std::ptr::null_mut();
        }
    };
    string_to_c(format.mime_type().to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn exporter_registry_lifecycle() {
        let reg = divi_exporter_registry_with_defaults();
        assert!(!reg.is_null());

        let count = unsafe { divi_exporter_registry_count(reg) };
        assert_eq!(count, 15);

        let list_ptr = unsafe { divi_exporter_registry_list(reg) };
        assert!(!list_ptr.is_null());
        let list_str = unsafe { CStr::from_ptr(list_ptr) }.to_str().unwrap();
        let ids: Vec<String> = serde_json::from_str(list_str).unwrap();
        assert_eq!(ids.len(), 15);
        unsafe { crate::helpers::divi_free_string(list_ptr) };

        unsafe { divi_exporter_registry_free(reg) };
    }

    #[test]
    fn importer_registry_lifecycle() {
        let reg = divi_importer_registry_with_defaults();
        assert!(!reg.is_null());

        let count = unsafe { divi_importer_registry_count(reg) };
        assert_eq!(count, 7);

        unsafe { divi_importer_registry_free(reg) };
    }

    #[test]
    fn bridge_registry_lifecycle() {
        let reg = divi_bridge_registry_with_defaults();
        assert!(!reg.is_null());

        let count = unsafe { divi_bridge_registry_count(reg) };
        assert_eq!(count, 1);

        unsafe { divi_bridge_registry_free(reg) };
    }

    #[test]
    fn export_json_via_ffi() {
        let reg = divi_exporter_registry_with_defaults();

        let digit = ideas::Digit::new("text".into(), x::Value::from("Hello FFI"), "cpub1test".into()).unwrap();
        let digits_json = serde_json::to_string(&vec![digit]).unwrap();
        let config = nexus::ExportConfig::new(nexus::ExportFormat::Json);
        let config_json = serde_json::to_string(&config).unwrap();

        let digits_c = std::ffi::CString::new(digits_json).unwrap();
        let config_c = std::ffi::CString::new(config_json).unwrap();

        let mut out_ptr: *mut u8 = std::ptr::null_mut();
        let mut out_len: usize = 0;

        let meta_ptr = unsafe {
            divi_export(
                reg,
                digits_c.as_ptr(),
                std::ptr::null(),
                config_c.as_ptr(),
                &mut out_ptr,
                &mut out_len,
            )
        };

        assert!(!meta_ptr.is_null(), "export should succeed");
        assert!(!out_ptr.is_null());
        assert!(out_len > 0);

        let meta_str = unsafe { CStr::from_ptr(meta_ptr) }.to_str().unwrap();
        assert!(meta_str.contains("export.json"));

        unsafe {
            crate::helpers::divi_free_string(meta_ptr);
            crate::helpers::divi_free_bytes(out_ptr, out_len);
            divi_exporter_registry_free(reg);
        }
    }

    #[test]
    fn import_csv_via_ffi() {
        let reg = divi_importer_registry_with_defaults();

        let csv_data = b"Name,Age\nAlice,30\nBob,25";
        let mime = std::ffi::CString::new("text/csv").unwrap();
        let config = nexus::ImportConfig::new("cpub1test");
        let config_json = std::ffi::CString::new(serde_json::to_string(&config).unwrap()).unwrap();

        let result_ptr = unsafe {
            divi_import(
                reg,
                csv_data.as_ptr(),
                csv_data.len(),
                mime.as_ptr(),
                config_json.as_ptr(),
            )
        };

        assert!(!result_ptr.is_null(), "import should succeed");
        let result_str = unsafe { CStr::from_ptr(result_ptr) }.to_str().unwrap();
        assert!(result_str.contains("digits"));

        unsafe {
            crate::helpers::divi_free_string(result_ptr);
            divi_importer_registry_free(reg);
        }
    }

    #[test]
    fn format_extension_and_mime() {
        let pdf = std::ffi::CString::new("pdf").unwrap();

        let ext_ptr = unsafe { divi_export_format_extension(pdf.as_ptr()) };
        assert!(!ext_ptr.is_null());
        let ext = unsafe { CStr::from_ptr(ext_ptr) }.to_str().unwrap();
        assert_eq!(ext, "pdf");
        unsafe { crate::helpers::divi_free_string(ext_ptr) };

        let mime_ptr = unsafe { divi_export_format_mime_type(pdf.as_ptr()) };
        assert!(!mime_ptr.is_null());
        let mime = unsafe { CStr::from_ptr(mime_ptr) }.to_str().unwrap();
        assert_eq!(mime, "application/pdf");
        unsafe { crate::helpers::divi_free_string(mime_ptr) };
    }

    #[test]
    fn null_safety() {
        assert_eq!(unsafe { divi_exporter_registry_count(std::ptr::null()) }, -1);
        assert_eq!(unsafe { divi_importer_registry_count(std::ptr::null()) }, -1);
        assert_eq!(unsafe { divi_bridge_registry_count(std::ptr::null()) }, -1);
        assert!(unsafe { divi_exporter_registry_list(std::ptr::null()) }.is_null());
        assert!(unsafe { divi_importer_registry_list(std::ptr::null()) }.is_null());
        assert!(unsafe { divi_bridge_registry_list(std::ptr::null()) }.is_null());
    }
}
