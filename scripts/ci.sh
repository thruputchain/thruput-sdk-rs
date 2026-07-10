#!/bin/bash

set -e

# Install the pinned groundhog CLI (dev.sh rv32 contract builds run
# `groundhog build --target rv32`, which lowers the ELF in-process).
# The version check makes re-runs with a warm ~/.cargo/bin free; the
# registry index + token are already configured (.cargo/config.toml,
# CARGO_REGISTRIES_GROUNDHOG_TOKEN).
. ./scripts/groundhog-cli-version.sh
if [ "$(groundhog --version 2>/dev/null | awk '{print $2}')" != "$GROUNDHOG_CLI_VERSION" ]; then
    cargo install groundhog-cli --version "=$GROUNDHOG_CLI_VERSION" \
        --registry groundhog --locked
fi

exec ./scripts/dev.sh ci
