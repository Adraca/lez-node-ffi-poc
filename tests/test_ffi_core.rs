use lez_node_ffi_poc::core::memory::{catch_unwind_ffi, catch_unwind_status};
use lez_node_ffi_poc::core::result::{free_boxed_pointer, PointerResult};
use lez_node_ffi_poc::core::status::OperationStatus;
use lez_node_ffi_poc::core::types::{FfiAccount, FfiShard};

#[test]
fn test_operation_status_bands() {
    // F#61, F#62 & Audit Findings 6-7: Stable numeric codes across 5 semantic bands
    assert!(OperationStatus::Ok.is_ok());

    let not_found = OperationStatus::NotFoundAccount;
    assert!(not_found.is_not_found());
    assert!(!not_found.is_invalid_argument());
    assert!(!not_found.is_backend_failure());
    assert!(!not_found.is_unauthorized());
    assert!(!not_found.is_conflict());
    assert_eq!(not_found as u32, 100);

    let invalid_arg = OperationStatus::InvalidArgumentBlockPin;
    assert!(invalid_arg.is_invalid_argument());
    assert!(!invalid_arg.is_not_found());
    assert!(!invalid_arg.is_backend_failure());
    assert!(!invalid_arg.is_unauthorized());
    assert!(!invalid_arg.is_conflict());
    assert_eq!(invalid_arg as u32, 203);

    let backend_fail = OperationStatus::BackendFailureStorage;
    assert!(backend_fail.is_backend_failure());
    assert!(!backend_fail.is_not_found());
    assert!(!backend_fail.is_invalid_argument());
    assert!(!backend_fail.is_unauthorized());
    assert!(!backend_fail.is_conflict());
    assert_eq!(backend_fail as u32, 300);

    // 400.. band checks
    let unauthorized_codes = [
        OperationStatus::PermissionDenied,
        OperationStatus::UnauthorizedNoGrant,
        OperationStatus::UnauthorizedUnknownCaller,
        OperationStatus::UnauthorizedRevokedGrant,
    ];
    for (i, code) in unauthorized_codes.iter().enumerate() {
        assert!(code.is_unauthorized());
        assert!(!code.is_not_found());
        assert!(!code.is_invalid_argument());
        assert!(!code.is_backend_failure());
        assert!(!code.is_conflict());
        assert_eq!(*code as u32, 400 + i as u32);
    }

    // 500.. band checks
    let conflict_codes = [
        OperationStatus::ConflictDuplicateTx,
        OperationStatus::ConflictNonce,
        OperationStatus::ConflictGeneric,
    ];
    for (i, code) in conflict_codes.iter().enumerate() {
        assert!(code.is_conflict());
        assert!(!code.is_not_found());
        assert!(!code.is_invalid_argument());
        assert!(!code.is_backend_failure());
        assert!(!code.is_unauthorized());
        assert_eq!(*code as u32, 500 + i as u32);
    }
}

#[test]
fn test_pointer_result_memory_lifecycle() {
    // F#58, F#63: Explicit malloc/free pairs
    let account = FfiAccount::zeroed([42u8; 32]);
    let result = PointerResult::from_value(account);
    assert!(result.status.is_ok());
    assert!(!result.value.is_null());

    unsafe {
        assert_eq!((*result.value).account_id, [42u8; 32]);
        assert_eq!((*result.value).balance, 0);
        free_boxed_pointer(result.value);
    }
}

#[test]
fn test_panic_barrier_shields_host() {
    // F#64: catch_unwind on FFI boundary
    let panic_result: PointerResult<FfiAccount> = catch_unwind_ffi(|| {
        panic!("simulated crash inside FFI execution");
    });
    assert_eq!(panic_result.status, OperationStatus::PanicCaught);
    assert!(panic_result.value.is_null());

    let panic_status = catch_unwind_status(|| {
        panic!("simulated crash inside admin FFI");
    });
    assert_eq!(panic_status, OperationStatus::PanicCaught);
}

#[test]
fn test_null_pointer_safety_behavior() {
    // F#64: Explicit null pointer protection
    let null_res: PointerResult<FfiShard> = PointerResult::null_pointer();
    assert_eq!(null_res.status, OperationStatus::NullPointer);
    assert!(null_res.value.is_null());

    // Null safety reconciliation: all free functions must accept NULL as a safe no-op
    // Note: lez_free_shard was removed as an interior-only type to eliminate double-free footguns.
    unsafe {
        lez_node_ffi_poc::core::types::lez_free_account(std::ptr::null_mut());
        lez_node_ffi_poc::core::types::lez_free_accounts_batch(std::ptr::null_mut());
        lez_node_ffi_poc::core::types::lez_free_tx_page(std::ptr::null_mut());
        lez_node_ffi_poc::core::types::lez_free_chain_tip(std::ptr::null_mut());
        lez_node_ffi_poc::core::memory::lez_free_string(std::ptr::null_mut());
        lez_node_ffi_poc::api_history::lez_free_cursor(
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
        );
        lez_node_ffi_poc::api::lez_indexer_free(std::ptr::null_mut());
    }
}

#[test]
fn test_pointer_result_null_on_error_guarantee() {
    // Audit Finding 4: Guarantee value == NULL whenever status != Ok
    let all_err_statuses = [
        OperationStatus::NullPointer,
        OperationStatus::PanicCaught,
        OperationStatus::Uninitialized,
        OperationStatus::AbiMismatch,
        OperationStatus::NotFoundAccount,
        OperationStatus::NotFoundBlock,
        OperationStatus::NotFoundTransaction,
        OperationStatus::NotFoundCursor,
        OperationStatus::NotFoundGeneric,
        OperationStatus::InvalidArgumentCaller,
        OperationStatus::InvalidArgumentAccount,
        OperationStatus::InvalidArgumentCursor,
        OperationStatus::InvalidArgumentBlockPin,
        OperationStatus::InvalidArgumentLimit,
        OperationStatus::InvalidArgumentGeneric,
        OperationStatus::InvalidArgumentAbiMismatch,
        OperationStatus::BackendFailureStorage,
        OperationStatus::BackendFailureSerialization,
        OperationStatus::BackendFailureLockPoisoned,
        OperationStatus::BackendFailureGeneric,
        OperationStatus::PermissionDenied,
        OperationStatus::UnauthorizedNoGrant,
        OperationStatus::UnauthorizedUnknownCaller,
        OperationStatus::UnauthorizedRevokedGrant,
        OperationStatus::ConflictDuplicateTx,
        OperationStatus::ConflictNonce,
        OperationStatus::ConflictGeneric,
    ];

    for status in all_err_statuses {
        let res: PointerResult<FfiAccount> = PointerResult::from_error(status);
        assert_eq!(res.status, status);
        assert!(
            res.value.is_null(),
            "value must be NULL on error status {:?}",
            status
        );
        assert!(!res.status.is_ok());
    }

    let null_res: PointerResult<FfiAccount> = PointerResult::null_pointer();
    assert_eq!(null_res.status, OperationStatus::NullPointer);
    assert!(null_res.value.is_null());

    let panic_res: PointerResult<FfiAccount> = PointerResult::panic_caught();
    assert_eq!(panic_res.status, OperationStatus::PanicCaught);
    assert!(panic_res.value.is_null());
}

#[test]
fn test_empty_batch_free_miri() {
    use lez_node_ffi_poc::core::types::{lez_free_accounts_batch, FfiAccountsBatch};

    // 1. Completely null/empty batch
    let batch_null = Box::into_raw(Box::new(FfiAccountsBatch {
        accounts: std::ptr::null_mut(),
        count: 0,
    }));
    unsafe { lez_free_accounts_batch(batch_null) };

    // 2. Batch with 0 count and non-null dangling slice pointer
    let accounts: Vec<FfiAccount> = Vec::new();
    let boxed_slice = accounts.into_boxed_slice();
    let ptr = Box::into_raw(boxed_slice) as *mut FfiAccount;
    let batch_zero = Box::into_raw(Box::new(FfiAccountsBatch {
        accounts: ptr,
        count: 0,
    }));
    unsafe { lez_free_accounts_batch(batch_zero) };
}

#[test]
fn test_ffi_account_shards_nested_free_miri() {
    // Audit Finding 2: Shard array and shard data capacity metadata (shards_cap and data_cap)
    // Allocate shard 0 with cap > len
    let mut data0 = Vec::with_capacity(16);
    data0.extend_from_slice(b"program0_data");
    let data0_len = data0.len();
    let data0_cap = data0.capacity();
    let data0_ptr = data0.as_mut_ptr();
    std::mem::forget(data0);

    let shard0 = FfiShard {
        program: [0xAA; 32],
        data: data0_ptr,
        data_len: data0_len,
        data_cap: data0_cap,
    };

    // Allocate shard 1 with cap > len
    let mut data1 = Vec::with_capacity(32);
    data1.extend_from_slice(b"program1_payload_bytes");
    let data1_len = data1.len();
    let data1_cap = data1.capacity();
    let data1_ptr = data1.as_mut_ptr();
    std::mem::forget(data1);

    let shard1 = FfiShard {
        program: [0xBB; 32],
        data: data1_ptr,
        data_len: data1_len,
        data_cap: data1_cap,
    };

    // Allocate shards vector with capacity > len (cap 4, len 2)
    let mut shards = Vec::with_capacity(4);
    shards.push(shard0);
    shards.push(shard1);
    let shards_len = shards.len();
    let shards_cap = shards.capacity();
    let shards_ptr = shards.as_mut_ptr();
    std::mem::forget(shards);

    let account = Box::into_raw(Box::new(FfiAccount {
        account_id: [0x42; 32],
        nonce: 10,
        balance: 5000,
        shards: shards_ptr,
        shards_len,
        shards_cap,
    }));

    // Free under Miri: must cleanly reconstruct Vec with correct cap and free without UB or memory leaks
    unsafe {
        lez_node_ffi_poc::core::types::lez_free_account(account);
    }
}

#[test]
fn test_panic_does_not_poison_handle_or_brick_callers() {
    // Audit Finding 8: Non-poisoning parking_lot locks ensure caught panics do not brick the handle
    use lez_node_ffi_poc::core::policy::{Action, PolicyEngine};
    use lez_node_ffi_poc::storage::cursors::CursorManager;

    let policy = PolicyEngine::new();
    let admin = [1u8; 32];
    policy.grant_admin(&admin);

    // Simulated panic while reading policy
    let panic_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert!(policy.check_permission(&admin, Action::AdminStart));
        panic!("simulated transient panic inside policy check");
    }));
    assert!(panic_res.is_err());

    // Subsequent access on SAME policy: MUST NOT be poisoned!
    assert!(policy.check_permission(&admin, Action::AdminStart));
    assert!(policy.check_permission(&admin, Action::QueryRead));

    let cursors = CursorManager::new();
    let panic_cursor = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = cursors.count();
        panic!("simulated transient panic inside cursor manager");
    }));
    assert!(panic_cursor.is_err());

    // Subsequent access on SAME cursor manager: MUST NOT be poisoned!
    assert_eq!(cursors.count(), 0);
}

#[test]
fn test_lez_query_accounts_max_batch_limit() {
    // Audit Finding 9: Enforce MAX_BATCH_ACCOUNTS (10,000) limit to prevent unbounded allocations
    let caller = [1u8; 32];

    // Query with count > MAX_BATCH_ACCOUNTS (10,001)
    let res = unsafe {
        lez_node_ffi_poc::api::lez_query_accounts(
            std::ptr::null_mut(),
            caller.as_ptr(),
            std::ptr::null(),
            lez_node_ffi_poc::api::MAX_BATCH_ACCOUNTS + 1,
        )
    };
    assert_eq!(res.status, OperationStatus::InvalidArgumentLimit);
    assert!(res.value.is_null());
}
