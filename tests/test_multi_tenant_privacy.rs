use lez_node_ffi_poc::core::policy::Action;
use lez_node_ffi_poc::core::status::OperationStatus;
use lez_node_ffi_poc::indexer::LezIndexer;
use lez_node_ffi_poc::storage::fixture::load_deterministic_fixtures;
use std::sync::Arc;
use std::thread;
use tempfile::tempdir;

#[test]
fn test_cross_caller_cursor_reuse_refused() {
    // F#66, Privacy#4: Cross-caller cursor isolation
    let dir = tempdir().unwrap();
    let indexer = LezIndexer::open(dir.path()).unwrap();
    let caller_a = [0xAA; 32];
    let caller_b = [0xBB; 32];
    let account = [0x01; 32];

    load_deterministic_fixtures(&indexer.store, 10, &[account]).unwrap();

    // Caller A queries first page and gets a cursor token
    let page1 = indexer
        .query_transactions_by_account(&caller_a, &account, None, 5, true)
        .unwrap();
    assert!(page1.has_more);
    let cursor_token = unsafe { std::ffi::CStr::from_ptr(page1.next_cursor) }
        .to_str()
        .unwrap()
        .to_owned();

    // Caller A successfully reads next page with their own cursor
    let res_a =
        indexer.query_transactions_by_account(&caller_a, &account, Some(&cursor_token), 5, true);
    assert!(res_a.is_ok());

    // Caller B attempts to use Caller A's cursor token: MUST be refused with PermissionDenied!
    let res_b =
        indexer.query_transactions_by_account(&caller_b, &account, Some(&cursor_token), 5, true);
    assert_eq!(res_b.unwrap_err(), OperationStatus::PermissionDenied);
}

#[test]
fn test_admin_policy_closed_by_default_and_grant_flow() {
    // F#67, F#68: Admin set closed by default, granted callers permitted
    let dir = tempdir().unwrap();
    let indexer = LezIndexer::open(dir.path()).unwrap();
    let unpriv_caller = [0x55; 32];
    let admin_caller = [0x01; 32];

    // Unprivileged caller denied for start, stop, reset
    assert!(!indexer
        .policy
        .check_permission(&unpriv_caller, Action::AdminStart));
    assert!(!indexer
        .policy
        .check_permission(&unpriv_caller, Action::AdminStop));
    assert!(!indexer
        .policy
        .check_permission(&unpriv_caller, Action::AdminReset));

    // Grant admin to admin_caller
    indexer.policy.grant_admin(&admin_caller);
    assert!(indexer
        .policy
        .check_permission(&admin_caller, Action::AdminStart));
    assert!(indexer
        .policy
        .check_permission(&admin_caller, Action::AdminStop));
    assert!(indexer
        .policy
        .check_permission(&admin_caller, Action::AdminReset));

    // Still denied for unprivileged caller
    assert!(!indexer
        .policy
        .check_permission(&unpriv_caller, Action::AdminStart));

    // Zeroed caller rejected across the board (unauthenticated)
    let zero_caller = [0u8; 32];
    assert!(!indexer
        .policy
        .check_permission(&zero_caller, Action::QueryRead));
    assert!(!indexer
        .policy
        .check_permission(&zero_caller, Action::AdminStart));
}

#[test]
fn test_concurrent_readers_do_not_block() {
    // F#70, R#5: Concurrent reads proceed in parallel without blocking
    let dir = tempdir().unwrap();
    let indexer = Arc::new(LezIndexer::open(dir.path()).unwrap());
    let accounts: Vec<[u8; 32]> = (1..=10)
        .map(|i| {
            let mut id = [0u8; 32];
            id[0] = i;
            id
        })
        .collect();

    load_deterministic_fixtures(&indexer.store, 20, &accounts).unwrap();

    let mut handles = Vec::new();
    for thread_idx in 0..8 {
        let idx_clone = Arc::clone(&indexer);
        let acc = accounts[thread_idx % accounts.len()];
        let caller = [thread_idx as u8 + 10; 32];

        let handle = thread::spawn(move || {
            for _ in 0..50 {
                let res = idx_clone.query_account(&acc).unwrap();
                assert!(res.balance > 0);

                let tip = idx_clone.query_chain_tip().unwrap();
                assert_eq!(tip.height, 20);

                let hist = idx_clone
                    .query_transactions_by_account(&caller, &acc, None, 5, true)
                    .unwrap();
                assert_eq!(hist.count, 5);
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }
}
