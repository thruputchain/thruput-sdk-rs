#!/bin/bash
set -euo pipefail

# Per-contract rv32 gas for a fixed workload: one line per contract,
#   <contract>\tgas_consumed\t<gas>
#
# Run from the SDK's top-level directory, after `scripts/dev.sh
# build-examples-rv32` has written the lowered artifacts to target/thruput.
# example-contracts/sdk_perf_gas says what each row's workload is.
#
# The bottle tests that used to produce this metric were removed in #558;
# rows are now measured from the host (erc20, hasher) or through the
# *_caller contracts that replaced them, so the series steps down once at
# that boundary (no bottle-tester frame in the measurement). The
# integration_test rows (calldata_wrapper, fibonacci, read_only_test,
# sha256_wrapper) are not reported.

ARTIFACT_DIR="$(realpath "${1:-target/thruput}")"

cd example-contracts
cargo run --quiet --locked -p sdk_perf_gas -- "$ARTIFACT_DIR"
