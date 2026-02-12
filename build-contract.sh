#!/bin/bash

set -eu

package="${1:?Please specify a contract package to build}"

PROFILE="${2:-release}"
if [ "$PROFILE" = "dev" ]; then
    PROFILE_DIR="debug"
else
    PROFILE_DIR="$PROFILE"
fi

TARGET_DIR="target"
CONTRACT="${TARGET_DIR}/wasm32-unknown-unknown/${PROFILE_DIR}"
OPTIMIZED="${TARGET_DIR}/optimized"

build_contract() {
    (
        cd example-contracts && 

        cargo build -v \
            --package "$package" \
            --target "wasm32-unknown-unknown" \
            --profile "$PROFILE" \
            -Z unstable-options \
            -Z build-std=core,alloc \
            -Z build-std-features=std/optimize_for_size
    )
}

make_optimized() {
    mkdir -p "$OPTIMIZED" || return 1

    contract="${CONTRACT}/${package}.wasm"
    optimized="${OPTIMIZED}/${package}.wasm"

    # further compress the wasm (anecdotally, wasm-opt -Os and -Oz do only
    # very slightly better)
    wasm-opt -O3 --enable-bulk-memory -o "${optimized}" "${contract}"
}

printf "Building '%s' ...\n" "$package"

build_contract
contract="${CONTRACT}/${package}.wasm"
ls -lF "$contract"

make_optimized
contract_optimized="${OPTIMIZED}/${package}.wasm"
ls -lF "$contract_optimized"
