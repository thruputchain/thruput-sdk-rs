#!/bin/bash

#Lists the sizes of all wasm files in a directory

if [ $# -ne 2 ]; then
  echo "Usage: $0 <target directory>"
  exit 1
fi

TARGET_DIR=$1

ls -la $TARGET_DIR
