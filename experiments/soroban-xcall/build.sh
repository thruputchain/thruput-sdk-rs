#!/usr/bin/env bash
# Build the soroban-xcall experiment into ./out:
#   out/{counter,caller,token,router}.rv32a
#                        Groundhog contracts each hosting one Soroban contract
#   out/entry.rv32a      the trusted Soroban entrypoint (gh-entry/)
#   out/ghcounter.rv32a  the SDK's native Groundhog counter (example-contracts)
#   out/ghcaller.rv32a   a native Groundhog contract that calls Soroban
#                        (gh-caller/, built with `groundhog build`)
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

# 2. Guests: one Groundhog contract per Soroban contract (cargo feature).
GH_LOWER() { "$GROUNDHOG_RPC/target/release/rv32-lower" --elf "$1" --out "$2" --memory-bytes "$MEMORY_BYTES" --json; }
(cd "$GROUNDHOG_RPC" && cargo build --release -q -p groundhog-rv32-lower)
for c in counter caller token router; do
  (cd guest && cargo rustc --release --features "$c" --target riscv32im-risc0-zkvm-elf \
    -Zbuild-std=std,panic_abort -Zbuild-std-features=compiler-builtins-mem --bin sorhog-xcall -- \
    -C link-arg=--defsym=__memory_size=$MEMORY_BYTES \
    -C link-arg=--defsym=__stack_size=$STACK_BYTES)
  GH_LOWER guest/target/riscv32im-risc0-zkvm-elf/release/sorhog-xcall "$OUT/$c.rv32a"
done

# 3. Native Groundhog contracts, built the standard way.
G=${GROUNDHOG:-groundhog}
(cd gh-caller && "$G" build)
cp gh-caller/target/groundhog/gh_caller.rv32a "$OUT/ghcaller.rv32a"
(cd gh-entry && "$G" build)
cp gh-entry/target/groundhog/gh_entry.rv32a "$OUT/entry.rv32a"
(cd ../../example-contracts/counter && "$G" build)
cp ../../target/groundhog/counter.rv32a "$OUT/ghcounter.rv32a"

# 4. Calldata tool.
(cd tools/sorgen && cargo build --release -q)
