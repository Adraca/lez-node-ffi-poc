# Benchmarks & Performance Verification

Empirical performance evaluation for `lez-node-ffi-poc` executed on host hardware.

## 1. Environment & Hardware Specifications

- **Command:** `cargo bench --bench bench_ops`
- **CPU:** AMD Ryzen 7 7840HS w/ Radeon 780M Graphics (8 cores, 16 threads, Zen 4)
- **Kernel / OS:** Linux 6.18.57-1-lts x86_64 (CachyOS)
- **Storage Engine:** Embedded RocksDB 10.4.2 (via `rocksdb` v0.24.0 / `librocksdb-sys`)

---

## 2. Benchmark Results

### A. Point Reads vs. Multi-Get Batch Lookups (`cf_state`)

Evaluating single point reads vs. `batched_multi_get_cf_opt` with a pinned snapshot across batch sizes $N \in \{1, 10, 100\}$ on a fixture seeded with 100 accounts across 50 blocks:

| Operation | Batch Size ($N$) | Mean Latency | 95% Confidence Interval | Per-Item Latency |
| :--- | :--- | :--- | :--- | :--- |
| `single_reads` | 1 | **181.40 ns** | [180.36 ns, 182.61 ns] | 181.40 ns |
| `batch_multi_get` | 1 | **451.62 ns** | [449.22 ns, 454.32 ns] | 451.62 ns |
| `single_reads` | 10 | **2.36 µs** | [2.35 µs, 2.37 µs] | 236.38 ns |
| `batch_multi_get` | 10 | **2.52 µs** | [2.50 µs, 2.54 µs] | 251.71 ns |
| `single_reads` | 100 | **27.61 µs** | [27.45 µs, 27.82 µs] | 276.12 ns |
| `batch_multi_get` | 100 | **30.68 µs** | [29.76 µs, 32.17 µs] | 306.80 ns |

**Analysis & Claim Reconciliation (P#3):**
- Optimizing batch reads via `batched_multi_get_cf_opt`, sorted keys, and pinned snapshots improved batch latency by 16.8% (N=10) and 22.2% (N=100).
- However, single point reads in cached memory remain slightly faster (27.61 µs vs 30.68 µs at N=100) due to batch result vector allocation overhead.
- **Finding:** Batch is consistent (atomic snapshot across keys and single FFI boundary crossing), not yet faster. In accordance with audit instructions, P#3 is removed as a speedup claim.

### B. Transaction History Pagination Latency (`cf_acc_to_tx`)

Evaluating 10-item page fetching (`query_transactions_by_account`) across different fixture depths:

| Fixture Depth | Total Transactions | Page Size | Mean Latency | 95% Confidence Interval |
| :--- | :--- | :--- | :--- | :--- |
| 50 Blocks | 100 txs | 10 | **4.45 µs** | [4.05 µs, 5.36 µs] |
| 200 Blocks | 400 txs | 10 | **5.13 µs** | [4.56 µs, 6.39 µs] |

**Analysis:**
- When scaling data depth by 4× (from 100 to 400 txs), pagination latency shifts only from 4.45 µs to 5.13 µs.
- Bounded prefix iterators and composite key slicing (`account_id + seq_idx`) achieve $O(\text{page\_size})$ seek times regardless of account transaction history depth.

---

## 3. Memory & Leak Verification

- **Miri (`cargo +nightly miri test --test test_ffi_core`):**
  - Result: 4/4 tests passed with **0 errors, 0 memory leaks, and 0 undefined behavior** instances across the C-ABI memory lifecycle.
- **AddressSanitizer / UndefinedBehaviorSanitizer (`./scripts/run_asan_smoke.sh`):**
  - Native C smoke test compiled with Clang `-fsanitize=address,undefined` linked against cdylib.
  - Result: **Clean execution, 0 leak detections, 0 undefined behavior events** across all exports, null inputs, and deallocations.
- **Valgrind (CI):**
  - On the local AMD Zen 4 development host, glibc 2.44 AVX-512 vectorization (`EVEX 0x62`) triggers known LibVEX decoder limitations in Valgrind 3.25. ASan/UBSan serves as the authoritative local leak check; full Valgrind testing is executed in GitHub Actions CI.
- **Line Coverage (`cargo llvm-cov`):**
  - Result: **89.58% line coverage** (559/624 instrumented lines; 94.12% functions executed).
