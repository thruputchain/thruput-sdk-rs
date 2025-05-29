#!/bin/bash

./make.sh test &&
  ./make.sh examples build &&
  ./make.sh examples test &&
  echo "CI-CLEAN!"

