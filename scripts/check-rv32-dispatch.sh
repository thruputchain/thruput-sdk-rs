#!/usr/bin/env bash
# Assert a linked rv32 contract ELF uses guest-side dispatch:
#   - _start is the entry at vaddr 0
#   - the dispatch table is present AND non-empty
#
# Usage: scripts/check-rv32-dispatch.sh path/to/contract.elf
# Requires: readelf (GNU or llvm) and nm (llvm/rust/GNU) on PATH; READELF/NM
# env vars override. All ELF-header queries use readelf, which the Ubuntu CI
# runner ships (GNU binutils readelf parses rv32 ELFs); llvm-objdump does not
# have to be installed.
set -euo pipefail

ELF="${1:?usage: check-rv32-dispatch.sh <elf>}"
READELF="${READELF:-$(command -v llvm-readelf || command -v readelf || true)}"
NM="${NM:-$(command -v llvm-nm || command -v rust-nm || command -v nm)}"

fail() { echo "FAIL: $*" >&2; exit 1; }

[ -n "$READELF" ] || fail "no llvm-readelf or readelf on PATH (or set READELF)"

# 1. _start at entry vaddr 0. Accept any spelling of zero; fail closed on an
#    empty/unparsed value (a missing Entry line must not silently pass).
# GNU readelf and llvm-readelf (GNU output style, its default) both print
# "Entry point address:               0x0".
ENTRY="$("$READELF" -h "$ELF" | awk '/Entry point address:/ {print $NF}')"
case "$ENTRY" in
  0x0 | 0x00000000 | 0) : ;;
  *) fail "_start not at vaddr 0 (entry='$ENTRY')" ;;
esac

# 2. Dispatch table present AND non-empty. A bare ".rodata exists" check is too
#    weak: string literals make .rodata non-empty even if the dispatch section
#    were silently dropped (e.g. --gc-sections, a misconfigured KEEP, or a
#    macro/linker-script section-name mismatch), which would route every call
#    to the ebreak backstop with no link error.
#
#    Preferred check: assert __thruput_rv32_entrypoints_end > __thruput_rv32_entrypoints_start via the
#    symbol table. These bracket symbols are resolved to link-time constants,
#    so a *stripped* release ELF no longer carries them — there we fall back to
#    a .rodata presence check, and the authoritative per-method population
#    check lives in the cross-repo smoke (it knows the contract's method_ids
#    and verifies they appear in the initial-memory image).
SYMS="$("$NM" "$ELF" 2>/dev/null || true)"
START_ADDR="$(printf '%s\n' "$SYMS" | awk '$3=="__thruput_rv32_entrypoints_start"{print $1}')"
STOP_ADDR="$(printf '%s\n' "$SYMS" | awk '$3=="__thruput_rv32_entrypoints_end"{print $1}')"

if [ -n "$START_ADDR" ] && [ -n "$STOP_ADDR" ]; then
  if [ "$((16#$STOP_ADDR))" -le "$((16#$START_ADDR))" ]; then
    fail "dispatch table is empty (end=0x$STOP_ADDR <= start=0x$START_ADDR) — section dropped or no methods registered"
  fi
  echo "OK: $ELF uses guest-side dispatch (_start@0, dispatch table non-empty)"
elif "$READELF" -SW "$ELF" | grep -E '\.rodata' > /dev/null; then
  echo "WARN: $ELF passed only the weak stripped-ELF dispatch check (_start@0, .rodata present); the smoke must verify method_id dispatch"
else
  fail ".rodata missing (dispatch table would be unmaterialized)"
fi
