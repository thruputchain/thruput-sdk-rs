#!/bin/bash

set -e # exit on error

./make.sh test
./make.sh doc

./make.sh examples build
./make.sh examples test

./make.sh integration_test_contracts build
./make.sh integration_test_contracts test

cargo run -p groundhog-erc20-if -F spec --bin show_spec >/dev/null
cargo run -p fibonacci -F spec --bin generate_spec -- /dev/null

echo "CI-CLEAN!"
