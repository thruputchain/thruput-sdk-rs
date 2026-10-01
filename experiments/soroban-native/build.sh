#!/usr/bin/env bash
# Build the soroban-native experiment into ./out:
#   out/sorhog-native.rv32a   Groundhog contract: soroban-env-host + the Soroban
#                             counter contract, both compiled natively to RV32
#   out/cd_*.hex              XDR HostFunction calldata (create-native/increment)
# Needs: the groundhog registry configured (see sdk_rs README) and a
# groundhog-rpc checkout for rv32-lower (GROUNDHOG_RPC, default
# ../../../groundhog-rpc).
set -euo pipefail
cd "$(dirname "$0")"
X=$PWD
OUT=$X/out
GROUNDHOG_RPC=${GROUNDHOG_RPC:-$X/../../../groundhog-rpc}
MEMORY_BYTES=${MEMORY_BYTES:-2097152}
STACK_BYTES=${STACK_BYTES:-262144}
mkdir -p "$OUT"

# 1. Vendor + patch soroban-env-host and soroban-sdk (see patches/).
vendor() { # crate version
  local dst=guest/vendor/$1-$2
  [ -d "$dst" ] && return
  mkdir -p guest/vendor
  local src
  src=$(ls -d "${CARGO_HOME:-$HOME/.cargo}"/registry/src/index.crates.io-*/"$1-$2" | head -1)
  cp -r "$src" "$dst"
  patch -d "$dst" -p1 -s < "patches/$1-$2.patch"
}
if [ ! -d guest/vendor/soroban-env-host-28.0.2 ] || [ ! -d guest/vendor/soroban-sdk-28.0.0 ]; then
  # Populate the registry cache with the pristine crates.
  tmp=$(mktemp -d)
  printf '[package]\nname="f"\nversion="0.0.0"\nedition="2021"\n[workspace]\n[dependencies]\nsoroban-env-host={version="=28.0.2",features=["recording_mode"]}\nsoroban-sdk="=28.0.0"\n' > "$tmp/Cargo.toml"
  mkdir "$tmp/src" && touch "$tmp/src/lib.rs"
  cargo fetch -q --manifest-path "$tmp/Cargo.toml"
  rm -rf "${tmp:?}"
fi
vendor soroban-env-host 28.0.2
vendor soroban-sdk 28.0.0

# 2. Guest: std via the zkvm PAL target, linked with the SDK runtime.
(cd guest && cargo rustc --release --target riscv32im-risc0-zkvm-elf \
  -Zbuild-std=std,panic_abort -Zbuild-std-features=compiler-builtins-mem --bin sorhog-native -- \
  -C link-arg=--defsym=__memory_size=$MEMORY_BYTES \
  -C link-arg=--defsym=__stack_size=$STACK_BYTES)

# 3. Lower ELF -> .rv32a with Groundhog's own lowerer.
(cd "$GROUNDHOG_RPC" && cargo build --release -q -p groundhog-rv32-lower)
"$GROUNDHOG_RPC/target/release/rv32-lower" \
  --elf guest/target/riscv32im-risc0-zkvm-elf/release/sorhog-native \
  --out "$OUT/sorhog-native.rv32a" --memory-bytes "$MEMORY_BYTES" --json

# 4. Calldata.
(cd tools/sorgen && cargo build --release -q)
G=tools/sorgen/target/release/sorgen
$G create-native soroban-counter > "$OUT/cd_create_native.hex"
$G increment 5 > "$OUT/cd_inc5.hex"
