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
