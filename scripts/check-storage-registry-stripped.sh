#!/usr/bin/env bash
#
# Verifies the host-only storage domain registry is absent from a guest (rv32)
# build.
#
# The domain registry (STORAGE_DOMAINS, DomainRecord, __GH_DOMAIN_* statics) is
# guarded by cfg(not(target_arch = "riscv32")) and must never appear in a
# deployed contract artifact.
#
# Takes a contract package; its [[bin]] name is the underscored package name,
# which is the ELF name.
#
# Usage:
#   scripts/check-storage-registry-stripped.sh [package]
#
# Example:
#   scripts/check-storage-registry-stripped.sh complex-storage
#
# Outcomes:
#   toolchain unconfirmable          -> SKIPPED, exit 0
#   build OK + symbols absent        -> OK,      exit 0
#   build OK + symbols present       -> FAIL,    exit 1  (registry leaked)
#   toolchain present + build fails  -> ERROR,   exit 1

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SDK_DIR="$SCRIPT_DIR/.."

PKG="${1:-complex-storage}"
BIN_NAME="${PKG//-/_}"
RV32_TARGET="riscv32im-unknown-none-elf"

EXAMPLE_CONTRACTS_DIR="${SDK_DIR}/example-contracts"
# example-contracts/.cargo/config.toml sets target-dir = "../target" so artifacts
# land in the SDK-level target directory, not example-contracts/target/.
ARTIFACT="${SDK_DIR}/target/${RV32_TARGET}/release/${BIN_NAME}"

# --- SKIP path (exit 0) is reserved EXCLUSIVELY for an unconfirmable toolchain ---
#
# We must POSITIVELY confirm both the pinned nightly toolchain and its rust-src
# component (needed for -Z build-std) are installed. If we cannot confirm both --
# for any reason, including a missing/unparseable rust-toolchain.toml, an
# undeterminable channel, or missing rustup -- we SKIP (exit 0). We never let an
# unconfirmable toolchain fall through into a build.
#
# Once the toolchain IS confirmed, a build failure is a HARD ERROR (exit 1), not a
# skip: a regression that un-gates STORAGE_DOMAINS and breaks the rv32 build must
# be caught, not silently swallowed.

# `strings` is required to inspect the artifact; if absent, we cannot perform the
# check at all, so SKIP cleanly rather than fail confusingly mid-pipeline.
if ! command -v strings >/dev/null 2>&1; then
    echo "SKIPPED: contract toolchain unavailable/unverifiable ('strings' not found)"
    exit 0
fi

if ! command -v rustup >/dev/null 2>&1; then
    echo "SKIPPED: contract toolchain unavailable/unverifiable ('rustup' not found)"
    exit 0
fi

# rustup resolves rust-toolchain.toml by walking up from the build dir, so the
# pin can live at the SDK root or in example-contracts. Prefer the nearest.
if [ -f "${EXAMPLE_CONTRACTS_DIR}/rust-toolchain.toml" ]; then
    TOOLCHAIN_FILE="${EXAMPLE_CONTRACTS_DIR}/rust-toolchain.toml"
elif [ -f "${SDK_DIR}/rust-toolchain.toml" ]; then
    TOOLCHAIN_FILE="${SDK_DIR}/rust-toolchain.toml"
else
    echo "SKIPPED: contract toolchain unavailable/unverifiable (no rust-toolchain.toml found)"
    exit 0
fi

# Parse the pinned channel; the value must be non-empty and not contain
# whitespace (a sign the parse picked up garbage).
PINNED_CHANNEL="$(grep -E '^[[:space:]]*channel[[:space:]]*=' "$TOOLCHAIN_FILE" \
    | head -1 | sed -E 's/.*=[[:space:]]*"([^"]*)".*/\1/')"
if [ -z "$PINNED_CHANNEL" ] || [ "$PINNED_CHANNEL" != "${PINNED_CHANNEL%%[[:space:]]*}" ]; then
    echo "SKIPPED: contract toolchain unavailable/unverifiable (could not determine pinned channel)"
    exit 0
fi

# Confirm rustup actually has this toolchain installed.
if ! rustup toolchain list 2>/dev/null | grep -qF "$PINNED_CHANNEL"; then
    echo "SKIPPED: contract toolchain unavailable/unverifiable (toolchain '$PINNED_CHANNEL' not installed)"
    exit 0
fi

# Confirm rust-src is installed for it (required for -Z build-std).
if ! rustup component list --toolchain "$PINNED_CHANNEL" 2>/dev/null \
        | grep -q '^rust-src (installed)'; then
    echo "SKIPPED: contract toolchain unavailable/unverifiable (rust-src not installed for '$PINNED_CHANNEL')"
    exit 0
fi

# --- Toolchain confirmed: from here on, any failure is a real failure ---

# Build the ELF with plain cargo rather than through the groundhog CLI: this
# check only needs the linked guest binary, not the lowered artifact, so it
# stays runnable without the CLI installed. example-contracts/.cargo/config.toml
# supplies the rv32 rustflags; build-std goes on the command line (it must not
# live in [unstable] there — it would break host `cargo test` from that cwd).
echo "Building $PKG for $RV32_TARGET ..."
(
    cd "$EXAMPLE_CONTRACTS_DIR" &&
    cargo build -p "$PKG" -F entrypoints --release \
        --target "$RV32_TARGET" \
        -Zbuild-std=core,alloc \
        -Zbuild-std-features=std/optimize_for_size
) || {
    echo "ERROR: rv32 build failed; see output above" >&2
    exit 1
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
