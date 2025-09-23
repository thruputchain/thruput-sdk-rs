#!/bin/bash

SDK_SCRIPT_DIR="$(dirname "$(realpath "$0")")"

# shellcheck source="scripts/env.sh"
source "${SDK_SCRIPT_DIR}/env.sh"

# shellcheck source="scripts/lib_make.sh"
source "${SDK_SCRIPT_DIR}/lib_make.sh"

set -e # exit on error

make test
make doc

make examples build
make examples test

make integration_test_contracts build
make integration_test_contracts test

cargo run -p groundhog-erc20-if -F spec --bin show_spec >/dev/null
cargo run -p fibonacci -F spec --bin generate_spec -- /dev/null

echo "CI-CLEAN!"
