#!/bin/bash

set -e

# Install the pinned groundhog CLI (dev.sh rv32 contract builds run
# `groundhog build`, which lowers the ELF in-process).
# The version check makes re-runs with a warm ~/.cargo/bin free; the
# registry index + token are already configured (.cargo/config.toml,
# CARGO_REGISTRIES_GROUNDHOG_TOKEN).
. ./scripts/groundhog-cli-version.sh
# `cargo install` deliberately ignores the repo's .cargo/config.toml
# (install is global, not project-scoped), so the registry index must
# arrive via environment config. Keep in sync with .cargo/config.toml
# [registries]. Auth comes from CARGO_REGISTRIES_GROUNDHOG_TOKEN +
# CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS in ci.yml.
export CARGO_REGISTRIES_GROUNDHOG_INDEX="sparse+https://groundhog.dev/crates/api/v1/crates/"

# CI sets GROUNDHOG_CLI_ROOT to a Namespace-cached directory so the compiled
# binary survives across runs; the version check below then makes re-installs
# a no-op until scripts/groundhog-cli-version.sh bumps the pin. Unset (local
# runs): installs to the default cargo root, exactly as before.
install_flags=()
if [ -n "${GROUNDHOG_CLI_ROOT:-}" ]; then
    export PATH="${GROUNDHOG_CLI_ROOT}/bin:${PATH}"
    install_flags=(--root "${GROUNDHOG_CLI_ROOT}")
fi
if [ "$(groundhog --version 2>/dev/null | awk '{print $2}')" != "$GROUNDHOG_CLI_VERSION" ]; then
    cargo install groundhog-cli --version "=$GROUNDHOG_CLI_VERSION" \
        --registry groundhog --locked "${install_flags[@]}"
fi

# groundhog 0.6 refuses to run without an active config profile, even for an
# offline `build`. A CI runner has no global config, so bootstrap
# a throwaway one. The guard keeps this a no-op for a developer running
# scripts/ci.sh locally: `config get` with no argument resolves the active
# profile and exits nonzero only when there isn't one.
if ! groundhog config get --json >/dev/null 2>&1; then
    groundhog config new ci --network local --use --global --quiet
fi

exec ./scripts/dev.sh ci
