#!/bin/bash
set -euo pipefail

# Builds the big-integer benchmark contract once per (backend, operation) pair
# for rv32, records each artifact's codesize, then measures each one's runtime
# gas and reports the diff.
#
# Both metrics come off the same artifact on purpose. Per-operation codesize is
# a marginal-linkage number -- what linking the operation adds over a build that
# does not call it -- and cannot be recovered from a binary that carries every
# operation at once, because LLVM shares helpers across them (`div` and `rem`
# are one routine; a Newton-iteration `sqrt` inlines a whole divide). The
# `none` variant is that baseline build, and every reported number is a
# difference against it.
#
# Usage:
#   scripts/bigint-bench.sh [-i ITERS] [-b BACKENDS] [-o OPS] [-k]
#
#   -i ITERS     kernel iterations per gas measurement (default 1000)
#   -b BACKENDS  space-separated backend subset (default: u256 uint)
#   -o OPS       space-separated operation subset; `none` is always built,
#                since every other number is measured against it
#   -k           keep going after a failed build instead of aborting
#
# Run from the SDK's top-level directory. Requires the pinned groundhog CLI
# (scripts/ci.sh installs it), llvm-readelf or readelf on PATH, and whatever
# `sdk`'s `dev` feature needs to stand up the mock node -- the same set the wasm
# bottle tests already require.

SDK_DIR="$(realpath ".")"
TARGET_DIR="$(realpath "target")"
CONTRACTS_DIR="${SDK_DIR}/example-contracts"
RV32_RELEASE="${TARGET_DIR}/riscv32im-unknown-none-elf/release"
LOWERED="${TARGET_DIR}/groundhog"
OUT_DIR="${TARGET_DIR}/bigint-bench"

# The contract package, its bin/artifact stem, and the ELF the cargo build
# leaves behind. `groundhog build` derives `--bin` from the package name.
PACKAGE="bigint-bench-contract"
STEM="bigint_bench_contract"

# Backends: the value is the cargo feature that selects it. `u256` is the
# default backend and needs no feature, so its entry is empty.
BACKENDS_ALL="u256 uint"
backend_feature() {
    case "$1" in
        u256) echo "" ;;
        uint) echo "backend-uint" ;;
        *) echo "unknown backend $1" >&2; return 1 ;;
    esac
}

# The operation menu. Keep in sync with the `op-*` features in
# example-contracts/bigint-bench-contract/Cargo.toml and the `kernels!` table in
# its src/kernel.rs -- a name here that has no matching feature fails the build
# loudly (cargo rejects an unknown `-F`), which is the intended behaviour.
#
# `none` first: it is the calibration build every other row subtracts.
OPS_ALL="none add sub mul mul_small widening_mul div div_small rem mul_div_u128 \
shl shr bitwise cmp sqrt bytes bit_len"

# The op name and its feature differ only in the separator.
op_feature() {
    echo "op-${1//_/-}"
}

ITERS=1000
BACKENDS="$BACKENDS_ALL"
OPS="$OPS_ALL"
KEEP_GOING=0

while getopts "i:b:o:kh" opt; do
    case "$opt" in
        i) ITERS="$OPTARG" ;;
        b) BACKENDS="$OPTARG" ;;
        o) OPS="$OPTARG" ;;
        k) KEEP_GOING=1 ;;
        h|*)
            sed -n '3,27p' "$0" >&2
            exit 0
            ;;
    esac
done

# Every number is a difference against the calibration build, so it is not
# optional even when the caller asked for a subset.
case " $OPS " in
    *" none "*) ;;
    *) OPS="none $OPS" ;;
esac

READELF="${READELF:-$(command -v llvm-readelf || command -v readelf || true)}"
[ -n "$READELF" ] && command -v "$READELF" > /dev/null || {
    echo "no llvm-readelf or readelf on PATH (or set READELF)" >&2
    exit 1
}

command -v groundhog > /dev/null || {
    echo "groundhog CLI not on PATH; scripts/ci.sh installs the pinned version" >&2
    exit 1
}

# Matches scripts/dev.sh: --locked unless CARGO_UNLOCK is set non-empty.
LOCKED="--locked"
[ -z "${CARGO_UNLOCK:-}" ] || LOCKED=""

section_size() {
    # Decimal size of section $2 in ELF $1; 0 if absent. Same readelf -SW
    # parse as scripts/metrics/sdk_codesizes_rv32.sh -- keep them in step.
    local hex
    hex=$("$READELF" -SW "$1" \
        | sed 's/^ *\[ *[0-9]*\] *//' \
        | awk -v s="$2" '$1 == s { print $5; exit }')
    if [ -n "$hex" ]; then
        echo "$((16#$hex))"
    else
        echo 0
    fi
}

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

CODESIZE_TSV="${OUT_DIR}/codesize.tsv"
# No raw-ELF file-size column: with the contract profile's strip="symbols" the
# ELF has no symtab, and what is left is padded to section alignment, so it
# reads identically across variants whose .text differs by hundreds of bytes.
# `.text` is the stored code image and `rv32a` is the deployed artifact; those
# are the on-chain numbers.
printf 'backend\top\ttext\trodata\tdata\tbss\trv32a\n' > "$CODESIZE_TSV"

failures=0

# Backend outer, operation inner: switching backends changes the dependency set
# (`uint` is an optional dependency, so a u256 build does not compile it), and
# alternating would rebuild it on every step.
for backend in $BACKENDS; do
    bfeature="$(backend_feature "$backend")"

    for op in $OPS; do
        features="$(op_feature "$op")"
        [ -z "$bfeature" ] || features="${features},${bfeature}"

        echo "=== BUILD(rv32) ${backend}.${op}  [-F ${features}]" >&2

        # build-std goes through as trailing cargo args, like scripts/dev.sh
        # build_rv32 and hogd's build_risc.sh: the rv32 rustflags use
        # panic=immediate-abort and target features the prebuilt core lacks.
        #
        # `-F` after the CLI's own flags is additive, and the CLI passes
        # --no-default-features, so the build gets exactly the op named here --
        # never the `op-none` default on top of it.
        #
        # strip=false keeps the symtab in the ELF this copies aside. It does not
        # change any size this reports (verified: identical .text/.rodata either
        # way), and it turns the kept ELF into a usable follow-up tool -- when a
        # row is surprising, `llvm-readelf -sW` on it attributes the bytes:
        #
        #   llvm-readelf -sW target/bigint-bench/uint.div_small.elf \
        #     | awk '$4=="FUNC" {print $3, $8}' | sort -rn | head
        #
        # which is how the compiler_builtins divide behind uint's div_small was
        # identified (see the design note). The deployed artifact is the .rv32a,
        # which carries no symtab regardless.
        if ! (
            cd "$CONTRACTS_DIR" &&
            groundhog build --target rv32 -p "$PACKAGE" -- \
                $LOCKED -Zbuild-std=core,alloc -F "$features" \
                --config 'profile.release.strip=false'
        ); then
            echo "FAILED to build ${backend}.${op}" >&2
            failures=$((failures + 1))
            [ "$KEEP_GOING" -eq 1 ] && continue
            exit 1
        fi

        elf="${RV32_RELEASE}/${STEM}"
        rv32a="${LOWERED}/${STEM}.rv32a"
        for f in "$elf" "$rv32a"; do
            [ -f "$f" ] || {
                echo "expected build output missing: $f" >&2
                exit 1
            }
        done

        # Every variant writes the same two paths, so copy each aside before the
        # next build overwrites them.
        cp "$elf" "${OUT_DIR}/${backend}.${op}.elf"
        cp "$rv32a" "${OUT_DIR}/${backend}.${op}.rv32a"

        printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
            "$backend" "$op" \
            "$(section_size "$elf" .text)" \
            "$(section_size "$elf" .rodata)" \
            "$(section_size "$elf" .data)" \
            "$(section_size "$elf" .bss)" \
            "$(wc -c < "$rv32a" | tr -d ' ')" \
            >> "$CODESIZE_TSV"
    done
done

if [ "$failures" -gt 0 ]; then
    echo "$failures build(s) failed; the report below is incomplete" >&2
fi

echo >&2
echo "=== MEASURE gas (${ITERS} iterations per operation)" >&2

# stdout: `name key=value ...` metric rows. stderr: the comparison table.
# Same split as scripts/metrics/sdk_perf_gas.sh, so this is safe to redirect
# into an append-only metric file.
cargo run --quiet -p bigint-bench --profile dev -- \
    --dir "$OUT_DIR" \
    --codesize "$CODESIZE_TSV" \
    --iters "$ITERS" \
    --baseline u256
