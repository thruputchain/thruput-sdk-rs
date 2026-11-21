#!/usr/bin/env bash
# Fails unless the integration_test/contracts submodule is pinned to a commit
# on its repository's main branch. A pin to an unmerged branch builds today
# and is stranded when main moves on without that branch: the next bump to
# main silently drops whatever the SDK relied on from it.
#
# Usage: check-contracts-submodule-on-main.sh   (after `git submodule update --init`)
set -euo pipefail

SDK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONTRACTS="$SDK_DIR/integration_test/contracts"

if ! pin="$(git -C "$CONTRACTS" rev-parse --verify --quiet HEAD)"; then
    echo "error: integration_test/contracts is not checked out;" \
        "run git submodule update --init" >&2
    exit 1
fi

# Ancestry needs main's history, which a shallow checkout does not have.
if [ "$(git -C "$CONTRACTS" rev-parse --is-shallow-repository)" = true ]; then
    git -C "$CONTRACTS" fetch --quiet --unshallow origin main
else
    git -C "$CONTRACTS" fetch --quiet origin main
fi
main="$(git -C "$CONTRACTS" rev-parse FETCH_HEAD)"

if ! git -C "$CONTRACTS" merge-base --is-ancestor "$pin" "$main"; then
    echo "error: integration_test/contracts is pinned to $pin," \
        "which is not on the contracts repository's main ($main)." >&2
    echo "Merge that commit's branch there first, then pin a commit from main." >&2
    exit 1
fi
echo "integration_test/contracts pin $pin is on main"
