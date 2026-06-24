#!/usr/bin/env bash
# Asserts that a duplicate #[slot(N)] assignment fails to compile with the
# expected error message. The crate is workspace-excluded, so it is only
# ever built here.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="${SCRIPT_DIR}/../example-contracts/slot_collision_fail"

OUT=$(cd "$CRATE_DIR" && cargo build --features dev 2>&1)
STATUS=$?

if [ "$STATUS" -eq 0 ]; then
    echo "FAIL: expected a storage slot collision compile error, but build succeeded" >&2
    exit 1
fi
if ! echo "$OUT" | grep -q "storage slot 3 is assigned to more than one field"; then
    echo "FAIL: build failed but not with the slot collision message" >&2
    echo "$OUT" >&2
    exit 1
fi
echo "OK: storage slot collision detected as expected"
