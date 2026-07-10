#!/bin/bash

set -e

# Install the pinned groundhog CLI (dev.sh rv32 contract builds run
# `groundhog build --target rv32`, which lowers the ELF in-process).
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
if [ "$(groundhog --version 2>/dev/null | awk '{print $2}')" != "$GROUNDHOG_CLI_VERSION" ]; then
    cargo install groundhog-cli --version "=$GROUNDHOG_CLI_VERSION" \
        --registry groundhog --locked
fi

exec ./scripts/dev.sh ci
