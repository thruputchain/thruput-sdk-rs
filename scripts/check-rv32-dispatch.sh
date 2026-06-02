#!/usr/bin/env bash
# Assert a lowered/linked rv32 contract ELF uses guest-side dispatch:
#   - no legacy 16-hex / .text.gh_* method-export symbols remain
#   - _start is the entry at vaddr 0
#   - the linkme dispatch table is present AND non-empty
#
# Usage: scripts/check-rv32-dispatch.sh path/to/contract.elf
# Requires: llvm-objdump, llvm-readobj, llvm-nm (or rust-* / plain nm) on PATH.
set -euo pipefail

ELF="${1:?usage: check-rv32-dispatch.sh <elf>}"
OBJDUMP="${OBJDUMP:-$(command -v llvm-objdump || command -v rust-objdump)}"
READOBJ="${READOBJ:-$(command -v llvm-readobj || command -v rust-readobj)}"
NM="${NM:-$(command -v llvm-nm || command -v rust-nm || command -v nm)}"

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

# 3. Dispatch table present AND non-empty. A bare ".rodata exists" check is too
#    weak: string literals make .rodata non-empty even if the linkme sections
#    were silently dropped (e.g. --gc-sections, a misconfigured KEEP, or a
#    future linkme that renames its sections), which would route every call to
#    the ebreak backstop with no link error.
#
#    Preferred check: assert __stop_linkme_… > __start_linkme_… via the symbol
#    table. These encapsulation symbols are resolved to link-time constants, so
#    a *stripped* release artifact (the v4 norm) no longer carries them — there
#    we fall back to a .rodata presence check, and the authoritative per-method
#    population check lives in the cross-repo smoke (it knows the contract's
#    method_ids and verifies they appear in the initial-memory image).
SYMS="$("$NM" "$ELF" 2>/dev/null || true)"
START_ADDR="$(printf '%s\n' "$SYMS" | awk '$3=="__start_linkme_GROUNDHOG_RV32_ENTRYPOINTS"{print $1}')"
STOP_ADDR="$(printf '%s\n' "$SYMS" | awk '$3=="__stop_linkme_GROUNDHOG_RV32_ENTRYPOINTS"{print $1}')"

if [ -n "$START_ADDR" ] && [ -n "$STOP_ADDR" ]; then
  if [ "$((16#$STOP_ADDR))" -le "$((16#$START_ADDR))" ]; then
    fail "dispatch table is empty (__stop=0x$STOP_ADDR <= __start=0x$START_ADDR) — linkme sections dropped or no methods registered"
  fi
  echo "OK: $ELF uses guest-side dispatch (no .text.gh_*, _start@0, dispatch table non-empty)"
elif "$OBJDUMP" --section-headers "$ELF" | grep -qE '\.rodata'; then
  echo "OK: $ELF uses guest-side dispatch (no .text.gh_*, _start@0, stripped; .rodata present — table population verified by the smoke's method_id check)"
else
  fail ".rodata missing (dispatch table would be unmaterialized)"
fi
