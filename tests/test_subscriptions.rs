use lez_node_ffi_poc::api::*;
use lez_node_ffi_poc::core::status::OperationStatus;
use lez_node_ffi_poc::core::types::{FfiChainTip, LEZ_ABI_VERSION_MAJOR};
use std::ffi::{c_void, CString};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

struct CallerContext {
    deliveries: AtomicU64,
    last_height: AtomicU64,
}

unsafe extern "C" fn test_callback(tip: *const FfiChainTip, user_data: *mut c_void) {
    if !tip.is_null() && !user_data.is_null() {
        let ctx = &*(user_data as *const CallerContext);
        ctx.deliveries.fetch_add(1, Ordering::SeqCst);
        ctx.last_height.store((*tip).height, Ordering::SeqCst);
    }
}

struct ReentrantContext {
    handle: *mut LezIndexerHandle,
    caller: [u8; 32],
    read_success: AtomicBool,
    reentrant_unsubscribed: AtomicBool,
    sub_id: AtomicU64,
}

unsafe impl Send for ReentrantContext {}
unsafe impl Sync for ReentrantContext {}

unsafe extern "C" fn reentrant_callback(_tip: *const FfiChainTip, user_data: *mut c_void) {
    if !user_data.is_null() {
        let ctx = &*(user_data as *const ReentrantContext);

        // 1. Re-entrant read: lez_query_schema must NOT deadlock!
        let schema_res =
            lez_node_ffi_poc::api_history::lez_query_schema(ctx.handle, ctx.caller.as_ptr());
        if schema_res.status == OperationStatus::Ok && !schema_res.value.is_null() {
            ctx.read_success.store(true, Ordering::SeqCst);
            lez_node_ffi_poc::core::memory::lez_free_string(schema_res.value);
        }

        // 2. Re-entrant write/unsubscribe: lez_unsubscribe must NOT deadlock!
        let id = ctx.sub_id.load(Ordering::SeqCst);
        if id > 0 {
            let unsub_res = lez_unsubscribe(ctx.handle, ctx.caller.as_ptr(), id);
            if unsub_res == OperationStatus::Ok {
                ctx.reentrant_unsubscribed.store(true, Ordering::SeqCst);
            }
        }
    }
}

#[test]
fn test_subscription_caller_isolation_and_unsubscribe() {
    let dir = tempdir().unwrap();
    let dir_c = CString::new(dir.path().to_str().unwrap()).unwrap();
    let res = unsafe { lez_indexer_create(dir_c.as_ptr(), LEZ_ABI_VERSION_MAJOR) };
    assert_eq!(res.status, OperationStatus::Ok);
    let handle = res.value;

    let caller_a = [0xAA; 32];
    let caller_b = [0xBB; 32];

    let ctx_a = Arc::new(CallerContext {
        deliveries: AtomicU64::new(0),
        last_height: AtomicU64::new(0),
    });
    let ctx_b = Arc::new(CallerContext {
        deliveries: AtomicU64::new(0),
        last_height: AtomicU64::new(0),
    });

    let sub_a = unsafe {
        lez_subscribe_finalized_blocks(
            handle,
            caller_a.as_ptr(),
            1, // anchor from height 1
            Some(test_callback),
            Arc::as_ptr(&ctx_a) as *mut c_void,
        )
    };
    assert!(sub_a > 0);

    let sub_b = unsafe {
        lez_subscribe_finalized_blocks(
            handle,
            caller_b.as_ptr(),
            5, // anchor from height 5
            Some(test_callback),
            Arc::as_ptr(&ctx_b) as *mut c_void,
        )
    };
    assert!(sub_b > 0);

    // Ingest blocks 1..4
    for h in 1..=4 {
        let hash = [h as u8; 32];
        unsafe {
            assert_eq!(
                lez_notify_block_ingest(handle, h, hash.as_ptr(), 1000 + h),
                OperationStatus::Ok
            );
        }
    }

    // Caller A must have received blocks 1..4 (deliveries == 4)
    // Caller B must have received 0 because from_anchor == 5
    assert_eq!(ctx_a.deliveries.load(Ordering::SeqCst), 4);
    assert_eq!(ctx_a.last_height.load(Ordering::SeqCst), 4);
    assert_eq!(ctx_b.deliveries.load(Ordering::SeqCst), 0);

    // Ingest block 5: both must receive
    let hash5 = [5u8; 32];
    unsafe {
        assert_eq!(
            lez_notify_block_ingest(handle, 5, hash5.as_ptr(), 1005),
            OperationStatus::Ok
        );
    }
    assert_eq!(ctx_a.deliveries.load(Ordering::SeqCst), 5);
    assert_eq!(ctx_b.deliveries.load(Ordering::SeqCst), 1);
    assert_eq!(ctx_b.last_height.load(Ordering::SeqCst), 5);

    // Caller B attempts to unsubscribe Caller A's subscription -> PermissionDenied!
    let unauthorized_unsub = unsafe { lez_unsubscribe(handle, caller_b.as_ptr(), sub_a) };
    assert_eq!(unauthorized_unsub, OperationStatus::PermissionDenied);

    // Caller A unsubscribes their own subscription -> Ok
    let authorized_unsub = unsafe { lez_unsubscribe(handle, caller_a.as_ptr(), sub_a) };
    assert_eq!(authorized_unsub, OperationStatus::Ok);

    // Ingest block 6: only Caller B should receive
    let hash6 = [6u8; 32];
    unsafe {
        assert_eq!(
            lez_notify_block_ingest(handle, 6, hash6.as_ptr(), 1006),
            OperationStatus::Ok
        );
    }
    assert_eq!(ctx_a.deliveries.load(Ordering::SeqCst), 5); // Unsubscribed: no increase!
    assert_eq!(ctx_b.deliveries.load(Ordering::SeqCst), 2); // Still active: received!
    assert_eq!(ctx_b.last_height.load(Ordering::SeqCst), 6);

    unsafe {
        assert_eq!(
            lez_unsubscribe(handle, caller_b.as_ptr(), sub_b),
            OperationStatus::Ok
        );
        lez_indexer_free(handle);
    }
}

#[test]
fn test_reentrant_read_and_unsubscribe_does_not_deadlock() {
    let dir = tempdir().unwrap();
    let dir_c = CString::new(dir.path().to_str().unwrap()).unwrap();
    let res = unsafe { lez_indexer_create(dir_c.as_ptr(), LEZ_ABI_VERSION_MAJOR) };
    assert_eq!(res.status, OperationStatus::Ok);
    let handle = res.value;

    let caller = [0x55; 32];
    let reentrant_ctx = Arc::new(ReentrantContext {
        handle,
        caller,
        read_success: AtomicBool::new(false),
        reentrant_unsubscribed: AtomicBool::new(false),
        sub_id: AtomicU64::new(0),
    });

    let sub_id = unsafe {
        lez_subscribe_finalized_blocks(
            handle,
            caller.as_ptr(),
            1,
            Some(reentrant_callback),
            Arc::as_ptr(&reentrant_ctx) as *mut c_void,
        )
    };
    assert!(sub_id > 0);
    reentrant_ctx.sub_id.store(sub_id, Ordering::SeqCst);

    // Trigger ingest: invokes reentrant_callback on the current thread
    let hash = [0x77; 32];
    let st = unsafe { lez_notify_block_ingest(handle, 10, hash.as_ptr(), 1010) };
    assert_eq!(st, OperationStatus::Ok);

    // Assert re-entrant calls succeeded without deadlocking!
    assert!(reentrant_ctx.read_success.load(Ordering::SeqCst));
    assert!(reentrant_ctx.reentrant_unsubscribed.load(Ordering::SeqCst));

    unsafe {
        lez_indexer_free(handle);
    }
}

#[test]
fn test_fixture_ingest_thread_parallel_dispatch() {
    let dir = tempdir().unwrap();
    let dir_c = CString::new(dir.path().to_str().unwrap()).unwrap();
    let res = unsafe { lez_indexer_create(dir_c.as_ptr(), LEZ_ABI_VERSION_MAJOR) };
    assert_eq!(res.status, OperationStatus::Ok);
    let handle = res.value;

    let caller = [0x88; 32];
    let ctx = Arc::new(CallerContext {
        deliveries: AtomicU64::new(0),
        last_height: AtomicU64::new(0),
    });

    let sub_id = unsafe {
        lez_subscribe_finalized_blocks(
            handle,
            caller.as_ptr(),
            1,
            Some(test_callback),
            Arc::as_ptr(&ctx) as *mut c_void,
        )
    };
    assert!(sub_id > 0);

    // Spawn fixture ingest thread producing 20 blocks
    let raw_handle_usize = handle as usize;
    let ingest_thread = thread::spawn(move || {
        let h = raw_handle_usize as *mut LezIndexerHandle;
        for i in 1..=20 {
            let hash = [i as u8; 32];
            unsafe {
                lez_notify_block_ingest(h, i, hash.as_ptr(), 1000 + i);
            }
            thread::sleep(Duration::from_millis(2));
        }
    });

    ingest_thread.join().unwrap();

    assert_eq!(ctx.deliveries.load(Ordering::SeqCst), 20);
    assert_eq!(ctx.last_height.load(Ordering::SeqCst), 20);

    unsafe {
        assert_eq!(
            lez_unsubscribe(handle, caller.as_ptr(), sub_id),
            OperationStatus::Ok
        );
        lez_indexer_free(handle);
    }
}
