#!/bin/bash
#
# Post-build check for embedded file-path strings in WASM contract binaries.
#
# These strings leak in from #[track_caller] or panic!() locations. In
# production WASM builds (panic=immediate-abort, no debug features)
# there should be none.
#
# Usage:
#   scripts/check-wasm-size.sh <wasm-dir>
#
# Example:
#   scripts/check-wasm-size.sh target/wasm32-unknown-unknown/release
#
# Exits 0 if no path strings found, 1 if any are present.

set -euo pipefail

if [ $# -ne 1 ]; then
    echo "Usage: $0 <wasm-dir>" >&2
    exit 1
fi

WASM_DIR="$1"
fail=0

# Contracts that are built with the "debugging" feature for testing
# (e.g., to match CrossExn wire format with the test runner).
# These are expected to contain embedded path strings.
ALLOW_LIST="exceptions swap_router"

is_allowed() {
    local name="$1"
    local base="${name%.wasm}"
    for allowed in $ALLOW_LIST; do
        if [ "$base" = "$allowed" ]; then
            return 0
        fi
    done
    return 1
}

for wasm in "$WASM_DIR"/*.wasm; do
    [ -f "$wasm" ] || continue
    name=$(basename "$wasm")

    rs_paths=$(strings "$wasm" | grep -E '\.rs$' || true)

    if [ -n "$rs_paths" ]; then
        if is_allowed "$name"; then
            echo "SKIP: $name (built with debugging)"
        else
            echo "FAIL: $name contains embedded file-path strings:"
            echo "$rs_paths" | sed 's/^/  /'
            fail=1
        fi
    else
        echo "OK:   $name"
    fi
done

echo

if [ "$fail" -ne 0 ]; then
    echo "Embedded path strings indicate #[track_caller] or panic locations"
    echo "leaking into the binary. See docs/wasm-size.md for details."
    exit 1
fi

echo "All clean."
