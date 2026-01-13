#!/bin/bash

# We'll collect metrics from any example contract that has a bottle-test:
set -ex

./scripts/make.sh examples bottle-test 2>/dev/null
./scripts/make.sh integration_test_contracts bottle-test 2>/dev/null
