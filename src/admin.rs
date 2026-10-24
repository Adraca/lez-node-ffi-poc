use crate::api::LezIndexerHandle;
use crate::core::memory::catch_unwind_status;
use crate::core::policy::Action;
use crate::core::status::OperationStatus;

fn parse_caller(caller_ptr: *const u8) -> Result<[u8; 32], OperationStatus> {
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

/// F#67: Administrative start_indexer, closed by default.
///
/// # Safety
/// `handle` must point to an open indexer. `caller_ptr` must point to a 32-byte caller address.
#[no_mangle]
pub unsafe extern "C" fn lez_admin_start_indexer(
    handle: *mut LezIndexerHandle,
    caller_ptr: *const u8,
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
        if !indexer.policy.check_permission(&caller, Action::AdminStart) {
            return OperationStatus::PermissionDenied;
        }
        indexer.start()
    })
}

/// F#67: Administrative stop_indexer, closed by default.
///
/// # Safety
/// `handle` must point to an open indexer. `caller_ptr` must point to a 32-byte caller address.
#[no_mangle]
pub unsafe extern "C" fn lez_admin_stop_indexer(
    handle: *mut LezIndexerHandle,
    caller_ptr: *const u8,
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
        if !indexer.policy.check_permission(&caller, Action::AdminStop) {
            return OperationStatus::PermissionDenied;
        }
        indexer.stop()
    })
}

/// F#67: Administrative reset_storage, closed by default.
///
/// # Safety
/// `handle` must point to an open indexer. `caller_ptr` must point to a 32-byte caller address.
#[no_mangle]
pub unsafe extern "C" fn lez_admin_reset_storage(
    handle: *mut LezIndexerHandle,
    caller_ptr: *const u8,
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
        if !indexer.policy.check_permission(&caller, Action::AdminReset) {
            return OperationStatus::PermissionDenied;
        }
        indexer.reset_storage()
    })
}

/// Grants administrative access to a designated caller.
///
/// # Safety
/// `handle` must point to an open indexer. `caller_ptr` and `target_ptr` must point to 32-byte addresses.
#[no_mangle]
pub unsafe extern "C" fn lez_admin_grant(
    handle: *mut LezIndexerHandle,
    caller_ptr: *const u8,
    target_ptr: *const u8,
) -> OperationStatus {
    catch_unwind_status(|| {
        if handle.is_null() || target_ptr.is_null() {
            return OperationStatus::NullPointer;
        }
        let _caller = match parse_caller(caller_ptr) {
            Ok(c) => c,
            Err(st) => return st,
        };
        let mut target = [0u8; 32];
        target.copy_from_slice(unsafe { std::slice::from_raw_parts(target_ptr, 32) });
        let indexer = unsafe { &*handle };
        indexer.policy.grant_admin(&target);
        OperationStatus::Ok
    })
}

/// Revokes administrative access from a designated caller.
///
/// # Safety
/// `handle` must point to an open indexer. `caller_ptr` and `target_ptr` must point to 32-byte addresses.
#[no_mangle]
pub unsafe extern "C" fn lez_admin_revoke(
    handle: *mut LezIndexerHandle,
    caller_ptr: *const u8,
    target_ptr: *const u8,
) -> OperationStatus {
    catch_unwind_status(|| {
        if handle.is_null() || target_ptr.is_null() {
            return OperationStatus::NullPointer;
        }
        let _caller = match parse_caller(caller_ptr) {
            Ok(c) => c,
            Err(st) => return st,
        };
        let mut target = [0u8; 32];
        target.copy_from_slice(unsafe { std::slice::from_raw_parts(target_ptr, 32) });
        let indexer = unsafe { &*handle };
        indexer.policy.revoke_admin(&target);
        OperationStatus::Ok
    })
}
