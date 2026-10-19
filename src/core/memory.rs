use std::ffi::{c_char, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use crate::core::result::PointerResult;
use crate::core::status::OperationStatus;

/// Safely executes an FFI closure, shielding host processes from unwinding panics (F#64).
pub fn catch_unwind_ffi<T, F>(f: F) -> PointerResult<T>
where
    F: FnOnce() -> PointerResult<T>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(_) => PointerResult::panic_caught(),
    }
}

/// Safely executes an FFI closure returning an OperationStatus.
pub fn catch_unwind_status<F>(f: F) -> OperationStatus
where
    F: FnOnce() -> OperationStatus,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(status) => status,
        Err(_) => OperationStatus::PanicCaught,
    }
}

/// Allocates an owned C string from a Rust string slice.
pub fn string_to_c_char(s: &str) -> *mut c_char {
    match CString::new(s) {
        Ok(c_str) => c_str.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Deallocates an owned C string allocated by lez-node-ffi-poc.
///
/// # Safety
/// If `ptr` is non-null, it must point to a valid C string created via `into_raw`. Passing NULL is safe and a no-op.
#[no_mangle]
pub unsafe extern "C" fn lez_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(unsafe { CString::from_raw(ptr) });
    }
}
