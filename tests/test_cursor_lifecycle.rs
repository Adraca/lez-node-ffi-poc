use lez_node_ffi_poc::core::status::OperationStatus;
use lez_node_ffi_poc::indexer::LezIndexer;
use lez_node_ffi_poc::storage::cursors::{
    CursorManager, CursorState, DEFAULT_MAX_CURSORS, DEFAULT_MAX_CURSORS_PER_CALLER,
};
use lez_node_ffi_poc::storage::fixture::load_deterministic_fixtures;
use std::ffi::{CStr, CString};
use std::thread::sleep;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_freed_cursor_is_refused() {
    // Audit Finding 1: Explicit cursor freeing and refusal of freed cursors
    let dir = tempdir().unwrap();
    let indexer = LezIndexer::open(dir.path()).unwrap();
    let caller = [0x11; 32];
    let account = [0x22; 32];

    load_deterministic_fixtures(&indexer.store, 10, &[account]).unwrap();

    // Query first page with limit 5; has_more will be true
    let page1 = indexer
        .query_transactions_by_account(&caller, &account, None, 5, true)
        .unwrap();
    assert!(page1.has_more);
    assert!(!page1.next_cursor.is_null());
    let token = unsafe { CStr::from_ptr(page1.next_cursor) }
        .to_str()
        .unwrap()
        .to_owned();

    // Verify token can be read
    let page2 = indexer.query_transactions_by_account(&caller, &account, Some(&token), 5, true);
    assert!(page2.is_ok());

    // Free the cursor via indexer.free_cursor
    let free_res = indexer.free_cursor(&caller, &token);
    assert_eq!(free_res, OperationStatus::Ok);

    // Verify that attempting to read with the freed cursor is refused with NotFoundCursor
    let refused_res =
        indexer.query_transactions_by_account(&caller, &account, Some(&token), 5, true);
    assert_eq!(refused_res.unwrap_err(), OperationStatus::NotFoundCursor);

    // Double-freeing returns NotFoundCursor
    let double_free = indexer.free_cursor(&caller, &token);
    assert_eq!(double_free, OperationStatus::NotFoundCursor);

    // Deallocate page structures
    unsafe {
        lez_node_ffi_poc::core::types::lez_free_tx_page(Box::into_raw(Box::new(page1)));
    }
}

#[test]
fn test_free_cursor_permission_isolation() {
    // Privacy#4 / F#66: Another caller cannot free or invalidate a cursor they do not own
    let dir = tempdir().unwrap();
    let indexer = LezIndexer::open(dir.path()).unwrap();
    let caller_a = [0xAA; 32];
    let caller_b = [0xBB; 32];
    let account = [0x33; 32];

    load_deterministic_fixtures(&indexer.store, 10, &[account]).unwrap();

    let page1 = indexer
        .query_transactions_by_account(&caller_a, &account, None, 5, true)
        .unwrap();
    let token = unsafe { CStr::from_ptr(page1.next_cursor) }
        .to_str()
        .unwrap()
        .to_owned();

    // Caller B attempts to free Caller A's cursor -> PermissionDenied
    let free_attempt = indexer.free_cursor(&caller_b, &token);
    assert_eq!(free_attempt, OperationStatus::PermissionDenied);

    // Cursor is still valid for Caller A
    let page2 = indexer.query_transactions_by_account(&caller_a, &account, Some(&token), 5, true);
    assert!(page2.is_ok());

    // Clean up
    assert_eq!(indexer.free_cursor(&caller_a, &token), OperationStatus::Ok);
    unsafe {
        lez_node_ffi_poc::core::types::lez_free_tx_page(Box::into_raw(Box::new(page1)));
    }
}

#[test]
fn test_expired_cursor_is_refused() {
    // Audit Finding 1: TTL eviction in CursorManager
    let short_ttl = Duration::from_millis(20);
    let manager = CursorManager::with_config(100, short_ttl);
    let caller = [0x01; 32];
    let account = [0x02; 32];

    let token = manager.create_token(CursorState {
        caller_id: caller,
        account_id: account,
        next_index: 5,
        snapshot_tip: 10,
        oldest_first: true,
        created_at: std::time::Instant::now(),
    });

    // Valid immediately
    assert!(manager.validate_token(&token, &caller, &account).is_ok());

    // Sleep past TTL
    sleep(Duration::from_millis(35));

    // Refused due to expiration
    let val_res = manager.validate_token(&token, &caller, &account);
    assert_eq!(val_res.unwrap_err(), OperationStatus::NotFoundCursor);
}

#[test]
fn test_cursor_map_stays_bounded_under_10k_pages() {
    // Audit Finding 1: Map stays bounded under 10k pages
    let manager = CursorManager::new(); // DEFAULT_MAX_CURSORS = 10_000
    assert_eq!(DEFAULT_MAX_CURSORS, 10_000);
    let account = [0x02; 32];

    // 10,500 cursors from 21 callers, each under the per-caller cap
    for i in 0..10_500u64 {
        let mut caller = [0u8; 32];
        caller[0] = (i % 21) as u8;
        manager.create_token(CursorState {
            caller_id: caller,
            account_id: account,
            next_index: i,
            snapshot_tip: 20_000,
            oldest_first: true,
            created_at: std::time::Instant::now(),
        });
    }

    // Must be bounded to at most DEFAULT_MAX_CURSORS (10,000)
    assert_eq!(manager.count(), DEFAULT_MAX_CURSORS);
}

#[test]
fn test_one_caller_cannot_evict_another_callers_cursors() {
    // A caller paging heavily hits its own cap and evicts only its own oldest cursors.
    let manager = CursorManager::new();
    let account = [0x02; 32];
    let state = |caller: [u8; 32], i: u64| CursorState {
        caller_id: caller,
        account_id: account,
        next_index: i,
        snapshot_tip: 20_000,
        oldest_first: true,
        created_at: std::time::Instant::now(),
    };
    let victim = [0xBB; 32];
    let victim_token = manager.create_token(state(victim, 0));

    let noisy = [0xAA; 32];
    for i in 0..(DEFAULT_MAX_CURSORS as u64 + 500) {
        manager.create_token(state(noisy, i));
    }

    assert!(manager
        .validate_token(&victim_token, &victim, &account)
        .is_ok());
    assert_eq!(manager.count(), DEFAULT_MAX_CURSORS_PER_CALLER + 1);
}

#[test]
fn test_ffi_lez_free_cursor() {
    use lez_node_ffi_poc::api::lez_indexer_create;
    use lez_node_ffi_poc::api::lez_indexer_free;
    use lez_node_ffi_poc::api_history::lez_free_cursor;

    let dir = tempdir().unwrap();
    let dir_c = CString::new(dir.path().to_str().unwrap()).unwrap();
    let res = unsafe {
        lez_indexer_create(
            dir_c.as_ptr(),
            lez_node_ffi_poc::core::types::LEZ_ABI_VERSION_MAJOR,
        )
    };
    assert_eq!(res.status, OperationStatus::Ok);
    let handle = res.value;

    let caller = [0x01; 32];
    let cursor_c = CString::new("cur_asc_0000000000000001").unwrap();

    // Non-existent cursor -> NotFoundCursor
    let st = unsafe { lez_free_cursor(handle, caller.as_ptr(), cursor_c.as_ptr()) };
    assert_eq!(st, OperationStatus::NotFoundCursor);

    // Null safety
    assert_eq!(
        unsafe { lez_free_cursor(std::ptr::null_mut(), std::ptr::null(), std::ptr::null()) },
        OperationStatus::Ok
    );
    assert_eq!(
        unsafe { lez_free_cursor(handle, caller.as_ptr(), std::ptr::null()) },
        OperationStatus::Ok
    );
    assert_eq!(
        unsafe { lez_free_cursor(handle, std::ptr::null(), cursor_c.as_ptr()) },
        OperationStatus::NullPointer
    );

    unsafe { lez_indexer_free(handle) };
}

#[test]
fn test_cursor_ttl_expiry_path_inside_create_token() {
    // Audit Finding 1 & Priority 4: TTL expiry path inside create_token (lines 110-121)
    let short_ttl = Duration::from_millis(20);
    let manager = CursorManager::with_limits(100, 100, short_ttl);
    let caller1 = [0x01; 32];
    let caller2 = [0x02; 32];
    let account = [0xAA; 32];

    // Create cursor 1
    let token1 = manager.create_token(CursorState {
        caller_id: caller1,
        account_id: account,
        next_index: 0,
        snapshot_tip: 100,
        oldest_first: true,
        created_at: std::time::Instant::now(),
    });
    assert_eq!(manager.count(), 1);

    // Sleep past TTL so token1 becomes stale
    sleep(Duration::from_millis(35));

    // Calling create_token for token2 must execute step 1 and expire token1
    let token2 = manager.create_token(CursorState {
        caller_id: caller2,
        account_id: account,
        next_index: 10,
        snapshot_tip: 100,
        oldest_first: true,
        created_at: std::time::Instant::now(),
    });

    // token1 was evicted during token2 creation: count is 1, and token1 is NotFoundCursor
    assert_eq!(manager.count(), 1);
    assert_eq!(
        manager
            .validate_token(&token1, &caller1, &account)
            .unwrap_err(),
        OperationStatus::NotFoundCursor
    );
    assert!(manager.validate_token(&token2, &caller2, &account).is_ok());
}

#[test]
fn test_cursor_per_caller_and_total_cap_eviction() {
    // Priority 4: Per-caller cap and total cap boundaries
    let account = [0xBB; 32];

    // 1. Per-caller cap eviction
    let manager_per_caller = CursorManager::with_limits(10, 2, Duration::from_secs(300));
    let caller_a = [0x0A; 32];

    let t1 = manager_per_caller.create_token(CursorState {
        caller_id: caller_a,
        account_id: account,
        next_index: 1,
        snapshot_tip: 50,
        oldest_first: true,
        created_at: std::time::Instant::now(),
    });
    let t2 = manager_per_caller.create_token(CursorState {
        caller_id: caller_a,
        account_id: account,
        next_index: 2,
        snapshot_tip: 50,
        oldest_first: true,
        created_at: std::time::Instant::now(),
    });
    assert_eq!(manager_per_caller.count(), 2);

    // 3rd token exceeds per-caller cap (2); t1 must be evicted
    let t3 = manager_per_caller.create_token(CursorState {
        caller_id: caller_a,
        account_id: account,
        next_index: 3,
        snapshot_tip: 50,
        oldest_first: true,
        created_at: std::time::Instant::now(),
    });
    assert_eq!(manager_per_caller.count(), 2);
    assert_eq!(
        manager_per_caller
            .validate_token(&t1, &caller_a, &account)
            .unwrap_err(),
        OperationStatus::NotFoundCursor
    );
    assert!(manager_per_caller
        .validate_token(&t2, &caller_a, &account)
        .is_ok());
    assert!(manager_per_caller
        .validate_token(&t3, &caller_a, &account)
        .is_ok());

    // 2. Total cap eviction
    let manager_total = CursorManager::with_limits(3, 5, Duration::from_secs(300));
    let c1 = [0x01; 32];
    let c2 = [0x02; 32];
    let c3 = [0x03; 32];
    let c4 = [0x04; 32];

    let tc1 = manager_total.create_token(CursorState {
        caller_id: c1,
        account_id: account,
        next_index: 1,
        snapshot_tip: 50,
        oldest_first: true,
        created_at: std::time::Instant::now(),
    });
    let tc2 = manager_total.create_token(CursorState {
        caller_id: c2,
        account_id: account,
        next_index: 2,
        snapshot_tip: 50,
        oldest_first: true,
        created_at: std::time::Instant::now(),
    });
    let tc3 = manager_total.create_token(CursorState {
        caller_id: c3,
        account_id: account,
        next_index: 3,
        snapshot_tip: 50,
        oldest_first: true,
        created_at: std::time::Instant::now(),
    });
    assert_eq!(manager_total.count(), 3);

    // 4th token from new caller c4 hits global cap (3); globally oldest tc1 is evicted
    let tc4 = manager_total.create_token(CursorState {
        caller_id: c4,
        account_id: account,
        next_index: 4,
        snapshot_tip: 50,
        oldest_first: true,
        created_at: std::time::Instant::now(),
    });
    assert_eq!(manager_total.count(), 3);
    assert_eq!(
        manager_total
            .validate_token(&tc1, &c1, &account)
            .unwrap_err(),
        OperationStatus::NotFoundCursor
    );
    assert!(manager_total.validate_token(&tc2, &c2, &account).is_ok());
    assert!(manager_total.validate_token(&tc3, &c3, &account).is_ok());
    assert!(manager_total.validate_token(&tc4, &c4, &account).is_ok());
}

#[test]
fn test_free_expired_cursor_returns_not_found() {
    // Priority 4: Free of an expired cursor
    let short_ttl = Duration::from_millis(20);
    let manager = CursorManager::with_limits(100, 100, short_ttl);
    let caller = [0x05; 32];
    let account = [0xCC; 32];

    let token = manager.create_token(CursorState {
        caller_id: caller,
        account_id: account,
        next_index: 5,
        snapshot_tip: 10,
        oldest_first: true,
        created_at: std::time::Instant::now(),
    });

    // Sleep past TTL
    sleep(Duration::from_millis(35));

    // Attempting to free an expired cursor must return NotFoundCursor
    assert_eq!(
        manager.free_cursor(&token, &caller),
        OperationStatus::NotFoundCursor
    );
    assert_eq!(manager.count(), 0);
}

#[test]
fn test_ffi_free_expired_and_cross_caller_cursor() {
    // Priority 4 across C ABI: lez_free_cursor permission isolation and expiration
    use lez_node_ffi_poc::api::lez_indexer_create;
    use lez_node_ffi_poc::api::lez_indexer_free;
    use lez_node_ffi_poc::api_history::lez_free_cursor;

    let dir = tempdir().unwrap();
    let dir_c = CString::new(dir.path().to_str().unwrap()).unwrap();
    let res = unsafe {
        lez_indexer_create(
            dir_c.as_ptr(),
            lez_node_ffi_poc::core::types::LEZ_ABI_VERSION_MAJOR,
        )
    };
    assert_eq!(res.status, OperationStatus::Ok);
    let handle = res.value;

    let caller_a = [0x0A; 32];
    let caller_b = [0x0B; 32];
    let account = [0xDD; 32];

    unsafe {
        load_deterministic_fixtures(&(*handle).store, 10, &[account]).unwrap();

        // Caller A queries transactions to obtain an active cursor token
        let page_res = lez_node_ffi_poc::api_history::lez_query_transactions_by_account(
            handle,
            caller_a.as_ptr(),
            account.as_ptr(),
            std::ptr::null(),
            5,
            true,
        );
        assert_eq!(page_res.status, OperationStatus::Ok);
        assert!(!(*page_res.value).next_cursor.is_null());
        let cursor_ptr = (*page_res.value).next_cursor;

        // Caller B attempts to free Caller A's cursor -> PermissionDenied
        let denied = lez_free_cursor(handle, caller_b.as_ptr(), cursor_ptr);
        assert_eq!(denied, OperationStatus::PermissionDenied);

        // Caller A frees their own cursor -> Ok
        let ok = lez_free_cursor(handle, caller_a.as_ptr(), cursor_ptr);
        assert_eq!(ok, OperationStatus::Ok);

        // Freeing again -> NotFoundCursor
        let not_found = lez_free_cursor(handle, caller_a.as_ptr(), cursor_ptr);
        assert_eq!(not_found, OperationStatus::NotFoundCursor);

        // Clean up page
        lez_node_ffi_poc::core::types::lez_free_tx_page(page_res.value);
        lez_indexer_free(handle);
    }
}
