use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use crate::core::policy::PolicyEngine;
use crate::core::status::OperationStatus;
use crate::storage::cursors::CursorManager;
use crate::storage::RocksStore;

/// Core indexer instance managing storage, authorization policy, and active cursors.
pub struct LezIndexer {
    pub store: Arc<RocksStore>,
    pub policy: Arc<PolicyEngine>,
    pub cursors: Arc<CursorManager>,
    pub running: AtomicBool,
}

impl LezIndexer {
    pub fn open(path: &Path) -> Result<Self, OperationStatus> {
        let store = RocksStore::open(path)?;
        Ok(Self {
            store: Arc::new(store),
            policy: Arc::new(PolicyEngine::new()),
            cursors: Arc::new(CursorManager::new()),
            running: AtomicBool::new(true),
        })
    }

    #[inline]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn start(&self) -> OperationStatus {
        self.running.store(true, Ordering::SeqCst);
        OperationStatus::Ok
    }

    pub fn stop(&self) -> OperationStatus {
        self.running.store(false, Ordering::SeqCst);
        OperationStatus::Ok
    }

    pub fn reset_storage(&self) -> OperationStatus {
        // Simulates dropping and resetting column families
        OperationStatus::Ok
    }

    /// Explicitly frees a cursor token registered to the given caller.
    pub fn free_cursor(&self, caller: &[u8; 32], cursor: &str) -> OperationStatus {
        self.cursors.free_cursor(cursor, caller)
    }
}
