#!/usr/bin/env bash
# Asserts that a HOST `cargo build` of the workspace-excluded fixture
# example-contracts/dup_entrypoint_neg FAILS at codegen with a
# duplicate-symbol error on the crate-scoped entrypoint guards.
#
# This locks in the host-side duplicate-entrypoint defense: two contract
# blocks whose (interface name, major version, method name) collide produce
# one dispatch ID, and before the crate-scoped guards existed that crate
# compiled cleanly on the host — `cargo build`, `cargo test`, and the mock
# registry all accepted it (the mock silently last-write-won) — so the only
# loud failure was the rv32 guest build a contract author may never run
# while iterating. The rv32 side of the same fixture is exercised by
# check_rv32_duplicate_entrypoint_detected in scripts/dev.sh.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURE_DIR="${SCRIPT_DIR}/../example-contracts/dup_entrypoint_neg"

out=$(cd "$FIXTURE_DIR" && cargo build --locked --features dev 2>&1)
status=$?

if [ "$status" -eq 0 ]; then
    echo "FAIL: expected the host build of dup_entrypoint_neg to fail with a duplicate entrypoint symbol, but it succeeded" >&2
    exit 1
fi
if ! echo "$out" | grep -q "already defined"; then
    echo "FAIL: host build of dup_entrypoint_neg failed, but not with a duplicate-symbol error:" >&2
    echo "$out" >&2
    exit 1
fi
# The error must come from the crate-scoped guard (the host-side defense),
# not some other symbol clash.
if ! echo "$out" | grep -q "\[dup_entrypoint_neg@.*\]\` is already defined"; then
    echo "FAIL: duplicate-symbol error did not name a crate-scoped entrypoint guard:" >&2
    echo "$out" >&2
    exit 1
fi
echo "OK: host duplicate-entrypoint collision detected in dup_entrypoint_neg"
