use crate::api::{parse_caller, LezIndexerHandle};
use crate::core::memory::{catch_unwind_ffi, catch_unwind_status, string_to_c_char};
use crate::core::policy::Action;
use crate::core::result::PointerResult;
use crate::core::status::OperationStatus;
use crate::core::types::FfiTxPage;
use std::ffi::{c_char, CStr};

/// Queries historical transaction digests for an account with pagination.
///
/// # Safety
/// `handle` must point to an open indexer. `caller_ptr` and `account_ptr` must point to 32-byte addresses.
/// `cursor` if non-null must point to a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn lez_query_transactions_by_account(
    handle: *mut LezIndexerHandle,
    caller_ptr: *const u8,
    account_ptr: *const u8,
    cursor: *const c_char,
    limit: usize,
    oldest_first: bool,
) -> PointerResult<FfiTxPage> {
    catch_unwind_ffi(|| {
        if handle.is_null() || account_ptr.is_null() {
            return PointerResult::null_pointer();
        }
        let caller = match parse_caller(caller_ptr) {
            Ok(c) => c,
            Err(st) => return PointerResult::from_error(st),
        };
        let indexer = unsafe { &*handle };
        if !indexer.policy.check_permission(&caller, Action::QueryRead) {
            return PointerResult::from_error(OperationStatus::PermissionDenied);
        }
        let mut acc_id = [0u8; 32];
        acc_id.copy_from_slice(unsafe { std::slice::from_raw_parts(account_ptr, 32) });
        let cursor_str = if !cursor.is_null() {
            match unsafe { CStr::from_ptr(cursor) }.to_str() {
                Ok(s) => Some(s),
                Err(_) => return PointerResult::from_error(OperationStatus::InvalidArgumentCursor),
            }
        } else {
            None
        };
        match indexer.query_transactions_by_account(
            &caller,
            &acc_id,
            cursor_str,
            limit,
            oldest_first,
        ) {
            Ok(page) => PointerResult::from_value(page),
            Err(st) => PointerResult::from_error(st),
        }
    })
}

/// Queries the storage schema definition as a JSON string.
///
/// # Safety
/// `handle` must point to an open indexer. `caller_ptr` must point to a 32-byte address.
#[no_mangle]
pub unsafe extern "C" fn lez_query_schema(
    handle: *mut LezIndexerHandle,
    caller_ptr: *const u8,
) -> PointerResult<c_char> {
    catch_unwind_ffi(|| {
        if handle.is_null() {
            return PointerResult::null_pointer();
        }
        let caller = match parse_caller(caller_ptr) {
            Ok(c) => c,
            Err(st) => return PointerResult::from_error(st),
        };
        let indexer = unsafe { &*handle };
        if !indexer.policy.check_permission(&caller, Action::QueryRead) {
            return PointerResult::from_error(OperationStatus::PermissionDenied);
        }
        let json_str = indexer.query_schema();
        let raw_c_char = string_to_c_char(&json_str);
        PointerResult {
            value: raw_c_char,
            status: OperationStatus::Ok,
        }
    })
}

/// Explicitly frees an active cursor token registered to the caller (Audit finding 1).
///
/// # Ownership & Free Ordering Protocol:
/// - This function unregisters the server-side cursor state registered to the caller in `CursorManager`.
/// - It borrows `cursor` (as `*const c_char`) and DOES NOT deallocate the string memory.
/// - The string memory is owned by `FfiTxPage` and is deallocated by `lez_free_tx_page`.
/// - Therefore, if calling this function, it MUST be called BEFORE `lez_free_tx_page(page)`.
///   Calling `lez_free_cursor` after `lez_free_tx_page` passes a dangling pointer and triggers UB.
///
/// # Safety
/// If `cursor` is non-null, it must point to a valid null-terminated C string.
/// Passing NULL for `cursor` or `handle` is safe and a no-op.
#[no_mangle]
pub unsafe extern "C" fn lez_free_cursor(
    handle: *mut LezIndexerHandle,
    caller_ptr: *const u8,
    cursor: *const c_char,
) -> OperationStatus {
    catch_unwind_status(|| {
        if handle.is_null() || cursor.is_null() {
            return OperationStatus::Ok;
        }
        let caller = match parse_caller(caller_ptr) {
            Ok(c) => c,
            Err(st) => return st,
        };
        let cursor_str = match unsafe { CStr::from_ptr(cursor) }.to_str() {
            Ok(s) => s,
            Err(_) => return OperationStatus::InvalidArgumentCursor,
        };
        let indexer = unsafe { &*handle };
        indexer.free_cursor(&caller, cursor_str)
    })
}
