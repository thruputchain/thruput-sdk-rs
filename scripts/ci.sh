#!/bin/bash

SDK_SCRIPT_DIR="$(dirname "$(realpath "$0")")"

# do the in-wasm "bottle test" instead of the native test
PREFER_BOTTLE_TEST=1

# below, each contract is built before being tested
BOTTLE_TEST_USE_BUILT_TARGET=1

# shellcheck source="scripts/env.sh"
source "${SDK_SCRIPT_DIR}/env.sh"

# shellcheck source="scripts/lib_make.sh"
source "${SDK_SCRIPT_DIR}/lib_make.sh"

echo "using CC: $CC"
echo "using CXX: $CXX"
echo "RUSTFLAGS=\"${RUSTFLAGS}\""

fail() {
    printf "\n!! CI-FAIL\n"
    exit 1
}

trap fail ERR;

# TODO: test aborts

make test
make doc

make examples build
make examples test

make integration_test_contracts build
PREFER_BOTTLE_TEST="" make integration_test_contracts test

make test-spec

make lint-check
make fmt-check
make xxx-check

printf "\n>> CI-CLEAN!\n"
