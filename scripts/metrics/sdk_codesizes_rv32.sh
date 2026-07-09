#!/bin/bash

# Per-contract rv32 codesize metrics: the on-chain-relevant numbers.
# .text is the stored code image; .rodata + .data form the stored initial
# data image (data_image); .bss is VM-zeroed (not stored) and reported for
# context; file is the raw ELF size for continuity with sdk_codesizes.
#
# Output format (one line per contract ELF, append-only metric history —
# do not reorder or rename fields):
#   <name> text=<B> rodata=<B> data=<B> bss=<B> data_image=<B> file=<B>

if [ $# -ne 1 ]; then
  echo "Usage: $0 <elf directory>"
  exit 1
fi

TARGET_DIR=$1

# GNU readelf (Ubuntu CI runner) parses rv32 ELFs; llvm-readelf's default
# GNU output style matches. Trailing path is the macOS/homebrew fallback.
READELF="${READELF:-$(command -v llvm-readelf || command -v readelf || echo /opt/homebrew/opt/llvm/bin/llvm-readelf)}"

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
for elf in "$TARGET_DIR"/*; do
    [ -f "$elf" ] && [ -x "$elf" ] || continue
    found=1
    name=$(basename "$elf")
    text=$(section_size "$elf" .text)
    rodata=$(section_size "$elf" .rodata)
    data=$(section_size "$elf" .data)
    bss=$(section_size "$elf" .bss)
    file=$(wc -c < "$elf" | tr -d ' ')
    echo "$name text=$text rodata=$rodata data=$data bss=$bss data_image=$((rodata + data)) file=$file"
done

if [ "$found" -eq 0 ]; then
    echo "No rv32 ELFs found in $TARGET_DIR" >&2
    exit 1
fi
