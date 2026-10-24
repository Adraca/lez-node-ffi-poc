use lez_node_ffi_poc::admin::*;
use lez_node_ffi_poc::api::*;
use lez_node_ffi_poc::api_history::*;
use lez_node_ffi_poc::core::memory::lez_free_string;
use lez_node_ffi_poc::core::result::PointerResult;
use lez_node_ffi_poc::core::status::OperationStatus;
use lez_node_ffi_poc::core::types::{
    lez_abi_version, lez_free_account, lez_free_accounts_batch, lez_free_chain_tip,
    lez_free_tx_page, FfiAccount, LEZ_ABI_VERSION_MAJOR,
};
use lez_node_ffi_poc::storage::fixture::load_deterministic_fixtures;
use std::ffi::{CStr, CString};
use tempfile::tempdir;

#[test]
fn test_c_api_lifecycle_and_queries() {
    let dir = tempdir().unwrap();
    let path_c = CString::new(dir.path().to_str().unwrap()).unwrap();

    let abi = lez_abi_version();
    assert_eq!(abi.major, LEZ_ABI_VERSION_MAJOR);
    assert_eq!(abi.minor, 0);

    // Mismatched major version rejected with InvalidArgumentAbiMismatch
    let bad_res = unsafe { lez_indexer_create(path_c.as_ptr(), 999) };
    assert_eq!(bad_res.status, OperationStatus::InvalidArgumentAbiMismatch);
    assert!(bad_res.value.is_null());

    let res = unsafe { lez_indexer_create(path_c.as_ptr(), LEZ_ABI_VERSION_MAJOR) };
    assert_eq!(res.status, OperationStatus::Ok);
    let indexer = res.value;

    let admin_caller = [1u8; 32];
    let user_caller = [2u8; 32];
    let acc = [5u8; 32];

    unsafe {
        // Populate fixtures
        load_deterministic_fixtures(&(*indexer).store, 5, &[acc]).unwrap();

        // Tip query
        let tip_res = lez_query_chain_tip(indexer, user_caller.as_ptr());
        assert_eq!(tip_res.status, OperationStatus::Ok);
        assert_eq!((*tip_res.value).height, 5);
        lez_free_chain_tip(tip_res.value);

        // Account query
        let acc_res = lez_query_account(indexer, user_caller.as_ptr(), acc.as_ptr());
        assert_eq!(acc_res.status, OperationStatus::Ok);
        assert_eq!((*acc_res.value).account_id, acc);
        lez_free_account(acc_res.value);

        // Pinned account query
        let mut h5_hash = [0u8; 32];
        h5_hash[0..8].copy_from_slice(&5u64.to_be_bytes());
        h5_hash[31] = 0xAA;
        let pinned_res = lez_query_account_at_block(
            indexer,
            user_caller.as_ptr(),
            acc.as_ptr(),
            5,
            h5_hash.as_ptr(),
        );
        assert_eq!(pinned_res.status, OperationStatus::Ok);
        lez_free_account(pinned_res.value);

        // Batch accounts query
        let batch_res = lez_query_accounts(indexer, user_caller.as_ptr(), acc.as_ptr(), 1);
        assert_eq!(batch_res.status, OperationStatus::Ok);
        assert_eq!((*batch_res.value).count, 1);
        lez_free_accounts_batch(batch_res.value);

        // History query
        let hist_res = lez_query_transactions_by_account(
            indexer,
            user_caller.as_ptr(),
            acc.as_ptr(),
            std::ptr::null(),
            5,
            true,
        );
        assert_eq!(hist_res.status, OperationStatus::Ok);
        assert_eq!((*hist_res.value).count, 5);
        lez_free_tx_page(hist_res.value);

        // Schema query
        let schema_res = lez_query_schema(indexer, user_caller.as_ptr());
        assert_eq!(schema_res.status, OperationStatus::Ok);
        let s = CStr::from_ptr(schema_res.value).to_str().unwrap();
        assert!(s.contains("lez_query_chain_tip"));
        lez_free_string(schema_res.value);

        // Admin lifecycle: user denied, grant, admin start/stop/reset
        assert_eq!(
            lez_admin_start_indexer(indexer, user_caller.as_ptr()),
            OperationStatus::PermissionDenied
        );
        assert_eq!(
            lez_admin_grant(indexer, admin_caller.as_ptr(), admin_caller.as_ptr()),
            OperationStatus::Ok
        );
        assert_eq!(
            lez_admin_start_indexer(indexer, admin_caller.as_ptr()),
            OperationStatus::Ok
        );
        assert_eq!(
            lez_admin_stop_indexer(indexer, admin_caller.as_ptr()),
            OperationStatus::Ok
        );

        // Audit Finding 8: Panic inside an FFI call on a live handle does NOT poison or brick it
        let panic_res: PointerResult<FfiAccount> =
            lez_node_ffi_poc::core::memory::catch_unwind_ffi(|| {
                assert!((*indexer).policy.check_permission(
                    &admin_caller,
                    lez_node_ffi_poc::core::policy::Action::AdminStart
                ));
                panic!("transient panic during operation");
            });
        assert_eq!(panic_res.status, OperationStatus::PanicCaught);
        assert!(panic_res.value.is_null());

        // Subsequent call on the SAME handle succeeds immediately
        assert_eq!(
            lez_admin_start_indexer(indexer, admin_caller.as_ptr()),
            OperationStatus::Ok
        );

        // Audit Finding 9: Batch limit enforcement
        let limit_res = lez_query_accounts(
            indexer,
            user_caller.as_ptr(),
            std::ptr::null(),
            MAX_BATCH_ACCOUNTS + 1,
        );
        assert_eq!(limit_res.status, OperationStatus::InvalidArgumentLimit);
        assert!(limit_res.value.is_null());

        lez_indexer_free(indexer);
    }
}

#[test]
fn test_c_api_abi_version_and_mismatch() {
    // Priority 2: lez_abi_version() and mismatched-major rejection in lez_indexer_create
    let dir = tempdir().unwrap();
    let path_c = CString::new(dir.path().to_str().unwrap()).unwrap();

    let abi = lez_abi_version();
    assert_eq!(abi.major, LEZ_ABI_VERSION_MAJOR);
    assert_eq!(abi.minor, 0);
    assert_eq!(abi.patch, 0);

    unsafe {
        // Rejection of major version 0, 2, and 999 with InvalidArgumentAbiMismatch and value == NULL
        let res_0 = lez_indexer_create(path_c.as_ptr(), 0);
        assert_eq!(res_0.status, OperationStatus::InvalidArgumentAbiMismatch);
        assert!(res_0.value.is_null());

        let res_2 = lez_indexer_create(path_c.as_ptr(), 2);
        assert_eq!(res_2.status, OperationStatus::InvalidArgumentAbiMismatch);
        assert!(res_2.value.is_null());

        let res_999 = lez_indexer_create(path_c.as_ptr(), 999);
        assert_eq!(res_999.status, OperationStatus::InvalidArgumentAbiMismatch);
        assert!(res_999.value.is_null());

        // Correct major version succeeds
        let res_ok = lez_indexer_create(path_c.as_ptr(), LEZ_ABI_VERSION_MAJOR);
        assert_eq!(res_ok.status, OperationStatus::Ok);
        assert!(!res_ok.value.is_null());
        lez_indexer_free(res_ok.value);
    }
}

#[test]
fn test_c_api_operation_status_error_paths_and_null_guarantee() {
    // Priority 1: Every OperationStatus the FFI can return triggered through C-ABI lez_* functions, with NULL on every error
    let dir = tempdir().unwrap();
    let path_c = CString::new(dir.path().to_str().unwrap()).unwrap();

    unsafe {
        // 1. NullPointer on lez_indexer_create and query functions
        let null_create = lez_indexer_create(std::ptr::null(), LEZ_ABI_VERSION_MAJOR);
        assert_eq!(null_create.status, OperationStatus::NullPointer);
        assert!(null_create.value.is_null());

        let null_tip = lez_query_chain_tip(std::ptr::null_mut(), std::ptr::null());
        assert_eq!(null_tip.status, OperationStatus::NullPointer);
        assert!(null_tip.value.is_null());

        let null_acc = lez_query_account(std::ptr::null_mut(), std::ptr::null(), std::ptr::null());
        assert_eq!(null_acc.status, OperationStatus::NullPointer);
        assert!(null_acc.value.is_null());

        let null_pin = lez_query_account_at_block(
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            std::ptr::null(),
        );
        assert_eq!(null_pin.status, OperationStatus::NullPointer);
        assert!(null_pin.value.is_null());

        let null_hist = lez_query_transactions_by_account(
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            5,
            true,
        );
        assert_eq!(null_hist.status, OperationStatus::NullPointer);
        assert!(null_hist.value.is_null());

        let null_schema = lez_query_schema(std::ptr::null_mut(), std::ptr::null());
        assert_eq!(null_schema.status, OperationStatus::NullPointer);
        assert!(null_schema.value.is_null());

        assert_eq!(
            lez_notify_block_ingest(std::ptr::null_mut(), 1, std::ptr::null(), 100),
            OperationStatus::NullPointer
        );

        // 2. InvalidArgumentGeneric on non-UTF-8 db_path
        let invalid_path = [b'f', b'o', b'o', 0xFF, 0xFE, 0];
        let bad_path_res = lez_indexer_create(
            invalid_path.as_ptr() as *const std::ffi::c_char,
            LEZ_ABI_VERSION_MAJOR,
        );
        assert_eq!(bad_path_res.status, OperationStatus::InvalidArgumentGeneric);
        assert!(bad_path_res.value.is_null());

        // Create valid instance for subsequent tests
        let init_res = lez_indexer_create(path_c.as_ptr(), LEZ_ABI_VERSION_MAJOR);
        assert_eq!(init_res.status, OperationStatus::Ok);
        let indexer = init_res.value;

        let caller = [1u8; 32];
        let zero_caller = [0u8; 32];
        let acc_a = [0xAA; 32];
        let acc_b = [0xBB; 32];

        // 3. NotFoundBlock: query_chain_tip on uninitialized DB before fixtures are loaded
        let empty_tip_res = lez_query_chain_tip(indexer, caller.as_ptr());
        assert_eq!(empty_tip_res.status, OperationStatus::NotFoundBlock);
        assert!(empty_tip_res.value.is_null());

        // 4. InvalidArgumentCaller (zero caller [0u8; 32]) across all query & admin functions
        let res1 = lez_query_chain_tip(indexer, zero_caller.as_ptr());
        assert_eq!(res1.status, OperationStatus::InvalidArgumentCaller);
        assert!(res1.value.is_null());

        let res2 = lez_query_account(indexer, zero_caller.as_ptr(), acc_a.as_ptr());
        assert_eq!(res2.status, OperationStatus::InvalidArgumentCaller);
        assert!(res2.value.is_null());

        let dummy_hash = [0x11; 32];
        let res3 = lez_query_account_at_block(
            indexer,
            zero_caller.as_ptr(),
            acc_a.as_ptr(),
            1,
            dummy_hash.as_ptr(),
        );
        assert_eq!(res3.status, OperationStatus::InvalidArgumentCaller);
        assert!(res3.value.is_null());

        let res4 = lez_query_accounts(indexer, zero_caller.as_ptr(), acc_a.as_ptr(), 1);
        assert_eq!(res4.status, OperationStatus::InvalidArgumentCaller);
        assert!(res4.value.is_null());

        let res5 = lez_query_transactions_by_account(
            indexer,
            zero_caller.as_ptr(),
            acc_a.as_ptr(),
            std::ptr::null(),
            5,
            true,
        );
        assert_eq!(res5.status, OperationStatus::InvalidArgumentCaller);
        assert!(res5.value.is_null());

        let res6 = lez_query_schema(indexer, zero_caller.as_ptr());
        assert_eq!(res6.status, OperationStatus::InvalidArgumentCaller);
        assert!(res6.value.is_null());

        assert_eq!(
            lez_admin_start_indexer(indexer, zero_caller.as_ptr()),
            OperationStatus::InvalidArgumentCaller
        );
        assert_eq!(
            lez_admin_stop_indexer(indexer, zero_caller.as_ptr()),
            OperationStatus::InvalidArgumentCaller
        );
        assert_eq!(
            lez_admin_reset_storage(indexer, zero_caller.as_ptr()),
            OperationStatus::InvalidArgumentCaller
        );
        assert_eq!(
            lez_admin_grant(indexer, zero_caller.as_ptr(), acc_a.as_ptr()),
            OperationStatus::InvalidArgumentCaller
        );
        assert_eq!(
            lez_admin_revoke(indexer, zero_caller.as_ptr(), acc_a.as_ptr()),
            OperationStatus::InvalidArgumentCaller
        );
        assert_eq!(
            lez_unsubscribe(indexer, zero_caller.as_ptr(), 1),
            OperationStatus::InvalidArgumentCaller
        );

        // Populate fixtures for remaining tests
        load_deterministic_fixtures(&(*indexer).store, 5, &[acc_a]).unwrap();

        // 5. NotFoundBlock: query_account_at_block with height above tip (tip=5, height=100)
        let above_tip_res = lez_query_account_at_block(
            indexer,
            caller.as_ptr(),
            acc_a.as_ptr(),
            100,
            dummy_hash.as_ptr(),
        );
        assert_eq!(above_tip_res.status, OperationStatus::NotFoundBlock);
        assert!(above_tip_res.value.is_null());

        // 6. InvalidArgumentBlockPin: block 1 exists, but hash does not match
        let bad_pin_res = lez_query_account_at_block(
            indexer,
            caller.as_ptr(),
            acc_a.as_ptr(),
            1,
            dummy_hash.as_ptr(),
        );
        assert_eq!(bad_pin_res.status, OperationStatus::InvalidArgumentBlockPin);
        assert!(bad_pin_res.value.is_null());

        // 7. InvalidArgumentLimit: history query with limit 0
        let zero_limit_res = lez_query_transactions_by_account(
            indexer,
            caller.as_ptr(),
            acc_a.as_ptr(),
            std::ptr::null(),
            0,
            true,
        );
        assert_eq!(zero_limit_res.status, OperationStatus::InvalidArgumentLimit);
        assert!(zero_limit_res.value.is_null());

        // 8. NotFoundCursor: query with nonexistent cursor
        let bad_cursor = CString::new("cur_asc_deadbeef00000000").unwrap();
        let bad_cur_res = lez_query_transactions_by_account(
            indexer,
            caller.as_ptr(),
            acc_a.as_ptr(),
            bad_cursor.as_ptr(),
            5,
            true,
        );
        assert_eq!(bad_cur_res.status, OperationStatus::NotFoundCursor);
        assert!(bad_cur_res.value.is_null());

        // 9. InvalidArgumentCursor: non-UTF-8 cursor pointer
        let invalid_utf8_cursor = [0xFFu8, 0xFE, 0];
        let bad_utf8_res = lez_query_transactions_by_account(
            indexer,
            caller.as_ptr(),
            acc_a.as_ptr(),
            invalid_utf8_cursor.as_ptr() as *const std::ffi::c_char,
            5,
            true,
        );
        assert_eq!(bad_utf8_res.status, OperationStatus::InvalidArgumentCursor);
        assert!(bad_utf8_res.value.is_null());

        // 10. InvalidArgumentAccount & PermissionDenied on cursor reuse
        let page_res = lez_query_transactions_by_account(
            indexer,
            caller.as_ptr(),
            acc_a.as_ptr(),
            std::ptr::null(),
            2,
            true,
        );
        assert_eq!(page_res.status, OperationStatus::Ok);
        assert!(!(*page_res.value).next_cursor.is_null());
        let valid_cursor_ptr = (*page_res.value).next_cursor;

        // Querying account B with account A's cursor -> InvalidArgumentAccount
        let diff_acc_res = lez_query_transactions_by_account(
            indexer,
            caller.as_ptr(),
            acc_b.as_ptr(),
            valid_cursor_ptr,
            2,
            true,
        );
        assert_eq!(diff_acc_res.status, OperationStatus::InvalidArgumentAccount);
        assert!(diff_acc_res.value.is_null());

        // Caller B attempting to use Caller A's cursor -> PermissionDenied
        let caller_b = [2u8; 32];
        let cross_caller_res = lez_query_transactions_by_account(
            indexer,
            caller_b.as_ptr(),
            acc_a.as_ptr(),
            valid_cursor_ptr,
            2,
            true,
        );
        assert_eq!(cross_caller_res.status, OperationStatus::PermissionDenied);
        assert!(cross_caller_res.value.is_null());

        // Caller B attempting to free Caller A's cursor -> PermissionDenied
        assert_eq!(
            lez_free_cursor(indexer, caller_b.as_ptr(), valid_cursor_ptr),
            OperationStatus::PermissionDenied
        );

        // Caller A frees their own cursor -> Ok
        assert_eq!(
            lez_free_cursor(indexer, caller.as_ptr(), valid_cursor_ptr),
            OperationStatus::Ok
        );
        // Freeing again -> NotFoundCursor
        assert_eq!(
            lez_free_cursor(indexer, caller.as_ptr(), valid_cursor_ptr),
            OperationStatus::NotFoundCursor
        );

        lez_free_tx_page(page_res.value);

        // 11. NotFoundGeneric on unsubscribe of unknown ID
        assert_eq!(
            lez_unsubscribe(indexer, caller.as_ptr(), 999_999),
            OperationStatus::NotFoundGeneric
        );

        lez_indexer_free(indexer);
    }
}

#[test]
fn test_c_api_admin_set_lifecycle() {
    // Priority 7: each admin call refused without a grant, allowed with one, grant revoked
    let dir = tempdir().unwrap();
    let path_c = CString::new(dir.path().to_str().unwrap()).unwrap();

    unsafe {
        let res = lez_indexer_create(path_c.as_ptr(), LEZ_ABI_VERSION_MAJOR);
        assert_eq!(res.status, OperationStatus::Ok);
        let indexer = res.value;

        let admin = [0xAA; 32];
        let unpriv = [0x55; 32];

        // 1. Refused without a grant: start, stop, reset all return PermissionDenied (400)
        assert_eq!(
            lez_admin_start_indexer(indexer, unpriv.as_ptr()),
            OperationStatus::PermissionDenied
        );
        assert_eq!(
            lez_admin_stop_indexer(indexer, unpriv.as_ptr()),
            OperationStatus::PermissionDenied
        );
        assert_eq!(
            lez_admin_reset_storage(indexer, unpriv.as_ptr()),
            OperationStatus::PermissionDenied
        );

        // 2. Grant admin to unpriv
        assert_eq!(
            lez_admin_grant(indexer, admin.as_ptr(), unpriv.as_ptr()),
            OperationStatus::Ok
        );

        // 3. Allowed with grant: start, stop, reset all return Ok
        assert_eq!(
            lez_admin_start_indexer(indexer, unpriv.as_ptr()),
            OperationStatus::Ok
        );
        assert_eq!(
            lez_admin_stop_indexer(indexer, unpriv.as_ptr()),
            OperationStatus::Ok
        );
        assert_eq!(
            lez_admin_reset_storage(indexer, unpriv.as_ptr()),
            OperationStatus::Ok
        );

        // 4. Revoke admin grant
        assert_eq!(
            lez_admin_revoke(indexer, admin.as_ptr(), unpriv.as_ptr()),
            OperationStatus::Ok
        );

        // 5. Refused after grant revoked: start, stop, reset all return PermissionDenied (400)
        assert_eq!(
            lez_admin_start_indexer(indexer, unpriv.as_ptr()),
            OperationStatus::PermissionDenied
        );
        assert_eq!(
            lez_admin_stop_indexer(indexer, unpriv.as_ptr()),
            OperationStatus::PermissionDenied
        );
        assert_eq!(
            lez_admin_reset_storage(indexer, unpriv.as_ptr()),
            OperationStatus::PermissionDenied
        );

        // 6. Null checks on grant, revoke, and admin execution
        assert_eq!(
            lez_admin_start_indexer(std::ptr::null_mut(), unpriv.as_ptr()),
            OperationStatus::NullPointer
        );
        assert_eq!(
            lez_admin_stop_indexer(std::ptr::null_mut(), unpriv.as_ptr()),
            OperationStatus::NullPointer
        );
        assert_eq!(
            lez_admin_reset_storage(std::ptr::null_mut(), unpriv.as_ptr()),
            OperationStatus::NullPointer
        );
        assert_eq!(
            lez_admin_grant(indexer, admin.as_ptr(), std::ptr::null()),
            OperationStatus::NullPointer
        );
        assert_eq!(
            lez_admin_revoke(indexer, admin.as_ptr(), std::ptr::null()),
            OperationStatus::NullPointer
        );
        assert_eq!(
            lez_admin_grant(std::ptr::null_mut(), admin.as_ptr(), unpriv.as_ptr()),
            OperationStatus::NullPointer
        );
        assert_eq!(
            lez_admin_revoke(std::ptr::null_mut(), admin.as_ptr(), unpriv.as_ptr()),
            OperationStatus::NullPointer
        );

        lez_indexer_free(indexer);
    }
}

#[test]
fn test_c_api_pinned_reads() {
    // Priority 5: wrong block hash, height above tip, unknown account at a pinned height
    let dir = tempdir().unwrap();
    let path_c = CString::new(dir.path().to_str().unwrap()).unwrap();

    unsafe {
        let res = lez_indexer_create(path_c.as_ptr(), LEZ_ABI_VERSION_MAJOR);
        assert_eq!(res.status, OperationStatus::Ok);
        let indexer = res.value;

        let caller = [1u8; 32];
        let known_acc = [0x11; 32];
        let unknown_acc = [0x99; 32];

        load_deterministic_fixtures(&(*indexer).store, 5, &[known_acc]).unwrap();

        let mut block_5_hash = [0u8; 32];
        block_5_hash[0..8].copy_from_slice(&5u64.to_be_bytes());
        block_5_hash[31] = 0xAA;

        // 1. Correct pinned read of known account
        let known_res = lez_query_account_at_block(
            indexer,
            caller.as_ptr(),
            known_acc.as_ptr(),
            5,
            block_5_hash.as_ptr(),
        );
        assert_eq!(known_res.status, OperationStatus::Ok);
        assert!(!known_res.value.is_null());
        assert_eq!((*known_res.value).nonce, 10);
        assert_eq!((*known_res.value).balance, 1050);
        lez_free_account(known_res.value);

        // 2. Unknown account at valid pinned height returns zeroed default account
        let unknown_res = lez_query_account_at_block(
            indexer,
            caller.as_ptr(),
            unknown_acc.as_ptr(),
            5,
            block_5_hash.as_ptr(),
        );
        assert_eq!(unknown_res.status, OperationStatus::Ok);
        assert!(!unknown_res.value.is_null());
        assert_eq!((*unknown_res.value).account_id, unknown_acc);
        assert_eq!((*unknown_res.value).nonce, 0);
        assert_eq!((*unknown_res.value).balance, 0);
        lez_free_account(unknown_res.value);

        // 3. Wrong block hash at valid height returns InvalidArgumentBlockPin and NULL value
        let mut wrong_hash = block_5_hash;
        wrong_hash[0] ^= 0xFF;
        let wrong_hash_res = lez_query_account_at_block(
            indexer,
            caller.as_ptr(),
            known_acc.as_ptr(),
            5,
            wrong_hash.as_ptr(),
        );
        assert_eq!(
            wrong_hash_res.status,
            OperationStatus::InvalidArgumentBlockPin
        );
        assert!(wrong_hash_res.value.is_null());

        // 4. Height above tip returns NotFoundBlock and NULL value
        let above_tip_res = lez_query_account_at_block(
            indexer,
            caller.as_ptr(),
            known_acc.as_ptr(),
            50,
            block_5_hash.as_ptr(),
        );
        assert_eq!(above_tip_res.status, OperationStatus::NotFoundBlock);
        assert!(above_tip_res.value.is_null());

        lez_indexer_free(indexer);
    }
}

#[test]
fn test_c_api_batch_accounts() {
    // Priority 6: count 0, count = MAX_BATCH_ACCOUNTS, count above it, NULL pointer with nonzero count
    let dir = tempdir().unwrap();
    let path_c = CString::new(dir.path().to_str().unwrap()).unwrap();

    unsafe {
        let res = lez_indexer_create(path_c.as_ptr(), LEZ_ABI_VERSION_MAJOR);
        assert_eq!(res.status, OperationStatus::Ok);
        let indexer = res.value;

        let caller = [1u8; 32];

        // 1. count 0 with NULL accounts_ptr returns Ok with count == 0 and non-null batch
        let batch_0 = lez_query_accounts(indexer, caller.as_ptr(), std::ptr::null(), 0);
        assert_eq!(batch_0.status, OperationStatus::Ok);
        assert!(!batch_0.value.is_null());
        assert_eq!((*batch_0.value).count, 0);
        lez_free_accounts_batch(batch_0.value);

        // 2. count = MAX_BATCH_ACCOUNTS (10,000)
        let max_accounts = vec![[0x42u8; 32]; MAX_BATCH_ACCOUNTS];
        let batch_max = lez_query_accounts(
            indexer,
            caller.as_ptr(),
            max_accounts.as_ptr() as *const u8,
            MAX_BATCH_ACCOUNTS,
        );
        assert_eq!(batch_max.status, OperationStatus::Ok);
        assert!(!batch_max.value.is_null());
        assert_eq!((*batch_max.value).count, MAX_BATCH_ACCOUNTS);
        lez_free_accounts_batch(batch_max.value);

        // 3. count > MAX_BATCH_ACCOUNTS (10,001) returns InvalidArgumentLimit and value == NULL
        let batch_over = lez_query_accounts(
            indexer,
            caller.as_ptr(),
            max_accounts.as_ptr() as *const u8,
            MAX_BATCH_ACCOUNTS + 1,
        );
        assert_eq!(batch_over.status, OperationStatus::InvalidArgumentLimit);
        assert!(batch_over.value.is_null());

        // 4. NULL pointer with nonzero count returns NullPointer and value == NULL
        let batch_null = lez_query_accounts(indexer, caller.as_ptr(), std::ptr::null(), 10);
        assert_eq!(batch_null.status, OperationStatus::NullPointer);
        assert!(batch_null.value.is_null());

        lez_indexer_free(indexer);
    }
}
