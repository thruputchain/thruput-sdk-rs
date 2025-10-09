#!/bin/bash

SDK_SCRIPT_DIR="$(dirname "$(realpath "$0")")"

# shellcheck source="scripts/env.sh"
source "${SDK_SCRIPT_DIR}/env.sh"

# shellcheck source="scripts/lib_make.sh"
source "${SDK_SCRIPT_DIR}/lib_make.sh"

# error on warning
export RUSTFLAGS="-D warnings"

echo "using CC: $CC"
echo "using CXX: $CXX"

cleanup() {
    echo "!! FAILED CI CHECKS"
}

trap cleanup EXIT;

set -o errexit

# TODO: test dynamic_bufs
# TODO: test invocation under wasm
# TODO: test aborts

make test
make doc

make examples build
make examples test

make integration_test_contracts build
make integration_test_contracts test

make test-spec

make fmt-check

make xxx-check

echo ">> CI-CLEAN!"
