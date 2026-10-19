#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

echo "=== Building lez-node-ffi-poc (release) ==="
cargo build --release

echo "=== Compiling C smoke test with Clang ASan + UBSan ==="
clang -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer \
  tests/c/smoke_test.c \
  -Ltarget/release -llez_node_ffi_poc \
  -Wl,-rpath,"$REPO_ROOT/target/release" \
  -Iinclude \
  -o target/release/c_smoke_test_asan

echo "=== Executing ASan/UBSan smoke test ==="
./target/release/c_smoke_test_asan
echo "=== ASan/UBSan smoke test PASSED with 0 errors and 0 leaks ==="
