use lez_node_ffi_poc::core::status::OperationStatus;
use lez_node_ffi_poc::indexer::LezIndexer;
use lez_node_ffi_poc::storage::fixture::load_deterministic_fixtures;
use std::sync::Arc;
use std::thread;
use tempfile::tempdir;

#[test]
fn test_query_chain_tip_and_schema() {
    // F#47, F#58
    let dir = tempdir().unwrap();
    let indexer = LezIndexer::open(dir.path()).unwrap();
    let acc = [1u8; 32];
    load_deterministic_fixtures(&indexer.store, 5, &[acc]).unwrap();

    let tip = indexer.query_chain_tip().unwrap();
    assert_eq!(tip.height, 5);
    assert_eq!(tip.hash[31], 0xAA);
    assert_eq!(tip.timestamp, 1_700_000_000 + 5 * 12);

    let schema_json = indexer.query_schema();
    assert!(schema_json.contains("lez_query_chain_tip"));
    assert!(schema_json.contains("lez_query_account_at_block"));
}

#[test]
fn test_query_account_default_zeroed_and_pinned_reads() {
    // F#10: zeroed default for unknown ID; F#11, F#14, R#3: pinned consistency
    let dir = tempdir().unwrap();
    let indexer = LezIndexer::open(dir.path()).unwrap();
    let known_acc = [7u8; 32];
    let unknown_acc = [99u8; 32];
    load_deterministic_fixtures(&indexer.store, 10, &[known_acc]).unwrap();

    // Unknown ID returns zeroed account (F#10)
    let unknown_res = indexer.query_account(&unknown_acc).unwrap();
    assert_eq!(unknown_res.account_id, unknown_acc);
    assert_eq!(unknown_res.balance, 0);
    assert_eq!(unknown_res.nonce, 0);

    // Known ID returns tip state
    let known_res = indexer.query_account(&known_acc).unwrap();
    assert_eq!(known_res.nonce, 20);
    assert_eq!(known_res.balance, 1000 + 10 * 10);

    // Pinned read at block 5
    let mut block_5_hash = [0u8; 32];
    block_5_hash[0..8].copy_from_slice(&5u64.to_be_bytes());
    block_5_hash[31] = 0xAA;

    let pinned_res = indexer
        .query_account_at_block(&known_acc, 5, &block_5_hash)
        .unwrap();
    assert_eq!(pinned_res.nonce, 10);
    assert_eq!(pinned_res.balance, 1000 + 5 * 10);

    // Invalid hash pin rejected (F#11)
    let bad_hash = [0xFFu8; 32];
    let err = indexer
        .query_account_at_block(&known_acc, 5, &bad_hash)
        .unwrap_err();
    assert_eq!(err, OperationStatus::InvalidArgumentBlockPin);
}

#[test]
fn test_query_accounts_batch_multi_get() {
    // F#13, R#4, P#3: batch read via multi_get_cf
    let dir = tempdir().unwrap();
    let indexer = LezIndexer::open(dir.path()).unwrap();
    let acc1 = [1u8; 32];
    let acc2 = [2u8; 32];
    let unknown = [9u8; 32];
    load_deterministic_fixtures(&indexer.store, 4, &[acc1, acc2]).unwrap();

    let batch = indexer.query_accounts(&[acc1, unknown, acc2]).unwrap();
    assert_eq!(batch.count, 3);
    unsafe {
        let slice = std::slice::from_raw_parts(batch.accounts, batch.count);
        assert_eq!(slice[0].account_id, acc1);
        assert_eq!(slice[0].nonce, 8);
        assert_eq!(slice[1].account_id, unknown);
        assert_eq!(slice[1].balance, 0); // zeroed
        assert_eq!(slice[2].account_id, acc2);
        assert_eq!(slice[2].nonce, 8);
    }
}

#[test]
fn test_query_transactions_stable_under_concurrent_ingestion() {
    // F#37–43, R#5: Opaque cursors stable while new blocks are ingested concurrently
    let dir = tempdir().unwrap();
    let indexer = Arc::new(LezIndexer::open(dir.path()).unwrap());
    let caller = [10u8; 32];
    let acc = [20u8; 32];

    // Initial state: 10 blocks (20 transactions for this account)
    load_deterministic_fixtures(&indexer.store, 10, &[acc]).unwrap();

    // Fetch Page 1 (limit = 10, oldest_first = true)
    let page1 = indexer
        .query_transactions_by_account(&caller, &acc, None, 10, true)
        .unwrap();
    assert_eq!(page1.count, 10);
    assert!(page1.has_more);
    assert!(!page1.next_cursor.is_null());

    let cursor_token = unsafe { std::ffi::CStr::from_ptr(page1.next_cursor) }
        .to_str()
        .unwrap()
        .to_owned();

    // Concurrent ingestion: Writer adds 10 more blocks (20 more transactions for this account)
    let indexer_clone = Arc::clone(&indexer);
    let writer_handle = thread::spawn(move || {
        // Appends blocks 11..20
        load_deterministic_fixtures(&indexer_clone.store, 20, &[acc]).unwrap();
    });
    writer_handle.join().unwrap();

    // Verify tip moved
    let new_tip = indexer.query_chain_tip().unwrap();
    assert_eq!(new_tip.height, 20);

    // Fetch Page 2 using cursor: MUST stay pinned to initial snapshot (20 txs total, not 40!)
    let page2 = indexer
        .query_transactions_by_account(&caller, &acc, Some(&cursor_token), 10, true)
        .unwrap();
    assert_eq!(page2.count, 10);
    assert!(!page2.has_more); // Snapshot boundary respected: finished the 20 items from snapshot!
}
