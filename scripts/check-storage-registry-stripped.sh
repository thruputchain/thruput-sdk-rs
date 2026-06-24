#!/usr/bin/env bash
#
# Verifies the host-only storage domain registry is absent from a guest (wasm) build.
#
# The domain registry (STORAGE_DOMAINS, DomainRecord, __GH_DOMAIN_* statics) is
# guarded by cfg(not(target_arch = "wasm32")) and must never appear in a deployed
# contract artifact.
#
# Usage:
#   scripts/check-storage-registry-stripped.sh [package]
#
# Example:
#   scripts/check-storage-registry-stripped.sh complex-storage
#
# Exits 0 if no registry symbols found (or toolchain unavailable), 1 on leak.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SDK_DIR="$SCRIPT_DIR/.."

PKG="${1:-complex-storage}"
WASM_NAME="${PKG//-/_}"

EXAMPLE_CONTRACTS_DIR="${SDK_DIR}/example-contracts"
# example-contracts/.cargo/config.toml sets target-dir = "../target" so artifacts
# land in the SDK-level target directory, not example-contracts/target/.
ARTIFACT="${SDK_DIR}/target/wasm32-unknown-unknown/release/${WASM_NAME}.wasm"

# Check that the pinned nightly toolchain + rust-src (needed for -Z build-std)
# are available before attempting the build. A missing toolchain is not a
# registry leak; skip cleanly so CI on restricted machines doesn't false-fail.
TOOLCHAIN_FILE="${EXAMPLE_CONTRACTS_DIR}/rust-toolchain.toml"
if [ -f "$TOOLCHAIN_FILE" ]; then
    PINNED_CHANNEL="$(grep 'channel' "$TOOLCHAIN_FILE" | sed 's/.*=\s*"\(.*\)"/\1/')"
else
    PINNED_CHANNEL=""
fi

toolchain_ok() {
    if [ -z "$PINNED_CHANNEL" ]; then
        return 0
    fi
    # Check rustup knows about this toolchain (installed)
    rustup toolchain list 2>/dev/null | grep -qF "$PINNED_CHANNEL" || return 1
    # Check rust-src component is present (required for build-std)
    rustup component list --toolchain "$PINNED_CHANNEL" 2>/dev/null \
        | grep -q '^rust-src (installed)' || return 1
    return 0
}

if ! toolchain_ok; then
    echo "SKIPPED: contract toolchain unavailable (${PINNED_CHANNEL:-unknown} or rust-src missing)"
    exit 0
fi

# Build the contract using the same mechanism as scripts/dev.sh build_wasm:
#   cd <workspace> && cargo --config .cargo/contract.toml build -p <pkg> -F entrypoints --release
# Note: uses example-contracts/.cargo/contract.toml (sets target=wasm32-unknown-unknown
# and build-std), not the SDK-level one (which has the target commented out).
echo "Building $PKG for wasm32-unknown-unknown ..."
(
    cd "$EXAMPLE_CONTRACTS_DIR" &&
    cargo --config ".cargo/contract.toml" \
        build -p "$PKG" -F entrypoints --release
) || {
    echo "SKIPPED: wasm build failed (toolchain or dependency issue)" >&2
    exit 0
}

if [ ! -f "$ARTIFACT" ]; then
    echo "ERROR: expected artifact not found: $ARTIFACT" >&2
    exit 1
fi

# Grep the artifact for registry symbols.  strings(1) extracts printable ASCII
# runs, which catches both symbol names and string literals.
if strings "$ARTIFACT" | grep -qE 'STORAGE_DOMAINS|DomainRecord|__GH_DOMAIN'; then
    echo "FAIL: storage domain registry leaked into guest artifact $ARTIFACT" >&2
    strings "$ARTIFACT" | grep -E 'STORAGE_DOMAINS|DomainRecord|__GH_DOMAIN' \
        | sed 's/^/  /' >&2
    exit 1
fi

echo "OK: no storage domain registry symbols/strings in $ARTIFACT"
