use std::sync::Arc;

use crate::{clear_last_error, set_last_error};

/// Shared async runtime for all crates that need async operations.
///
/// Not Globe-specific — any future async crate (Advisor streaming, etc.)
/// can share this runtime.
pub struct DiviRuntime {
    pub(crate) runtime: Arc<tokio::runtime::Runtime>,
}

/// Create a new multi-threaded async runtime.
///
/// Returns null if the runtime cannot be created (check `divi_last_error()`).
/// Free with `divi_runtime_free`.
#[unsafe(no_mangle)]
pub extern "C" fn divi_runtime_new() -> *mut DiviRuntime {
    clear_last_error();

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            set_last_error(format!("divi_runtime_new: failed to create tokio runtime: {e}"));
            return std::ptr::null_mut();
        }
    };

    Box::into_raw(Box::new(DiviRuntime {
        runtime: Arc::new(runtime),
    }))
}

/// Free the async runtime.
///
/// # Safety
/// `ptr` must be a valid pointer from `divi_runtime_new`, called exactly once.
/// All pools/handles using this runtime should be freed first.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn divi_runtime_free(ptr: *mut DiviRuntime) {
    if !ptr.is_null() {
        unsafe {
            drop(Box::from_raw(ptr));
        }
    }
}
