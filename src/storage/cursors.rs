use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use crate::core::policy::CallerId;
use crate::core::status::OperationStatus;

/// Default maximum number of concurrent active cursors retained in memory (Audit finding 1).
pub const DEFAULT_MAX_CURSORS: usize = 10_000;

/// Default time-to-live for pagination cursors (5 minutes).
pub const DEFAULT_CURSOR_TTL: Duration = Duration::from_secs(300);

/// Internal cursor state representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorState {
    pub caller_id: CallerId,
    pub account_id: [u8; 32],
    pub next_index: u64,
    pub snapshot_tip: u64,
    pub oldest_first: bool,
    pub created_at: Instant,
}

pub struct CursorManager {
    tokens: RwLock<HashMap<String, CursorState>>,
    nonce: RwLock<u64>,
    ttl: Duration,
    max_cursors: usize,
}

impl Default for CursorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorManager {
    pub fn new() -> Self {
        Self::with_config(DEFAULT_MAX_CURSORS, DEFAULT_CURSOR_TTL)
    }

    pub fn with_config(max_cursors: usize, ttl: Duration) -> Self {
        Self {
            tokens: RwLock::new(HashMap::new()),
            nonce: RwLock::new(1),
            ttl,
            max_cursors,
        }
    }

    /// Returns the number of currently active cursors in memory.
    pub fn count(&self) -> usize {
        self.tokens.read().unwrap().len()
    }

    /// Creates an opaque cursor token registered to the specific caller (F#66).
    /// Performs TTL expiration and max-count LRU/FIFO eviction (Audit finding 1).
    pub fn create_token(&self, mut state: CursorState) -> String {
        let mut nonce = self.nonce.write().unwrap();
        let id = *nonce;
        *nonce += 1;

        let token = format!(
            "cur_{}_{:016x}",
            if state.oldest_first { "asc" } else { "desc" },
            id
        );
        state.created_at = Instant::now();

        let mut map = self.tokens.write().unwrap();
        let now = Instant::now();

        // 1. Evict expired cursors
        map.retain(|_, s| now.duration_since(s.created_at) < self.ttl);

        // 2. Max-count eviction: drop oldest cursors if exceeding capacity limit
        while map.len() >= self.max_cursors {
            if let Some(oldest_token) = map.iter().min_by_key(|(_, s)| s.created_at).map(|(k, _)| k.clone()) {
                map.remove(&oldest_token);
            } else {
                break;
            }
        }

        map.insert(token.clone(), state);
        token
    }

    /// Resolves and validates an opaque cursor token against the caller ID.
    pub fn validate_token(
        &self,
        token: &str,
        caller_id: &CallerId,
        account_id: &[u8; 32],
    ) -> Result<CursorState, OperationStatus> {
        let map = self.tokens.read().unwrap();
        let state = map.get(token).ok_or(OperationStatus::NotFoundCursor)?;

        // Caller isolation (F#66, Privacy#4): Cannot use another caller's cursor
        if state.caller_id != *caller_id {
            return Err(OperationStatus::PermissionDenied);
        }

        // Account consistency check
        if state.account_id != *account_id {
            return Err(OperationStatus::InvalidArgumentAccount);
        }

        // TTL expiration check (Audit finding 1)
        if Instant::now().duration_since(state.created_at) >= self.ttl {
            return Err(OperationStatus::NotFoundCursor);
        }

        Ok(state.clone())
    }

    /// Explicitly frees an active cursor token (Audit finding 1).
    pub fn free_cursor(&self, token: &str, caller_id: &CallerId) -> OperationStatus {
        let mut map = self.tokens.write().unwrap();
        if let Some(state) = map.get(token) {
            if state.caller_id != *caller_id {
                return OperationStatus::PermissionDenied;
            }
            map.remove(token);
            OperationStatus::Ok
        } else {
            OperationStatus::NotFoundCursor
        }
    }
}
