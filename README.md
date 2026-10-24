# lez-node-ffi-poc

[![CI Quality Gates](https://github.com/Adraca/lez-node-ffi-poc/actions/workflows/ci.yml/badge.svg)](https://github.com/Adraca/lez-node-ffi-poc/actions)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

A focused proof-of-concept Rust crate demonstrating the C-ABI FFI boundary, RocksDB multi-column-family storage, stable snapshot cursors, and multi-tenant capability isolation for the **Logos Execution Zone (LEZ) Node** under [Logos RFP-027](https://github.com/logos-co/rfp/pull/205).

> [!NOTE]
> **Proof of Approach, Not the Deliverable:** This crate is an independent technical demonstration submitted by Abhishek Khaparde as part of an RFP-027 grant application. It is not an official Logos Foundation release and has not been endorsed by Logos.

---

## Architecture & Upstream Alignment

The storage schema and indexing semantics mirror upstream [`logos-execution-zone`](https://github.com/logos-blockchain/logos-execution-zone) pinned at commit [`db66590ab821a4e142c211017a3866d007f6fa77`](https://github.com/logos-blockchain/logos-execution-zone/commit/db66590ab821a4e142c211017a3866d007f6fa77). See [docs/UPSTREAM.md](docs/UPSTREAM.md) and [docs/DECISIONS.md](docs/DECISIONS.md) for rationale.

```
+-----------------------------------------------------------+
|                   C-ABI FFI Boundary                      |
|  - PointerResult<T> with explicit free functions          |
|  - Typed 3-way OperationStatus (100.. / 200.. / 300..)    |
|  - catch_unwind panic barrier & null safety on all inputs |
+-----------------------------+-----------------------------+
                              |
+-----------------------------v-----------------------------+
|                 Policy Engine & Tenant Auth               |
|  - Admin methods closed by default (F#67)                 |
|  - Caller-scoped opaque cursors (F#66, Privacy#4)         |
+-----------------------------+-----------------------------+
                              |
+-----------------------------v-----------------------------+
|            RocksDB Multi-CF Engine (7 Families)           |
|  cf_block | cf_hash_to_id | cf_state | cf_tx_to_id        |
|  cf_acc_to_tx | cf_acc_meta | cf_meta                     |
+-----------------------------------------------------------+
```

---

## Spec Requirements Covered

| Spec ID | Requirement Description | Verification Test |
| :--- | :--- | :--- |
| **F#58–64** | C-ABI `PointerResult<T>`, 3-way status codes, null safety, free funcs | `test_ffi_core::*`, `test_c_api::*` |
| **F#10** | Default zeroed account state for unknown identifiers | `test_vertical_slice::test_query_account_default_zeroed_and_pinned_reads` |
| **F#11, F#14, R#3** | Pinned account state at block height with block hash verification | `test_vertical_slice::test_query_account_default_zeroed_and_pinned_reads` |
| **F#13, R#4** | Batch account retrieval via RocksDB `batched_multi_get_cf_opt` (consistent snapshot across keys; P#3 speedup removed: batch is consistent, not yet faster) | `test_vertical_slice::test_query_accounts_batch_multi_get` |
| **F#37–43, R#5** | Stable opaque cursors, dual ordering, concurrent writer safety | `test_vertical_slice::test_query_transactions_stable_under_concurrent_ingestion` |
| **F#47** | Query chain tip (height, hash, timestamp) | `test_vertical_slice::test_query_chain_tip_and_schema` |
| **F#58** | Machine-readable schema discovery (`query_schema`) | `test_vertical_slice::test_query_chain_tip_and_schema` |
| **F#65, F#67, F#68** | Admin closed by default, zeroed caller rejected, capability checks | `test_multi_tenant_privacy::test_admin_policy_closed_by_default_and_grant_flow` |
| **F#66, Privacy#4** | Cross-caller cursor reuse rejected with PermissionDenied | `test_multi_tenant_privacy::test_cross_caller_cursor_reuse_refused` |
| **F#70, R#5** | Multi-threaded concurrent reads do not block | `test_multi_tenant_privacy::test_concurrent_readers_do_not_block` |

---

## Not Implemented (Out of Scope for PoC)

1. **Event & Log Bloom Filters (F#20–29):** Topic filtering and contract event emission.
2. **Block Range Iteration (F#15–19):** Full block body header/payload traversal.
3. **Reactive WebSocket Subscriptions (F#48–57):** Push notifications for headers and log streams.
4. **zkVM Prover Pipelines:** SNARK/STARK verification delegated to base node runtime.
5. **Dynamic Capability Delegation:** Tracks upstream `logos-capability-module#37`.

---

## Quickstart

```bash
# Run full unit and integration test suite
cargo test --all-targets

# Run strict linter
cargo clippy --all-targets -- -D warnings

# Run Miri undefined-behavior verification on core memory layer
cargo +nightly miri test --test test_ffi_core

# Build release cdylib and compile C smoke test
cargo build --release
gcc -O2 tests/c/smoke_test.c -Ltarget/release -llez_node_ffi_poc -Wl,-rpath,target/release -Iinclude -o target/release/c_smoke_test
./target/release/c_smoke_test
```

---

## Benchmarks & Coverage

- **Coverage:** **66.22% line coverage** (939/1,418 lines, 58.27% functions [81/139], 68.51% regions [1,477/2,156]) across 36 integration & unit tests via `cargo llvm-cov`.
- **Latency (Criterion, AMD Zen 4):**
  - Point account query: **~164 ns**
  - Batch account query ($N=10$): **~2.69 µs** (~269 ns per entity)
  - Batch account query ($N=100$): **~30.12 µs** (~301 ns per entity)
  - Batch read finding: Batch is consistent across multi-key snapshots, not yet faster than N raw single reads in-memory.
  - Pagination page latency (10 items): **~4.41 µs** (100 txs) / **~4.31 µs** (400 txs)
  - Full details in [docs/BENCHMARKS.md](docs/BENCHMARKS.md).

---

## License

Dual-licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE).
