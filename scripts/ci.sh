#!/bin/bash

set -e # exit on error

./make.sh test
./make.sh doc

./make.sh examples build
./make.sh examples test

./make.sh integration_test_contracts build
./make.sh integration_test_contracts test

# This is broken
# ./scripts/erc20-if-spec.sh

cargo  run --features spec --manifest-path integration_test_contracts/fibonacci/Cargo.toml -- tmp.json

echo "CI-CLEAN!"
