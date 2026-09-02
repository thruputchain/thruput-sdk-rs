#!/bin/bash

# Scaffold smoke test: verify that a contract scaffolded by the *latest
# released* groundhog CLI builds against the sdk at this checkout's head.
#
# sdk 0.17.8 shipped a feature problem (imported via an xdrs bump) that
# broke the `groundhog new` scaffold's build without anything in this repo's
# CI noticing. This script exercises exactly the new-user path: install the
# CLI from the registry, scaffold a project, point its sdk dependency at
# this checkout, and build the contract.
#
# Run by .github/workflows/scaffold_smoke.yml on pushes to main. Runnable
# locally too; set SKIP_CLI_INSTALL=1 to use the groundhog CLI already on
# PATH instead of overwriting it with the latest release.

set -euo pipefail

SDK_DIR="$(cd "$(dirname "$0")/.." && pwd)"

if [ "${SKIP_CLI_INSTALL:-0}" != 1 ]; then
    # Deliberately unpinned, unlike scripts/ci.sh: the point is the CLI a
    # new user gets from `cargo install groundhog-cli` today. `cargo
    # install` ignores the repo's .cargo/config.toml (install is global,
    # not project-scoped), so the registry index must arrive via
    # environment config. Keep in sync with .cargo/config.toml
    # [registries].
    export CARGO_REGISTRIES_GROUNDHOG_INDEX="sparse+https://groundhog.dev/crates/api/v1/crates/"
    cargo install groundhog-cli --registry groundhog --locked
fi
groundhog --version

# groundhog 0.6 refuses to run without an active config profile, even for
# offline builds. Bootstrap a throwaway one on CI; the guard keeps this a
# no-op on a developer machine with a profile already selected.
if ! groundhog config get --json >/dev/null 2>&1; then
    groundhog config new scaffold-smoke --network local --use --global --quiet
fi

# Scaffold outside the sdk_rs tree: the generated manifest has no
# [workspace] table, so nested under this repo cargo would attach it to
# this workspace root and fail.
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT
cd "$WORK_DIR"
groundhog new scaffold-smoke
cd scaffold-smoke

# Point the scaffold's sdk dependency at this checkout instead of the
# published release the template pins. A path dep ignores the template's
# version requirement, so this keeps testing head across version bumps.
#
# The template declares `sdk` once per target (guest `no_std`, host `std`),
# so rewrite only the version+registry part of each line and leave the rest
# — notably the host line's `features = ["std"]` — intact. Rewriting whole
# lines would silently drop those features.
if ! grep -q '^sdk = ' Cargo.toml; then
    echo "error: no '^sdk = ' line in the scaffolded Cargo.toml;" >&2
    echo "the CLI template layout changed — update $0" >&2
    exit 1
fi
sed -i.bak -E \
    "s|^sdk = \{ version = \"[^\"]+\", registry = \"groundhog\"|sdk = { path = \"$SDK_DIR/crates/sdk\"|" \
    Cargo.toml
rm Cargo.toml.bak
# A template that still resolves sdk from the registry would test the
# published release instead of this checkout — the failure this script exists
# to prevent, and one that is invisible in a passing build.
if grep -q '^sdk = .*registry = "groundhog"' Cargo.toml; then
    echo "error: an '^sdk = ' line still resolves sdk from the registry;" >&2
    echo "the CLI template layout changed — update $0" >&2
    exit 1
fi

# Build the contract, exactly as the scaffold's "next steps" suggest. RV32
# is the CLI's only artifact target, so the build takes no target flag: 0.9.6
# removed --target along with WASM. The scaffold's own rust-toolchain.toml
# pins the nightly + target; rustup installs them on first use. Host `cargo
# test` is deliberately out of scope: default features pull sdk/dev and with
# it the hogd FFI download.
groundhog build
