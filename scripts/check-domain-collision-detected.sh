#!/usr/bin/env bash
# Asserts the generated storage-domain uniqueness test FAILS for a crate whose
# two roots share a domain. The crate is workspace-excluded, so it is only
# ever built here.
set -uo pipefail

CRATE_DIR="example-contracts/multi_root_collision"

OUT=$(cd "$CRATE_DIR" && cargo test --features dev 2>&1)
STATUS=$?

if [ "$STATUS" -eq 0 ]; then
    echo "FAIL: expected a storage domain collision, but tests passed" >&2
    echo "$OUT" >&2
    exit 1
fi
if ! echo "$OUT" | grep -q "storage domain collision"; then
    echo "FAIL: tests failed but not with the collision message" >&2
    echo "$OUT" >&2
    exit 1
fi
echo "OK: storage domain collision detected as expected"
