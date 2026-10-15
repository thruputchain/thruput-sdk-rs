#!/usr/bin/env bash
# Execute the allocation-free exception fixture with a 16 KiB guest stack.
# Keep this build separate from normal artifacts and host dev/alloc features.
set -euo pipefail

SDK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STACK_TARGET="${SDK_DIR}/target/exception-stack"
command -v thruput >/dev/null || { echo 'thruput must be on PATH' >&2; exit 1; }

cd "${SDK_DIR}/example-contracts"
thruput build --no-ui -p exn_catch --stack-size 16K \
    --target-dir "$STACK_TARGET" -- --locked

EXN_CATCH_RV32_ARTIFACT="${STACK_TARGET}/thruput/exn_catch.rv32a" \
    cargo test --locked -p exn_catch --test rv32
