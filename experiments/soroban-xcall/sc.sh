#!/usr/bin/env bash
# sc.sh <alias> <fn> [typed args...]: call a soroban-xcall contract's Soroban
# function through Groundhog method 0 and decode the result.
# Needs out/addr_<alias> (written by run.sh) and GROUNDHOG.
set -euo pipefail
cd "$(dirname "$0")"
G=${GROUNDHOG:-groundhog}
name=$1; fn=$2; shift 2
addr=$(cat "out/addr_$name")
args=()
for a in "$@"; do  # addr:@name expands to that contract's address
  case $a in addr:@*) args+=("addr:$(cat "out/addr_${a#addr:@}")");; *) args+=("$a");; esac
done
cd_hex=$(tools/sorgen/target/release/sorgen call "$addr" "$fn" "${args[@]}")
"$G" call --profile local "$addr" 0x0000000000000000 "$cd_hex" --gas-limit "${GAS_LIMIT:-5000000000}" --force --json \
  | python3 -c "
import json, subprocess, sys
d = json.load(sys.stdin); x = d.get('data') or {}
r = x.get('returnData')
v = subprocess.run(['tools/sorgen/target/release/sorgen', 'decode', r], capture_output=True, text=True).stdout.strip() if r and d.get('ok') else ''
print(f\"{'$name'}.{'$fn'}: ok={d.get('ok')} gas={x.get('gasUsed')} -> {v or x.get('status')}\")
sys.exit(0 if d.get('ok') else 1)
"
