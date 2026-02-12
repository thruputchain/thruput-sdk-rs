#!/bin/bash

cargo +nightly check \
        --color=always \
        --features "guest" \
        --target wasm32-unknown-unknown \
        --release \
        --verbose \
        -Zbuild-std=alloc,core \
        -Zbuild-std-features=panic_immediate_abort \
  |& less -RS
