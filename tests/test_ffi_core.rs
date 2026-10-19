use lez_node_ffi_poc::core::memory::{catch_unwind_ffi, catch_unwind_status};
use lez_node_ffi_poc::core::result::{free_boxed_pointer, PointerResult};
use lez_node_ffi_poc::core::status::OperationStatus;
use lez_node_ffi_poc::core::types::{FfiAccount, FfiShard};

#[test]
fn test_operation_status_three_way_split() {
    // F#61, F#62: Stable numeric codes and 3-way split
    assert!(OperationStatus::Ok.is_ok());

    let not_found = OperationStatus::NotFoundAccount;
    assert!(not_found.is_not_found());
    assert!(!not_found.is_invalid_argument());
    assert!(!not_found.is_backend_failure());
    assert_eq!(not_found as u32, 100);

    let invalid_arg = OperationStatus::InvalidArgumentBlockPin;
    assert!(invalid_arg.is_invalid_argument());
    assert!(!invalid_arg.is_not_found());
    assert!(!invalid_arg.is_backend_failure());
    assert_eq!(invalid_arg as u32, 203);

    let backend_fail = OperationStatus::BackendFailureStorage;
    assert!(backend_fail.is_backend_failure());
    assert!(!backend_fail.is_not_found());
    assert!(!backend_fail.is_invalid_argument());
    assert_eq!(backend_fail as u32, 300);
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
    unsafe {
        lez_node_ffi_poc::core::types::lez_free_account(std::ptr::null_mut());
        lez_node_ffi_poc::core::types::lez_free_accounts_batch(std::ptr::null_mut());
        lez_node_ffi_poc::core::types::lez_free_tx_page(std::ptr::null_mut());
        lez_node_ffi_poc::core::types::lez_free_shard(std::ptr::null_mut());
        lez_node_ffi_poc::core::types::lez_free_chain_tip(std::ptr::null_mut());
        lez_node_ffi_poc::core::memory::lez_free_string(std::ptr::null_mut());
        lez_node_ffi_poc::api_history::lez_free_cursor(std::ptr::null_mut(), std::ptr::null(), std::ptr::null());
        lez_node_ffi_poc::api::lez_indexer_free(std::ptr::null_mut());
    }
}
