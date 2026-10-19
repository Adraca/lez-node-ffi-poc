# Access Control Policy & Multi-Tenant Guarantees

## 1. Upstream Capability Model & Fallback

- **Target Architecture:** In the full Logos Basecamp / Logos Core deployment, caller identity verification
  and capability attenuation are delegated to `logos-capability-module` (tracking PR #37).
- **Standalone Fallback:** When running as a standalone node daemon or embedded FFI library without
  `logos-capability-module`, `lez-node-ffi-poc` enforces access control via an in-memory `PolicyEngine`
  initialized from `policy/reference_policy.json` (F#65, F#67).

## 2. Default Access Control Rules
1. **Administrative Operations Closed by Default (F#67):**
   - Calls to `lez_admin_start_indexer`, `lez_admin_stop_indexer`, and `lez_admin_reset_storage` return
     `OperationStatus::PermissionDenied` (400) unless the `caller_id` has been explicitly granted the admin role.
2. **Unauthenticated Calls Refused (F#68):**
   - A zeroed caller ID (`[0u8; 32]`) represents an unauthenticated caller and is rejected across all operations
     with `OperationStatus::InvalidArgumentCaller` (200).
3. **Public Read Operations (F#65):**
   - Authorized non-zero callers can execute chain queries (`query_chain_tip`, `query_account`, etc.).

## 3. Multi-Tenant Caller Isolation & Privacy (F#66, Privacy#4)
- **Scoped Cursors:** Cursors generated during `query_transactions_by_account` are bound cryptographically
  to the requesting `caller_id`.
- **Cross-Caller Rejection:** If Caller B attempts to pass a cursor token minted for Caller A, the node refuses
  with `OperationStatus::PermissionDenied`. One tenant cannot enumerate, hijack, or observe another tenant's
  pagination walks.
- **Concurrent Readers Non-Blocking:** RocksDB multi-threaded reader architecture ensures that concurrent read
  queries from independent callers proceed simultaneously without lock contention or thread starvation.
