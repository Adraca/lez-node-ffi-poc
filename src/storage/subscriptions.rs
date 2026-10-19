use crate::core::policy::CallerId;
use crate::core::status::OperationStatus;
use crate::core::types::FfiChainTip;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Internal subscription representation (Audit Recommendation 3).
#[derive(Clone)]
pub struct Subscription {
    pub id: u64,
    pub caller_id: CallerId,
    pub from_anchor: u64,
    pub callback: unsafe extern "C" fn(tip: *const FfiChainTip, user_data: *mut std::ffi::c_void),
    pub user_data_addr: usize,
}

/// Manager tracking active finalized-block subscriptions with caller-isolation guarantees.
pub struct SubscriptionManager {
    subscriptions: RwLock<HashMap<u64, Subscription>>,
    next_id: AtomicU64,
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionManager {
    pub fn new() -> Self {
        Self {
            subscriptions: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// Registers a new finalized-block subscription bound to `caller_id`.
    pub fn subscribe(
        &self,
        caller_id: CallerId,
        from_anchor: u64,
        callback: unsafe extern "C" fn(tip: *const FfiChainTip, user_data: *mut std::ffi::c_void),
        user_data: *mut std::ffi::c_void,
    ) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let sub = Subscription {
            id,
            caller_id,
            from_anchor,
            callback,
            user_data_addr: user_data as usize,
        };
        let mut map = self.subscriptions.write();
        map.insert(id, sub);
        id
    }

    /// Unsubscribes a subscription. Enforces caller ownership: only the caller that opened
    /// the subscription can unsubscribe it.
    pub fn unsubscribe(&self, caller_id: &CallerId, subscription_id: u64) -> OperationStatus {
        let mut map = self.subscriptions.write();
        if let Some(sub) = map.get(&subscription_id) {
            if sub.caller_id != *caller_id {
                return OperationStatus::PermissionDenied;
            }
            map.remove(&subscription_id);
            OperationStatus::Ok
        } else {
            OperationStatus::NotFoundGeneric
        }
    }

    /// Number of active subscriptions.
    pub fn count(&self) -> usize {
        self.subscriptions.read().len()
    }

    /// Dispatches a finalized block event to active subscribers matching the height anchor.
    ///
    /// # Re-entrancy Guarantee:
    /// Clones the snapshot of matching subscriptions and drops the read lock BEFORE invoking
    /// foreign callbacks. Callbacks that perform re-entrant indexer queries or unsubscriptions
    /// will NEVER deadlock!
    pub fn notify_finalized_block(&self, tip: &FfiChainTip) {
        let snapshot: Vec<Subscription> = {
            let map = self.subscriptions.read();
            map.values()
                .filter(|s| tip.height >= s.from_anchor)
                .cloned()
                .collect()
        };

        for sub in snapshot {
            unsafe {
                (sub.callback)(tip, sub.user_data_addr as *mut std::ffi::c_void);
            }
        }
    }
}
