#!/usr/bin/env bash
# Execute the allocation-free exception fixture with a 16 KiB guest stack.
# Run only mapped handling here; normal-stack tests also exercise owned errors.
# Keep this build separate from normal artifacts and host dev/alloc features.
set -euo pipefail

SDK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STACK_TARGET="${SDK_DIR}/target/exception-stack"
command -v thruput >/dev/null || { echo 'thruput must be on PATH' >&2; exit 1; }

cd "${SDK_DIR}/fixtures"
thruput build --no-ui -p exn_catch --stack-size 16K \
    --target-dir "$STACK_TARGET" -- --locked -Zbuild-std=core,alloc

EXN_CATCH_RV32_ARTIFACT="${STACK_TARGET}/thruput/exn_catch.rv32a" \
    cargo test --locked -p exn_catch --test rv32 rv32_exn_catch_mapped -- --exact
