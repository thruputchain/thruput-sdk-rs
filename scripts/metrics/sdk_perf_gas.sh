#!/bin/bash

# We'll collect metrics from any example contract that has a bottle-test:

./scripts/dev.sh run-bottle-tests 2>/dev/null
