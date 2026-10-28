#!/bin/bash

set -e

# Install the pinned Thruput CLI (rv32 contract builds run `thruput build`,
# which lowers the ELF in-process) and give it a config profile if it has
# none. Honors THRUPUT_CLI_ROOT, the Namespace-cached install root ci.yml sets.
cargo gnaw setup

# Everything CI runs, or the given gnaw targets (see `cargo gnaw list`).
if [ $# -eq 0 ]; then
    set -- ci
fi
exec cargo gnaw run "$@"
