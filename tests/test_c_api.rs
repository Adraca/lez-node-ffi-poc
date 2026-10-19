use std::ffi::{CStr, CString};
use tempfile::tempdir;
use lez_node_ffi_poc::admin::*;
use lez_node_ffi_poc::api::*;
use lez_node_ffi_poc::api_history::*;
use lez_node_ffi_poc::core::memory::lez_free_string;
use lez_node_ffi_poc::core::status::OperationStatus;
use lez_node_ffi_poc::core::types::{lez_free_account, lez_free_accounts_batch, lez_free_chain_tip, lez_free_shard, lez_free_tx_page};
use lez_node_ffi_poc::storage::fixture::load_deterministic_fixtures;

#[test]
fn test_c_api_lifecycle_and_queries() {
    let dir = tempdir().unwrap();
    let path_c = CString::new(dir.path().to_str().unwrap()).unwrap();

    let res = unsafe { lez_indexer_create(path_c.as_ptr()) };
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
        let pinned_res = lez_query_account_at_block(indexer, user_caller.as_ptr(), acc.as_ptr(), 5, h5_hash.as_ptr());
        assert_eq!(pinned_res.status, OperationStatus::Ok);
        lez_free_account(pinned_res.value);

        // Batch accounts query
        let batch_res = lez_query_accounts(indexer, user_caller.as_ptr(), acc.as_ptr(), 1);
        assert_eq!(batch_res.status, OperationStatus::Ok);
        assert_eq!((*batch_res.value).count, 1);
        lez_free_accounts_batch(batch_res.value);

        // History query
        let hist_res = lez_query_transactions_by_account(indexer, user_caller.as_ptr(), acc.as_ptr(), std::ptr::null(), 5, true);
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
        assert_eq!(lez_admin_start_indexer(indexer, user_caller.as_ptr()), OperationStatus::PermissionDenied);
        assert_eq!(lez_admin_grant(indexer, admin_caller.as_ptr(), admin_caller.as_ptr()), OperationStatus::Ok);
        assert_eq!(lez_admin_start_indexer(indexer, admin_caller.as_ptr()), OperationStatus::Ok);
        assert_eq!(lez_admin_stop_indexer(indexer, admin_caller.as_ptr()), OperationStatus::Ok);
        assert_eq!(lez_admin_reset_storage(indexer, admin_caller.as_ptr()), OperationStatus::Ok);

        lez_free_shard(std::ptr::null_mut());
        lez_indexer_free(indexer);
    }
}
