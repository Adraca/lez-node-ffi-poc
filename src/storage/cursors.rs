use crate::core::policy::CallerId;
use crate::core::status::OperationStatus;
use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

/// Default maximum number of concurrent active cursors retained in memory (Audit finding 1).
pub const DEFAULT_MAX_CURSORS: usize = 10_000;

/// Default maximum number of active cursors any single caller may hold.
pub const DEFAULT_MAX_CURSORS_PER_CALLER: usize = 1_000;

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

struct Registry {
    /// token -> (creation id, state)
    tokens: HashMap<String, (u64, CursorState)>,
    /// creation id -> token, oldest first (ids are monotonic, so this is creation order)
    order: BTreeMap<u64, String>,
    /// caller -> (creation id -> token), for the per-caller cap
    per_caller: HashMap<CallerId, BTreeMap<u64, String>>,
    next_id: u64,
}

impl Registry {
    fn remove(&mut self, token: &str) -> Option<CursorState> {
        let (id, state) = self.tokens.remove(token)?;
        self.order.remove(&id);
        if let Some(set) = self.per_caller.get_mut(&state.caller_id) {
            set.remove(&id);
            if set.is_empty() {
                self.per_caller.remove(&state.caller_id);
            }
        }
        Some(state)
    }
}

/// Server-side cursor registry.
///
/// Every operation is O(log n): expiry pops from the front of the creation-ordered index, so
/// minting a cursor never scans the whole registry. Each caller has its own cap, so one caller
/// paging heavily evicts only its own oldest cursors, never another caller's (F#66, Privacy #4).
pub struct CursorManager {
    inner: RwLock<Registry>,
    ttl: Duration,
    max_cursors: usize,
    max_per_caller: usize,
}

impl Default for CursorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorManager {
    pub fn new() -> Self {
        Self::with_limits(
            DEFAULT_MAX_CURSORS,
            DEFAULT_MAX_CURSORS_PER_CALLER,
            DEFAULT_CURSOR_TTL,
        )
    }

    /// Total cap and TTL; the per-caller cap equals the total cap.
    pub fn with_config(max_cursors: usize, ttl: Duration) -> Self {
        Self::with_limits(max_cursors, max_cursors, ttl)
    }

    pub fn with_limits(max_cursors: usize, max_per_caller: usize, ttl: Duration) -> Self {
        Self {
            inner: RwLock::new(Registry {
                tokens: HashMap::new(),
                order: BTreeMap::new(),
                per_caller: HashMap::new(),
                next_id: 1,
            }),
            ttl,
            max_cursors: max_cursors.max(1),
            max_per_caller: max_per_caller.max(1),
        }
    }

    /// Returns the number of currently active cursors in memory.
    pub fn count(&self) -> usize {
        self.inner.read().tokens.len()
    }

    /// Creates an opaque cursor token registered to the specific caller (F#66).
    /// Expires stale cursors, then enforces the per-caller cap and the total cap,
    /// evicting oldest first (Audit finding 1).
    pub fn create_token(&self, mut state: CursorState) -> String {
        let mut reg = self.inner.write();
        let now = Instant::now();

        // 1. Expire from the oldest end only; stop at the first live cursor.
        while let Some((_, tok)) = reg.order.first_key_value() {
            let tok = tok.clone();
            let expired = reg
                .tokens
                .get(&tok)
                .map(|(_, s)| now.duration_since(s.created_at) >= self.ttl)
                .unwrap_or(true);
            if !expired {
                break;
            }
            reg.remove(&tok);
        }

        // 2. Per-caller cap: evict this caller's own oldest cursors.
        while reg.per_caller.get(&state.caller_id).map_or(0, |s| s.len()) >= self.max_per_caller {
            let oldest = reg
                .per_caller
                .get(&state.caller_id)
                .and_then(|s| s.first_key_value())
                .map(|(_, t)| t.clone());
            match oldest {
                Some(t) => {
                    reg.remove(&t);
                }
                None => break,
            }
        }

        // 3. Total cap: evict globally oldest.
        while reg.tokens.len() >= self.max_cursors {
            let oldest = reg.order.first_key_value().map(|(_, t)| t.clone());
            match oldest {
                Some(t) => {
                    reg.remove(&t);
                }
                None => break,
            }
        }

        let id = reg.next_id;
        reg.next_id += 1;
        let token = format!(
            "cur_{}_{:016x}",
            if state.oldest_first { "asc" } else { "desc" },
            id
        );
        state.created_at = now;
        let caller = state.caller_id;
        reg.order.insert(id, token.clone());
        reg.per_caller
            .entry(caller)
            .or_default()
            .insert(id, token.clone());
        reg.tokens.insert(token.clone(), (id, state));
        token
    }

    /// Resolves and validates an opaque cursor token against the caller ID.
    pub fn validate_token(
        &self,
        token: &str,
        caller_id: &CallerId,
        account_id: &[u8; 32],
    ) -> Result<CursorState, OperationStatus> {
        let reg = self.inner.read();
        let (_, state) = reg
            .tokens
            .get(token)
            .ok_or(OperationStatus::NotFoundCursor)?;

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
        let mut reg = self.inner.write();
        match reg.tokens.get(token) {
            Some((_, state)) if state.caller_id != *caller_id => OperationStatus::PermissionDenied,
            Some(_) => {
                reg.remove(token);
                OperationStatus::Ok
            }
            None => OperationStatus::NotFoundCursor,
        }
    }
}
