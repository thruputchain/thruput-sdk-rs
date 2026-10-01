#!/usr/bin/env bash
# Build everything for the soroban-in-groundhog experiment into ./out:
#   out/sorhog.rv32a        the Groundhog contract embedding soroban-env-host
#   out/counter.wasm        a Soroban counter contract to run inside it
#   out/cd_*.hex            XDR HostFunction calldata (upload/create/increment)
# Needs: the groundhog registry configured (see sdk_rs README), rustup with the
# wasm32v1-none target on stable, and a groundhog-rpc checkout for rv32-lower
# (GROUNDHOG_RPC, default ../../../groundhog-rpc).
set -euo pipefail
cd "$(dirname "$0")"
X=$PWD
OUT=$X/out
GROUNDHOG_RPC=${GROUNDHOG_RPC:-$X/../../../groundhog-rpc}
MEMORY_BYTES=${MEMORY_BYTES:-2097152}   # 2 MiB works; 1 MiB faults
STACK_BYTES=${STACK_BYTES:-262144}
mkdir -p "$OUT"

# 1. Soroban counter (Wasm). The env var skips stellar-cli's spec shaking,
#    which only affects the contractspec section, not execution.
(cd soroban-counter && SOROBAN_SDK_BUILD_SYSTEM_SUPPORTS_SPEC_SHAKING_V2=1 \
  cargo +stable build --release --target wasm32v1-none)
cp soroban-counter/target/wasm32v1-none/release/soroban_counter.wasm "$OUT/counter.wasm"

# 2. Vendor + patch soroban-env-host for the guest.
V=guest/vendor/soroban-env-host-28.0.2
if [ ! -d "$V" ]; then
  mkdir -p guest/vendor
  cargo fetch -q --manifest-path tools/native-check/Cargo.toml
  SRC=$(ls -d "${CARGO_HOME:-$HOME/.cargo}"/registry/src/index.crates.io-*/soroban-env-host-28.0.2 | head -1)
  cp -r "$SRC" "$V"
  patch -d "$V" -p1 < patches/soroban-env-host-28.0.2-vm-timer.patch
fi

# 3. Guest: std via the zkvm PAL target, linked with the SDK runtime.
(cd guest && cargo rustc --release --target riscv32im-risc0-zkvm-elf \
  -Zbuild-std=std,panic_abort -Zbuild-std-features=compiler-builtins-mem --bin sorhog -- \
  -C link-arg=--defsym=__memory_size=$MEMORY_BYTES \
  -C link-arg=--defsym=__stack_size=$STACK_BYTES)

# 4. Lower ELF -> .rv32a with Groundhog's own lowerer.
(cd "$GROUNDHOG_RPC" && cargo build --release -q -p groundhog-rv32-lower)
"$GROUNDHOG_RPC/target/release/rv32-lower" \
  --elf guest/target/riscv32im-risc0-zkvm-elf/release/sorhog \
  --out "$OUT/sorhog.rv32a" --memory-bytes "$MEMORY_BYTES" --json

# 5. Calldata.
(cd tools/sorgen && cargo build --release -q)
G=tools/sorgen/target/release/sorgen
$G upload "$OUT/counter.wasm" > "$OUT/cd_upload.hex"
$G create "$OUT/counter.wasm" > "$OUT/cd_create.hex"
$G increment 5 > "$OUT/cd_inc5.hex"
