#!/bin/bash
set -euo pipefail

# Per-contract rv32 codesize metrics: the on-chain-relevant numbers.
# .text is the stored code image; .rodata + .data form the stored initial
# data image (data_image); .bss is VM-zeroed (not stored) and reported for
# context. file is the raw size of the ELF as CI builds it — UNSTRIPPED
# since 2026-07-09 (dev.sh build_rv32 sets CARGO_PROFILE_RELEASE_STRIP=false
# so the symbol checks can run), so it includes the symtab and is a loose
# upper bound, not an on-chain number; comparisons across that boundary see
# a one-time step. The section-based fields are unaffected by stripping.
#
# Input rows come from two sources (output format unchanged — append-only
# metric history, do not reorder or rename fields):
#   1. example-contracts *-rv32 glob (wrapper packages)
#   2. integration_test/rv32-contracts list file (explicit list, '#' comments)
#
# Output format (one line per contract ELF):
#   <name> text=<B> rodata=<B> data=<B> bss=<B> data_image=<B> file=<B>

if [ $# -lt 1 ] || [ $# -gt 3 ]; then
  echo "Usage: $0 <elf directory> [contracts dir] [itc contracts dir]" >&2
  exit 1
fi

TARGET_DIR=$1
# The metric input set is derived from the source tree's *-rv32 wrapper
# packages, NOT from globbing the ELF dir: the release dir is shared with
# root-workspace rv32 builds and can hold stale ELFs from renamed/removed
# wrappers — neither may emit a row into the append-only history, and a
# missing build for a current wrapper must fail loudly.
CONTRACTS_DIR=${2:-example-contracts}
# integration_test rv32 contracts are not *-rv32 wrapper packages; they are
# auto-discovered (same source-tree-derived input rationale as above).
ITC_CONTRACTS_DIR=${3:-integration_test/contracts}

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

emit_row() {
    # $1 = snake-case contract/bin name; ELF must exist in $TARGET_DIR.
    local name elf text rodata data bss file
    name=$1
    elf="$TARGET_DIR/$name"
    if [ ! -f "$elf" ] || [ ! -x "$elf" ]; then
        echo "Expected rv32 contract ELF missing: $elf (build with scripts/dev.sh build-examples-rv32 / build-integration-rv32)" >&2
        exit 1
    fi
    text=$(section_size "$elf" .text)
    rodata=$(section_size "$elf" .rodata)
    data=$(section_size "$elf" .data)
    bss=$(section_size "$elf" .bss)
    file=$(wc -c < "$elf" | tr -d ' ')
    echo "$name text=$text rodata=$rodata data=$data bss=$bss data_image=$((rodata + data)) file=$file"
}

found=0
for pkg_dir in "$CONTRACTS_DIR"/*-rv32; do
    [ -d "$pkg_dir" ] && [ -f "$pkg_dir/Cargo.toml" ] || continue
    found=1
    # Bin name = underscored package name by convention (the wrapper's
    # [[bin]] declares it; see dev.sh eg_build_rv32_all).
    name=$(basename "$pkg_dir")
    emit_row "${name//-/_}"
done

# No duplicate-name guard here: a cross-workspace name collision reads the same ELF twice (identical rows); sdk_codesizes_rv32_lowered.sh is the collision tripwire (it exits 1).
#
# integration_test rv32 discovery, mirroring dev.sh ls_itc_rv32_packages
# (keep the rules in sync): contract dirs minus the `disabled` marker,
# narrowed to packages declaring a [[bin]] (the rv32 wrapper bin is the
# enablement signal; raw contracts with hand-written entrypoints have no bin
# and are excluded
# by construction).
for pkg_dir in "$ITC_CONTRACTS_DIR"/*/; do
    pkg_dir=${pkg_dir%/}
    [ -f "$pkg_dir/Cargo.toml" ] || continue
    [ -f "$pkg_dir/disabled" ] && continue
    grep -q '^\[\[bin\]\]' "$pkg_dir/Cargo.toml" || continue
    name=$(basename "$pkg_dir")
    [ "$name" != "target" ] || continue
    found=1
    emit_row "${name//-/_}"
done

if [ "$found" -eq 0 ]; then
    echo "No *-rv32 wrapper packages under $CONTRACTS_DIR and no rv32-capable contracts under $ITC_CONTRACTS_DIR" >&2
    exit 1
fi
