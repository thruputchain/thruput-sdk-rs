#!/bin/bash
set -euo pipefail

# Per-contract rv32 codesize metrics: the on-chain-relevant numbers.
# .text is the stored code image; .rodata + .data form the stored initial
# data image (data_image); .bss is VM-zeroed (not stored) and reported for
# context. file is the raw size of the ELF as CI builds it — UNSTRIPPED
# since 2026-07-09 (gnaw's rv32 build sets CARGO_PROFILE_RELEASE_STRIP=false
# so the symbol checks can run), so it includes the symtab and is a loose
# upper bound, not an on-chain number; comparisons across that boundary see
# a one-time step. The section-based fields are unaffected by stripping.
#
# Input rows come from the package lists that gnaw's rv32 build tasks write
# (target/gnaw/rv32-packages/). The output format is append-only metric
# history: do not reorder or rename fields.
#
# Output format (one line per contract ELF):
#   <name> text=<B> rodata=<B> data=<B> bss=<B> data_image=<B> file=<B>

if [ $# -lt 1 ]; then
  echo "Usage: $0 <elf directory> [package list]..." >&2
  exit 1
fi

TARGET_DIR=$1
# The metric input set is the packages the rv32 build tasks built, as those
# tasks list them, NOT from globbing the ELF dir: the release dir is shared
# with root-workspace rv32 builds and can hold stale ELFs from renamed or
# removed contracts. None of those may emit a row into the append-only
# history, and a missing build for a listed contract must fail loudly.
if [ $# -gt 1 ]; then
    LISTS=("${@:2}")
else
    LISTS=(target/gnaw/rv32-packages/fixtures.txt
        target/gnaw/rv32-packages/integration.txt)
fi
for list in "${LISTS[@]}"; do
    [ -f "$list" ] || {
        echo "rv32 package list missing: $list (build with cargo gnaw run metrics-inputs)" >&2
        exit 1
    }
done

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
        echo "Expected rv32 contract ELF missing: $elf (build with cargo gnaw run metrics-inputs)" >&2
        exit 1
    fi
    text=$(section_size "$elf" .text)
    rodata=$(section_size "$elf" .rodata)
    data=$(section_size "$elf" .data)
    bss=$(section_size "$elf" .bss)
    file=$(wc -c < "$elf" | tr -d ' ')
    echo "$name text=$text rodata=$rodata data=$data bss=$bss data_image=$((rodata + data)) file=$file"
}

# Bin name = underscored package name. No duplicate-name guard here: a
# cross-workspace name collision reads the same ELF twice (identical rows);
# sdk_codesizes_rv32_lowered.sh is the collision tripwire (it exits 1).
packages=$(cat "${LISTS[@]}")
found=0
for name in $packages; do
    found=1
    emit_row "${name//-/_}"
done

if [ "$found" -eq 0 ]; then
    echo "No rv32 contracts listed (${LISTS[*]})" >&2
    exit 1
fi
