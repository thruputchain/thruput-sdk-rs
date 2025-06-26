#!/bin/bash

./make.sh test &&
  ./make.sh examples build &&
  ./make.sh examples test &&
  ./make.sh doc &&
  echo "CI-CLEAN!"

