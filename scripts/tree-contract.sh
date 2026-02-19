#!/bin/bash

set -eu

package="${1:?Please specify a contract package to build}"

tree_contract() {
    (
        cd example-contracts && 

        cargo --config ../.cargo/contract.toml \
            tree \
            --package "$package" \
            --target "wasm32-unknown-unknown" \
            --edges features,no-build,no-dev \
            -Z build-std=core,alloc \
            -Z build-std-features=std/optimize_for_size \
            #
    )
}

tree_contract
