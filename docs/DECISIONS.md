# Architectural Decisions: `lez-node-ffi-poc`

## 1. Direct Dependency vs. Thin Local Model

### Decision
Model the upstream `logos-execution-zone` storage types (`Account`, `Block`, `Transaction`, `Shard`)
and RocksDB column families locally in `lez-node-ffi-poc`, citing upstream paths and commit `db66590ab8`.

### Rationale
1. **Unpublished Monorepo Workspace:** The upstream `indexer_ffi` and `lee` crates in `logos-blockchain/logos-execution-zone`
   are not published on crates.io. They belong to a massive workspace containing zkVM provers (RISC0/SP1),
   cryptographic circuits, and Nix flake tooling.
2. **Build Isolation & Standalone Evidence:** Depending on `logos-execution-zone` via git pulls hundreds of
   megabytes of transitive zkVM dependencies, requires specialized compiler toolchains, and fails to build
   in lightweight CI environments.
3. **Spec Alignment:** Upstream types at commit `db66590ab8` lack critical RFP-027 requirements:
   - 3-way status code partition (`NotFound`, `InvalidArgument`, `BackendFailure`).
   - Catch-unwind panic boundaries.
   - Pinned block consistency checks (`query_account_at_block`).
   - Opaque, concurrent-safe pagination cursors with caller scoping.
4. **Clean C ABI:** By modeling the C-repr structs locally, we generate a standalone, standard C header
   via `cbindgen` that cleanly links against any C/C++ consumer without zkVM linkage baggage.

## 2. Storage Architecture & RocksDB Column Families

### Decision
Mirror the exact RocksDB column family layout from upstream `lez/storage/src/indexer/mod.rs`:
- `cf_block`: Block headers and bodies keyed by big-endian u64 height.
- `cf_meta`: Chain metadata keys (`last_block_in_db`, `first_block_in_db`, `tip_slot`).
- `cf_breakpoint`: Periodic state snapshots for fast historical reads.
- `cf_hash_to_id`: 32-byte block hash to height index.
- `cf_tx_to_id`: 32-byte transaction hash to height index.
- `cf_acc_meta`: 32-byte account ID to transaction count.
- `cf_acc_to_tx`: `account_id` + `index` composite key to 32-byte transaction hash.
- `cf_state`: Key-value state storage for account balances, nonces, and program shards.

## 3. Memory & FFI Contract (`PointerResult<T>`)

### Decision
All heap-allocated results cross the FFI boundary wrapped in `#[repr(C)] PointerResult<T>`.
Every returned pointer has an explicit free function (`free_pointer_result`, `free_ffi_vec`, `free_cstring`).
All entry points are wrapped with `std::panic::catch_unwind` to prevent unwinding into host C/C++ runtimes.

## 4. Multi-Tenant Caller Isolation & Access Control

### Decision
Every FFI call accepts a `caller_id` (`[u8; 32]`). Pagination cursors are tracked in an in-memory
session store scoped to `caller_id`. Cross-caller cursor access returns `OperationStatus::PermissionDenied` (403).
Administrative operations (`start_indexer`, `stop_indexer`, `reset_storage`) verify caller permissions
against a local capability policy engine conforming to the future `logos-capability-module#37` interface.
