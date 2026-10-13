#!/usr/bin/env bash
# Asserts that storage-domain collision tests FAIL for the workspace-excluded
# fixture crates. Two fixtures are checked:
#   1. multi_root_collision: two roots in one crate sharing a domain (within-crate).
#   2. cross_crate_collision/wrapper: two contract crates linked into one bin
#      wrapper, caught by thruput_sdk::storage_domain_uniqueness_test!()
#      (cross-crate).
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

check_collision_detected() {
    local crate_dir="$1"
    local test_cmd="$2"
    local out status
    out=$(cd "$crate_dir" && eval "$test_cmd" 2>&1)
    status=$?

    if [ "$status" -eq 0 ]; then
        echo "FAIL: expected a storage domain collision in $crate_dir, but tests passed" >&2
        echo "$out" >&2
        exit 1
    fi
    if ! echo "$out" | grep -q "storage domain collision"; then
        echo "FAIL: $crate_dir tests failed but not with the collision message" >&2
        echo "$out" >&2
        exit 1
    fi
    echo "OK: storage domain collision detected in $crate_dir"
}

# Within-crate: two roots sharing a domain in one crate (generated lib test).
check_collision_detected \
    "${SCRIPT_DIR}/../example-contracts/multi_root_collision" \
    "cargo test --features dev"

# Cross-crate: two contract crates linked into one bin-wrapper; caught by the
# wrapper's thruput_sdk::storage_domain_uniqueness_test!() aggregation test.
check_collision_detected \
    "${SCRIPT_DIR}/../example-contracts/cross_crate_collision/wrapper" \
    "cargo test"
