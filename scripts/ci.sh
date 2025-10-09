#!/bin/bash

SDK_SCRIPT_DIR="$(dirname "$(realpath "$0")")"

# shellcheck source="scripts/env.sh"
source "${SDK_SCRIPT_DIR}/env.sh"

# shellcheck source="scripts/lib_make.sh"
source "${SDK_SCRIPT_DIR}/lib_make.sh"

echo "using CC: $CC"
echo "using CXX: $CXX"

export RUSTFLAGS="-D warnings"

ci_checks() {
    local - # save/restore shell options
    set -e # exit on error

    make test
    make doc

    make examples build
    make examples test

    make integration_test_contracts build
    make integration_test_contracts test

    make test-spec

    make xxx-clean
}

if ci_checks
then
    echo ">> CI-CLEAN!"
    exit 0
else
    echo "!! FAILED CI CHECKS"
    exit 1
fi
