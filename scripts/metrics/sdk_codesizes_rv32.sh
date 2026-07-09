#!/bin/bash
set -euo pipefail

# Per-contract rv32 codesize metrics: the on-chain-relevant numbers.
# .text is the stored code image; .rodata + .data form the stored initial
# data image (data_image); .bss is VM-zeroed (not stored) and reported for
# context; file is the raw ELF size for continuity with sdk_codesizes.
#
# Output format (one line per contract ELF, append-only metric history —
# do not reorder or rename fields):
#   <name> text=<B> rodata=<B> data=<B> bss=<B> data_image=<B> file=<B>

if [ $# -lt 1 ] || [ $# -gt 2 ]; then
  echo "Usage: $0 <elf directory> [contracts dir]"
  exit 1
fi

TARGET_DIR=$1
# The metric input set is derived from the source tree's *-rv32 wrapper
# packages, NOT from globbing the ELF dir: the release dir is shared with
# root-workspace rv32 builds and can hold stale ELFs from renamed/removed
# wrappers — neither may emit a row into the append-only history, and a
# missing build for a current wrapper must fail loudly.
CONTRACTS_DIR=${2:-example-contracts}

# GNU readelf (Ubuntu CI runner) parses rv32 ELFs; llvm-readelf's default
# GNU output style matches. READELF env overrides.
READELF="${READELF:-$(command -v llvm-readelf || command -v readelf || true)}"

# Fail loudly rather than emit zero-valued lines into append-only history.
[ -n "$READELF" ] && command -v "$READELF" > /dev/null || {
    echo "no llvm-readelf or readelf on PATH (or set READELF)" >&2
    exit 1
}

section_size() {
    # Decimal size of section $2 in ELF $1; 0 if the section is absent.
    # readelf -SW line (after stripping the "[ N]" index):
    #   Name Type Addr Off Size ES Flg Lk Inf Al
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

found=0
for pkg_dir in "$CONTRACTS_DIR"/*-rv32; do
    [ -d "$pkg_dir" ] && [ -f "$pkg_dir/Cargo.toml" ] || continue
    found=1
    # Bin name = underscored package name by convention (the wrapper's
    # [[bin]] declares it; see dev.sh eg_build_rv32_all).
    name=$(basename "$pkg_dir")
    name=${name//-/_}
    elf="$TARGET_DIR/$name"
    if [ ! -f "$elf" ] || [ ! -x "$elf" ]; then
        echo "Expected rv32 contract ELF missing: $elf (build with scripts/dev.sh build-examples-rv32)" >&2
        exit 1
    fi
    text=$(section_size "$elf" .text)
    rodata=$(section_size "$elf" .rodata)
    data=$(section_size "$elf" .data)
    bss=$(section_size "$elf" .bss)
    file=$(wc -c < "$elf" | tr -d ' ')
    echo "$name text=$text rodata=$rodata data=$data bss=$bss data_image=$((rodata + data)) file=$file"
done

if [ "$found" -eq 0 ]; then
    echo "No *-rv32 wrapper packages found under $CONTRACTS_DIR" >&2
    exit 1
fi
