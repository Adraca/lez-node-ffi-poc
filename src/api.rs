use std::ffi::{c_char, CStr};
use std::path::Path;
use crate::core::memory::catch_unwind_ffi;
use crate::core::policy::Action;
use crate::core::result::PointerResult;
use crate::core::status::OperationStatus;
use crate::core::types::{FfiAccount, FfiAccountsBatch, FfiChainTip};
use crate::indexer::LezIndexer;

pub type LezIndexerHandle = LezIndexer;

pub(crate) fn parse_caller(caller_ptr: *const u8) -> Result<[u8; 32], OperationStatus> {
    if caller_ptr.is_null() { return Err(OperationStatus::NullPointer); }
    let mut caller = [0u8; 32];
    caller.copy_from_slice(unsafe { std::slice::from_raw_parts(caller_ptr, 32) });
    if caller == [0u8; 32] { return Err(OperationStatus::InvalidArgumentCaller); }
    Ok(caller)
}

/// Opens or creates a LEZ indexer instance at `db_path`.
///
/// # Safety
/// `db_path` must be a valid, null-terminated C string pointing to a filesystem path.
#[no_mangle]
pub unsafe extern "C" fn lez_indexer_create(db_path: *const c_char) -> PointerResult<LezIndexerHandle> {
    catch_unwind_ffi(|| {
        if db_path.is_null() { return PointerResult::null_pointer(); }
        let c_str = unsafe { CStr::from_ptr(db_path) };
        let path_str = match c_str.to_str() {
            Ok(s) => s,
            Err(_) => return PointerResult::from_error(OperationStatus::InvalidArgumentGeneric),
        };
        match LezIndexer::open(Path::new(path_str)) {
            Ok(indexer) => PointerResult::from_value(indexer),
            Err(st) => PointerResult::from_error(st),
        }
    })
}

/// Frees a LEZ indexer instance handle.
///
/// # Safety
/// If `handle` is non-null, it must have been created by `lez_indexer_create`. Passing NULL is safe and a no-op.
#[no_mangle]
pub unsafe extern "C" fn lez_indexer_free(handle: *mut LezIndexerHandle) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}

/// Queries the current chain tip.
///
/// # Safety
/// `handle` must point to an open indexer. `caller_ptr` must point to a 32-byte caller address.
#[no_mangle]
pub unsafe extern "C" fn lez_query_chain_tip(handle: *mut LezIndexerHandle, caller_ptr: *const u8) -> PointerResult<FfiChainTip> {
    catch_unwind_ffi(|| {
        if handle.is_null() { return PointerResult::null_pointer(); }
        let caller = match parse_caller(caller_ptr) { Ok(c) => c, Err(st) => return PointerResult::from_error(st) };
        let indexer = unsafe { &*handle };
        if !indexer.policy.check_permission(&caller, Action::QueryRead) {
            return PointerResult::from_error(OperationStatus::PermissionDenied);
        }
        match indexer.query_chain_tip() {
            Ok(tip) => PointerResult::from_value(tip),
            Err(st) => PointerResult::from_error(st),
        }
    })
}

/// Queries the latest state of an account.
///
/// # Safety
/// `handle` must point to an open indexer. `caller_ptr` and `account_ptr` must point to 32-byte addresses.
#[no_mangle]
pub unsafe extern "C" fn lez_query_account(handle: *mut LezIndexerHandle, caller_ptr: *const u8, account_ptr: *const u8) -> PointerResult<FfiAccount> {
    catch_unwind_ffi(|| {
        if handle.is_null() || account_ptr.is_null() { return PointerResult::null_pointer(); }
        let caller = match parse_caller(caller_ptr) { Ok(c) => c, Err(st) => return PointerResult::from_error(st) };
        let indexer = unsafe { &*handle };
        if !indexer.policy.check_permission(&caller, Action::QueryRead) {
            return PointerResult::from_error(OperationStatus::PermissionDenied);
        }
        let mut acc_id = [0u8; 32];
        acc_id.copy_from_slice(unsafe { std::slice::from_raw_parts(account_ptr, 32) });
        match indexer.query_account(&acc_id) {
            Ok(acc) => PointerResult::from_value(acc),
            Err(st) => PointerResult::from_error(st),
        }
    })
}

/// Queries historical account state at a specific block height and hash.
///
/// # Safety
/// `handle` must point to an open indexer. `caller_ptr`, `account_ptr`, and `hash_ptr` must point to valid 32-byte buffers.
#[no_mangle]
pub unsafe extern "C" fn lez_query_account_at_block(
    handle: *mut LezIndexerHandle, caller_ptr: *const u8, account_ptr: *const u8, height: u64, hash_ptr: *const u8,
) -> PointerResult<FfiAccount> {
    catch_unwind_ffi(|| {
        if handle.is_null() || account_ptr.is_null() || hash_ptr.is_null() { return PointerResult::null_pointer(); }
        let caller = match parse_caller(caller_ptr) { Ok(c) => c, Err(st) => return PointerResult::from_error(st) };
        let indexer = unsafe { &*handle };
        if !indexer.policy.check_permission(&caller, Action::QueryRead) {
            return PointerResult::from_error(OperationStatus::PermissionDenied);
        }
        let mut acc_id = [0u8; 32];
        acc_id.copy_from_slice(unsafe { std::slice::from_raw_parts(account_ptr, 32) });
        let mut hash = [0u8; 32];
        hash.copy_from_slice(unsafe { std::slice::from_raw_parts(hash_ptr, 32) });
        match indexer.query_account_at_block(&acc_id, height, &hash) {
            Ok(acc) => PointerResult::from_value(acc),
            Err(st) => PointerResult::from_error(st),
        }
    })
}

/// Batch queries multiple accounts at once.
///
/// # Safety
/// `handle` must point to an open indexer. `caller_ptr` must point to a 32-byte address.
/// `accounts_ptr` must point to `count * 32` contiguous bytes.
#[no_mangle]
pub unsafe extern "C" fn lez_query_accounts(
    handle: *mut LezIndexerHandle, caller_ptr: *const u8, accounts_ptr: *const u8, count: usize,
) -> PointerResult<FfiAccountsBatch> {
    catch_unwind_ffi(|| {
        if handle.is_null() || (count > 0 && accounts_ptr.is_null()) { return PointerResult::null_pointer(); }
        let caller = match parse_caller(caller_ptr) { Ok(c) => c, Err(st) => return PointerResult::from_error(st) };
        let indexer = unsafe { &*handle };
        if !indexer.policy.check_permission(&caller, Action::QueryRead) {
            return PointerResult::from_error(OperationStatus::PermissionDenied);
        }
        let raw_slice = unsafe { std::slice::from_raw_parts(accounts_ptr, count * 32) };
        let mut ids = Vec::with_capacity(count);
        for chunk in raw_slice.chunks_exact(32) {
            let mut id = [0u8; 32];
            id.copy_from_slice(chunk);
            ids.push(id);
        }
        match indexer.query_accounts(&ids) {
            Ok(batch) => PointerResult::from_value(batch),
            Err(st) => PointerResult::from_error(st),
        }
    })
}
