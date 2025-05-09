#!/bin/bash

CC=gcc
CXX=g++

for package_dir in examples/* ; do
    if [ -d $package_dir ]; then
        package=$(basename "$package_dir")
        cargo +nightly -v test --package $package

        if [[ $? -ne 0 ]]; then
            echo ">> ${package}: Test failed"
            exit 1
        fi
    fi
done
