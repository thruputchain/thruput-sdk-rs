#!/bin/bash

set -e

# Install the pinned Thruput CLI (dev.sh rv32 contract builds run
# `thruput build`, which lowers the ELF in-process). The helper skips the
# install when the pin is already on PATH and honors THRUPUT_CLI_ROOT, the
# Namespace-cached install root ci.yml sets.
. ./scripts/install-thruput-cli.sh

# groundhog 0.6 refuses to run without an active config profile, even for an
# offline `build`. A CI runner has no global config, so bootstrap
# a throwaway one. The guard keeps this a no-op for a developer running
# scripts/ci.sh locally: `config get` with no argument resolves the active
# profile and exits nonzero only when there isn't one.
if ! thruput config get --json >/dev/null 2>&1; then
    thruput config new ci --network local --use --global --quiet
fi

exec ./scripts/dev.sh ci
