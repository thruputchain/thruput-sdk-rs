#!/bin/bash

# Scaffold smoke test: verify that a contract scaffolded by the Thruput CLI
# this repo pins builds against the sdk at this checkout's head.
#
# sdk 0.17.8 shipped a feature problem (imported via an xdrs bump) that
# broke the `thruput new` scaffold's build without anything in this repo's
# CI noticing. This script exercises exactly the new-user path: install the
# pinned CLI, scaffold a project, point its sdk dependency at
# this checkout, and build the contract.
#
# The CLI is the version scripts/thruput-cli-version.sh pins, not the
# latest stable release. The pin can be a source commit before publication.
# `thruput build` refuses a contract whose SDK
# writes a raw spec format the CLI cannot read, and the SDK bumps that
# format before the stable CLI release that adopts it exists (sdk 0.38.0
# moved to format 3 while the newest stable CLI, 0.18.0, read format 2). An
# unpinned install then fails on that version skew, not on anything a change
# here could cause, until a stable CLI release catches up.
#
# Run by .github/workflows/scaffold_smoke.yml on pushes to main. Runnable
# locally too; set SKIP_CLI_INSTALL=1 to use the Thruput CLI already on
# PATH instead of installing the pin.

set -euo pipefail

SDK_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# Keep registry lookup explicit when building outside the SDK workspace.
export CARGO_REGISTRIES_THRUPUT_INDEX="sparse+https://crates.thruput.network/api/v1/crates/"

if [ "${SKIP_CLI_INSTALL:-0}" != 1 ]; then
    # Same pin and install path as scripts/ci.sh; the helper honors
    # THRUPUT_CLI_ROOT, the Namespace-cached install root the workflow
    # sets, so the CLI compiles once per pin bump rather than every run.
    . "$SDK_DIR/scripts/install-thruput-cli.sh"
fi
thruput --version

# groundhog 0.6 refuses to run without an active config profile, even for
# offline builds. Bootstrap a throwaway one on CI; the guard keeps this a
# no-op on a developer machine with a profile already selected.
if ! thruput config get --json >/dev/null 2>&1; then
    thruput config new scaffold-smoke --network local --use --global --quiet
fi

# Scaffold outside the sdk_rs tree: the generated manifest has no
# [workspace] table, so nested under this repo cargo would attach it to
# this workspace root and fail.
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT
cd "$WORK_DIR"
thruput new scaffold-smoke
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
    "s|^sdk = \{ version = \"[^\"]+\", registry = \"thruput\"|sdk = { path = \"$SDK_DIR/crates/sdk\"|" \
    Cargo.toml
rm Cargo.toml.bak
# Every inline declaration must now point at this checkout. Assert the path
# is present rather than that a registry reference is absent: an absence
# check would restate the rewrite's own assumptions about quoting and key
# order, and so pass on exactly the templates the rewrite silently skips —
# leaving the smoke test to build the published release and still succeed.
if grep '^sdk = ' Cargo.toml | grep -qvF "path = \"$SDK_DIR/crates/sdk\""; then
    echo "error: an '^sdk = ' line was not rewritten to this checkout;" >&2
    echo "the CLI template layout changed — update $0" >&2
    exit 1
fi

# A source-pinned CLI can scaffold an RPC client version that has not yet
# been published. Resolve it from the same verified checkout, preserving the
# template's version and features rather than substituting an older client.
if [ -n "${THRUPUT_CLI_SOURCE_DIR:-}" ]; then
    python3 - "$THRUPUT_CLI_SOURCE_DIR" <<'PY'
import json
from pathlib import Path
import sys

client = Path(sys.argv[1]) / 'crates/json-rpc/json-rpc-client'
assert (client / 'Cargo.toml').is_file(), 'pinned RPC client source is missing'
with Path('.cargo/config.toml').open('a') as config:
    config.write('\n[patch."sparse+https://crates.thruput.network/api/v1/crates/"]\n')
    config.write(f'groundhog-json-rpc-client = {{ path = {json.dumps(str(client))} }}\n')
PY
fi

# Build the contract, exactly as the scaffold's "next steps" suggest. RV32
# is the CLI's only artifact target, so the build takes no target flag: 0.9.6
# removed --target along with WASM. The scaffold's own rust-toolchain.toml
# pins the nightly + target; rustup installs them on first use. Host `cargo
# test` is deliberately out of scope: default features pull sdk/dev and with
# it the hogd FFI download.
thruput build
