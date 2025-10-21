#!/bin/bash

# We'll collect metrics from any example contract that has a bottle-test:

./scripts/make.sh examples bottle-test 2>/dev/null
