use crate::core::memory::{catch_unwind_default, catch_unwind_ffi, catch_unwind_status};
use crate::core::policy::Action;
use crate::core::result::PointerResult;
use crate::core::status::OperationStatus;
use crate::core::types::{FfiAccount, FfiAccountsBatch, FfiBlockCallback, FfiChainTip};
use crate::indexer::LezIndexer;
use std::ffi::{c_char, CStr};
use std::path::Path;

pub type LezIndexerHandle = LezIndexer;

/// Parses a 32-byte caller identifier across the C ABI.
///
/// ### Trust Boundary & Caller Identity Contract (Audit finding 11):
/// - In this standalone FFI PoC, `caller_ptr` is accepted across the C ABI as a 32-byte byte array.
/// - In the production Logos Core C++ module architecture, the caller identity **must be supplied and
///   attested by the host runtime / capability module** (e.g. `logos-capability-module` PR #37).
/// - Unprivileged client callers must NEVER be permitted to self-assert or forge caller identity.
pub(crate) fn parse_caller(caller_ptr: *const u8) -> Result<[u8; 32], OperationStatus> {
    if caller_ptr.is_null() {
        return Err(OperationStatus::NullPointer);
    }
    let mut caller = [0u8; 32];
    caller.copy_from_slice(unsafe { std::slice::from_raw_parts(caller_ptr, 32) });
    if caller == [0u8; 32] {
        return Err(OperationStatus::InvalidArgumentCaller);
    }
    Ok(caller)
}

/// Opens or creates a LEZ indexer instance at `db_path`.
/// Rejects callers with mismatched major ABI version (Audit finding 5).
///
/// # Safety
/// `db_path` must be a valid, null-terminated C string pointing to a filesystem path.
#[no_mangle]
pub unsafe extern "C" fn lez_indexer_create(
    db_path: *const c_char,
    expected_abi_major: u32,
) -> PointerResult<LezIndexerHandle> {
    catch_unwind_ffi(|| {
        if expected_abi_major != crate::core::types::LEZ_ABI_VERSION_MAJOR {
            return PointerResult::from_error(OperationStatus::InvalidArgumentAbiMismatch);
        }
        if db_path.is_null() {
            return PointerResult::null_pointer();
        }
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
pub unsafe extern "C" fn lez_query_chain_tip(
    handle: *mut LezIndexerHandle,
    caller_ptr: *const u8,
) -> PointerResult<FfiChainTip> {
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
pub unsafe extern "C" fn lez_query_account(
    handle: *mut LezIndexerHandle,
    caller_ptr: *const u8,
    account_ptr: *const u8,
) -> PointerResult<FfiAccount> {
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
    handle: *mut LezIndexerHandle,
    caller_ptr: *const u8,
    account_ptr: *const u8,
    height: u64,
    hash_ptr: *const u8,
) -> PointerResult<FfiAccount> {
    catch_unwind_ffi(|| {
        if handle.is_null() || account_ptr.is_null() || hash_ptr.is_null() {
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
        let mut hash = [0u8; 32];
        hash.copy_from_slice(unsafe { std::slice::from_raw_parts(hash_ptr, 32) });
        match indexer.query_account_at_block(&acc_id, height, &hash) {
            Ok(acc) => PointerResult::from_value(acc),
            Err(st) => PointerResult::from_error(st),
        }
    })
}

/// Maximum number of accounts allowed in a single batch query (Audit finding 9).
pub const MAX_BATCH_ACCOUNTS: usize = 10_000;

/// Batch queries multiple accounts at once.
///
/// # Bounds & Limits:
/// `count` must be <= `MAX_BATCH_ACCOUNTS` (10,000). Exceeding this limit returns
/// `OperationStatus::InvalidArgumentLimit` with `value == NULL` to protect host memory against unbounded allocations.
///
/// # Safety
/// `handle` must point to an open indexer. `caller_ptr` must point to a 32-byte address.
/// `accounts_ptr` must point to `count * 32` contiguous bytes.
#[no_mangle]
pub unsafe extern "C" fn lez_query_accounts(
    handle: *mut LezIndexerHandle,
    caller_ptr: *const u8,
    accounts_ptr: *const u8,
    count: usize,
) -> PointerResult<FfiAccountsBatch> {
    catch_unwind_ffi(|| {
        if count > MAX_BATCH_ACCOUNTS {
            return PointerResult::from_error(OperationStatus::InvalidArgumentLimit);
        }
        if handle.is_null() || (count > 0 && accounts_ptr.is_null()) {
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

/// Subscribes to finalized block events starting from `from_anchor` (Audit Recommendation 3).
///
/// # Returns:
/// Returns a non-zero unique subscription ID on success, or 0 on error (e.g. unauthenticated, null pointer).
///
/// # Safety
/// `handle` must point to an open indexer. `caller_ptr` must point to 32 bytes.
/// `callback` must be a valid extern "C" function pointer and must NOT throw C++ exceptions or unwind.
#[no_mangle]
pub unsafe extern "C" fn lez_subscribe_finalized_blocks(
    handle: *mut LezIndexerHandle,
    caller_ptr: *const u8,
    from_anchor: u64,
    callback: FfiBlockCallback,
    user_data: *mut std::ffi::c_void,
) -> u64 {
    catch_unwind_default(|| {
        if handle.is_null() || callback.is_none() {
            return 0;
        }
        let caller = match parse_caller(caller_ptr) {
            Ok(c) => c,
            Err(_) => return 0,
        };
        let indexer = unsafe { &*handle };
        if !indexer.policy.check_permission(&caller, Action::QueryRead) {
            return 0;
        }
        let cb = callback.unwrap();
        indexer
            .subscriptions
            .subscribe(caller, from_anchor, cb, user_data)
    })
}

/// Unsubscribes an active finalized-block subscription.
/// Enforces caller isolation: only the caller that created the subscription can unsubscribe it.
///
/// # Safety
/// `handle` must point to an open indexer. `caller_ptr` must point to 32 bytes.
#[no_mangle]
pub unsafe extern "C" fn lez_unsubscribe(
    handle: *mut LezIndexerHandle,
    caller_ptr: *const u8,
    subscription_id: u64,
) -> OperationStatus {
    catch_unwind_status(|| {
        if handle.is_null() {
            return OperationStatus::NullPointer;
        }
        let caller = match parse_caller(caller_ptr) {
            Ok(c) => c,
            Err(st) => return st,
        };
        let indexer = unsafe { &*handle };
        indexer.subscriptions.unsubscribe(&caller, subscription_id)
    })
}

/// Dispatches a finalized block notification to active subscribers (used by ingestion threads).
///
/// # Safety
/// `handle` must point to an open indexer. `hash_ptr` must point to 32 bytes.
#[no_mangle]
pub unsafe extern "C" fn lez_notify_block_ingest(
    handle: *mut LezIndexerHandle,
    height: u64,
    hash_ptr: *const u8,
    timestamp: u64,
) -> OperationStatus {
    catch_unwind_status(|| {
        if handle.is_null() || hash_ptr.is_null() {
            return OperationStatus::NullPointer;
        }
        let mut hash = [0u8; 32];
        hash.copy_from_slice(unsafe { std::slice::from_raw_parts(hash_ptr, 32) });
        let tip = FfiChainTip {
            height,
            hash,
            timestamp,
        };
        let indexer = unsafe { &*handle };
        indexer.notify_finalized_block(&tip);
        OperationStatus::Ok
    })
}
