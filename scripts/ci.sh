#!/bin/bash

./make.sh test &&
  ./make.sh examples build &&
  ./make.sh examples test &&
  ./make.sh integration_test_contracts build &&
  ./make.sh integration_test_contracts test &&
  ./make.sh doc &&
  ./scripts/erc20-if-spec.sh &&
  echo "CI-CLEAN!"

