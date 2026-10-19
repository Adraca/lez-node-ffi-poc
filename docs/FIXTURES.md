# Deterministic Fixture Format

The RocksDB storage layer is seeded by `load_deterministic_fixtures(store, num_blocks, accounts)`
conforming to the upstream schema layout (`db66590ab8`).

## 1. Block & Hash Derivation
- **Block Height ($h$):** Sequential `u64` starting at height 1 up to $N$.
- **Block Hash:** 32-byte array with `h.to_be_bytes()` at bytes `0..8` and suffix `0xAA` at byte 31.
- **Timestamp:** Deterministic Unix timestamp: `1_700_000_000 + h * 12` seconds.
- **Column Families Updated:**
  - `cf_block`: Key = `h.to_be_bytes()`, Value = `[hash (32B), timestamp (8B)]`.
  - `cf_hash_to_id`: Key = `block_hash (32B)`, Value = `h.to_be_bytes()`.
  - `cf_meta`: Key = `b"chain_tip"`, Value = `[h (8B), hash (32B), timestamp (8B)]`.

## 2. Account State Model
- **Account Nonce:** `(h - 1) * 2 + 2`.
- **Account Balance:** `(acc_idx + 1) * 1000 + h * 10`.
- **Storage Layout in `cf_state`:**
  - Tip state: Key = `account_id (32B)`, Value = `[nonce (8B), balance (8B)]`.
  - Pinned state (height $h$): Key = `account_id (32B) + h.to_be_bytes() (8B)`, Value = `[nonce (8B), balance (8B)]`.

## 3. Transaction Indexing & Pagination
- **Per-Block Account Transactions:** 2 transactions per account per block.
  - `tx_hash_1`: `h.to_be_bytes()` + `acc_idx.to_be_bytes()` with discriminator byte `0x01`.
  - `tx_hash_2`: identical prefix with discriminator byte `0x02`.
- **Column Families Updated:**
  - `cf_tx_to_id`: Key = `tx_hash (32B)`, Value = `h.to_be_bytes()`.
  - `cf_acc_to_tx`: Key = `account_id (32B) + seq_idx.to_be_bytes() (8B)`, Value = `tx_hash (32B)`.
  - `cf_acc_meta`: Key = `account_id (32B)`, Value = `total_tx_count.to_be_bytes() (8B)`.
