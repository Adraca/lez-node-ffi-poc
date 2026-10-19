use std::collections::HashSet;
use std::sync::RwLock;

/// Represents an authorized caller ID (32-byte public key or zone hash).
pub type CallerId = [u8; 32];

/// Permission actions conforming to RFP-027 F#65, F#67.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    QueryRead,
    AdminStart,
    AdminStop,
    AdminReset,
}

/// Capability policy engine protecting administrative and read APIs.
pub struct PolicyEngine {
    admin_callers: RwLock<HashSet<CallerId>>,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyEngine {
    pub fn new() -> Self {
        Self {
            admin_callers: RwLock::new(HashSet::new()),
        }
    }

    /// Grants admin privilege to a specific caller ID.
    pub fn grant_admin(&self, caller: &CallerId) {
        let mut set = self.admin_callers.write().unwrap();
        set.insert(*caller);
    }

    /// Revokes admin privilege from a caller ID.
    pub fn revoke_admin(&self, caller: &CallerId) {
        let mut set = self.admin_callers.write().unwrap();
        set.remove(caller);
    }

    /// Verifies if a caller is authorized for a specific action (F#67, F#68).
    /// Read actions are open to any valid non-zero caller; admin actions are closed by default.
    pub fn check_permission(&self, caller: &CallerId, action: Action) -> bool {
        if *caller == [0u8; 32] {
            return false; // Zero caller is unauthenticated
        }
        match action {
            Action::QueryRead => true,
            Action::AdminStart | Action::AdminStop | Action::AdminReset => {
                let set = self.admin_callers.read().unwrap();
                set.contains(caller)
            }
        }
    }
}
