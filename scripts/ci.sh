#!/bin/bash

SDK_SCRIPT_DIR="$(dirname "$(realpath "$0")")"
    # shellcheck source="scripts/env.sh"
source "${SDK_SCRIPT_DIR}/env.sh"
    # shellcheck source="scripts/lib_make.sh"
source "${SDK_SCRIPT_DIR}/lib_make.sh"

# prepare hogd cargo cache
(
CARGO_NET_GIT_FETCH_WITH_CLI=true ${SDK_SCRIPT_DIR}/../hogd/scripts/fetch.sh
)

# Below, each contract is built before being tested;
# this skips any attempt to rebuild
BOTTLE_TEST_USE_BUILT_TARGET=yes

echo "using CC: $CC"
echo "using CXX: $CXX"
echo "RUSTFLAGS=\"${RUSTFLAGS:-<unset>}\""

fail() {
    printf "\n!! CI-FAIL\n"
    exit 1
}

trap fail ERR;

# TODO: test aborts

make test
make doc

make examples build
make examples native-test
make examples bottle-test

# XXX: these still use deprecated symbols
# make integration_test_contracts build
# make integration_test_contracts native-test
# make integration_test_contracts bottle-test

make test-spec

make lint-check
make fmt-check
make xxx-check

printf "\n>> CI-CLEAN!\n"
