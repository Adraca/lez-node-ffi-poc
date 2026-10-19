# Upstream Ground Truth: logos-execution-zone

- **Pinned Commit:** `db66590ab821a4e142c211017a3866d007f6fa77` (2026-09-29, v0.3.0 merge)
- **Repository:** `https://github.com/logos-blockchain/logos-execution-zone`

## Upstream Files Inspected
- `lez/indexer/ffi/src/api/result.rs`: `PointerResult<Type, Error>` with raw pointers.
- `lez/indexer/ffi/src/errors.rs`: `OperationStatus` (Ok=0, NullPointer=1, Init=2, Client=3, Invalid=4).
- `lez/indexer/ffi/src/api/types/account.rs`: `FfiAccount`, `FfiShard`, `free_ffi_account`.
- `lez/storage/src/lib.rs`: `DBIO` trait, column families (`cf_block`, `cf_meta`).
- `lez/storage/src/indexer/mod.rs`: RocksDB CF setup (`cf_block`, `cf_meta`, `cf_breakpoint`,
  `cf_hash_to_id`, `cf_tx_to_id`, `cf_acc_meta`, `cf_acc_to_tx`, `cf_events`).
- `lez/storage/src/indexer/indexer_cells.rs`: cell structures and Borsh serialization.
- `lez/storage/src/indexer/read_multiple.rs`: `multi_get_cf` batch reading for blocks.
- `lez/storage/src/indexer/write_atomic.rs`: atomic batch writes (`cf_acc_to_tx` seq indexing).

## Exists in Upstream vs RFP-027 Requirements

| Feature / Area | Upstream (`lez/indexer/ffi`) | RFP-027 Spec (`F#`, `R#`, `P#`) |
|---|---|---|
| Status codes | 5 generic codes (`Ok`, `ClientError`, etc.) | 3-way split: Not-Found, Invalid-Arg, Backend-Failure |
| Panic safety | Direct invocation; panics cross FFI | Mandatory `catch_unwind` on every export |
| Account queries | Fails/errors on unknown account ID | Default zeroed account for unknown ID (`F#10`) |
| Pinned reads | Only unpinned tip reads | Pinned height + block hash verification (`F#11`, `F#14`) |
| Batch reads | Only batch block reads | Batch accounts via `batched_multi_get_cf_opt` (`F#13`, `R#4`; consistent, not yet faster) |
| Pagination | Sequential sub/limit, no stable token | Opaque cursors, dual ordering, concurrent safety (`F#37-43`) |
| Chain tip | Returns only `block_id: u64` | Returns tip height, block hash, and timestamp (`F#47`) |
| Introspection | None | Machine-readable `query_schema` JSON export (`F#58`) |
| Multi-tenancy | Single global context, zero caller scoping | Per-caller context, isolated cursors (`F#65-72`) |
| Access control | Admin calls (`start`, `stop`, `reset`) open | Admin operations closed by default, policy-gated (`F#67`) |
