# Memory Ownership and Lifetime Contract

This document formalizes the memory ownership, allocation, deallocation, and ordering semantics for `lez-node-ffi-poc` (RFP-027).

## Ownership Taxonomy

| Returned Type | Allocator | Paired Free Function | Recursive? | Interior Pointers Ownership |
| :--- | :--- | :--- | :--- | :--- |
| `PointerResult<T>` | Stack (by-value) | N/A (wrapper) | N/A | `value` owned on `Ok`; strictly `NULL` on error. |
| `FfiAccount` | Rust Global (`Box`) | `lez_free_account` | Yes | `shards` owned (`FfiShard[]`); each shard owns `data` (`u8[]`). |
| `FfiAccountsBatch` | Rust Global (`Box`) | `lez_free_accounts_batch` | Yes | `accounts` owned (`FfiAccount[]`); recurses into each account & shard. |
| `FfiTxPage` | Rust Global (`Box`) | `lez_free_tx_page` | Yes | `items` owned (`FfiTxDigest[]`); `next_cursor` owned (`c_char*`). |
| `FfiChainTip` | Rust Global (`Box`) | `lez_free_chain_tip` | No | Plain value; zero heap interior pointers. |
| `char*` (schema JSON) | Rust Global (`CString`) | `lez_free_string` | No | Owned string; caller must free. |
| `LezIndexerHandle` | Rust Global (`Box`) | `lez_indexer_free` | Yes | Closes RocksDB and releases cursor/subscription registries. |

## Core Invariants

### 1. PointerResult Null-on-Error Guarantee
- For any `PointerResult<T>`, `value` is guaranteed to be non-null and valid **only** when `status == OperationStatus::Ok`.
- Whenever `status != OperationStatus::Ok`, `value` is strictly guaranteed to be `NULL` (`nullptr`).
- Callers must check `status == Ok` before dereferencing `value`. On error, no deallocator call is required.

### 2. Nested Shard Ownership & Asymmetry Fix
- `FfiAccount` contains `shards: *mut FfiShard`, `shards_len: usize`, and `shards_cap: usize`.
- Each `FfiShard` contains `data: *mut u8`, `data_len: usize`, and `data_cap: usize`.
- Both tiers preserve exact capacity metadata to ensure `Vec::from_raw_parts` reconstruction during deallocation matches global allocator layout requirements with zero undefined behavior.
- **No Standalone Shard Free**: Shards are reachable only as interior fields of `FfiAccount` or `FfiAccountsBatch`. `lez_free_shard` is intentionally not exported to eliminate double-free footguns. Deallocating the parent account automatically and recursively deallocates all child shards and byte buffers.

### 3. Cursors: Dual Lifetimes and Free Ordering Protocol
A pagination cursor token has two distinct lifetimes:
1. **Host Memory Allocation**: The heap-allocated null-terminated C string at `FfiTxPage.next_cursor`.
2. **Server-Side Registry Entry**: The active session entry retained inside the indexer's `CursorManager`.

**Rules and Ordering Protocol:**
- `FfiTxPage` owns the heap allocation of `next_cursor`. Calling `lez_free_tx_page(page)` deallocates `page->next_cursor`.
- `lez_free_cursor(handle, caller, cursor)` borrows the cursor string to remove the server-side registry entry. It **does not** deallocate the string memory.
- **Ordering Constraint**: If an integrator chooses to release a cursor early before its TTL expires, the call order **must** be:
  ```c
  // 1. Unregister server-side cursor first (borrows page->next_cursor)
  lez_free_cursor(indexer, caller, page->next_cursor);
  // 2. Free page struct and string allocation second
  lez_free_tx_page(page);
  ```
  Calling `lez_free_cursor` *after* `lez_free_tx_page` dereferences a deallocated string and is undefined behavior (caught by ASan).

### 4. Batch vs Single Account Pointers
- `FfiAccountsBatch` owns the contiguous array of `FfiAccount` structs at `accounts`.
- Callers **must not** pass interior pointers (e.g., `&batch->accounts[i]`) to `lez_free_account`.
- The entire batch must be freed exclusively with `lez_free_accounts_batch(batch)`.

### 5. Trust Boundary & Caller Identity
- In this FFI proof-of-concept, `caller_ptr` (32 bytes) is accepted as a parameter.
- In the production Logos Core C++ module, caller identity **must be injected by the host runtime/module security context** and never accepted from unprivileged untrusted client input.
