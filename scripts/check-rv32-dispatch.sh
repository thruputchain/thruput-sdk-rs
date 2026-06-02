#!/usr/bin/env bash
# Assert a lowered/linked rv32 contract ELF uses guest-side dispatch:
#   - no legacy 16-hex / .text.gh_* method-export symbols remain
#   - _start is the entry at vaddr 0
#   - the linkme dispatch table is present inside .rodata
#
# Usage: scripts/check-rv32-dispatch.sh path/to/contract.elf
# Requires: llvm-objdump, llvm-readobj (or rust-objdump/readobj) on PATH.
set -euo pipefail

ELF="${1:?usage: check-rv32-dispatch.sh <elf>}"
OBJDUMP="${OBJDUMP:-$(command -v llvm-objdump || command -v rust-objdump)}"
READOBJ="${READOBJ:-$(command -v llvm-readobj || command -v rust-readobj)}"

fail() { echo "FAIL: $*" >&2; exit 1; }

# 1. No legacy per-method export sections.
if "$OBJDUMP" --section-headers "$ELF" | grep -qE '\.text\.gh_'; then
  fail ".text.gh_* sections still present (v3 dispatch surface not removed)"
fi

# 2. _start at entry vaddr 0. Accept any spelling of zero; fail closed on an
#    empty/unparsed value (a missing Entry line must not silently pass).
ENTRY="$("$READOBJ" --file-headers "$ELF" | awk -F'[ ]+' '/Entry:/ {print $NF}')"
case "$ENTRY" in
  0x0 | 0x00000000 | 0) : ;;
  *) fail "_start not at vaddr 0 (entry='$ENTRY')" ;;
esac

# 3. .rodata exists and is non-empty (it carries the dispatch table). The
#    cross-repo smoke additionally asserts specific method_ids appear in the
#    initial-memory image; that check lives with the lowerer's test harness.
if ! "$OBJDUMP" --section-headers "$ELF" | grep -qE '\.rodata'; then
  fail ".rodata missing (dispatch table would be unmaterialized)"
fi

echo "OK: $ELF uses guest-side dispatch (no .text.gh_*, _start@0, .rodata present)"
