#!/usr/bin/env bash
# ep.sh <entry-alias> <spec.json>: send a Soroban call + auth trees through an
# entrypoint contract (see tools/sorgen `entry`) and decode the outcome.
set -euo pipefail
cd "$(dirname "$0")"
G=${GROUNDHOG:-groundhog}
S=tools/sorgen/target/release/sorgen
cd_hex=$($S entry "$2" 2>/dev/null)
"$G" call --profile local "$(cat "out/addr_$1")" 0x0000000000000000 "$cd_hex" \
  --gas-limit "${GAS_LIMIT:-5000000000}" --force --json | python3 -c "
import json, subprocess, sys
d = json.load(sys.stdin); x = d.get('data') or {}
dec = lambda h: subprocess.run(['$S', 'decode', h], capture_output=True, text=True).stdout.strip() if h else ''
out = dec(x.get('returnData')) if d.get('ok') else (dec(x.get('abortData')) or x.get('status'))
print(f\"  {'$2'.split('/')[-1]:24} via {'$1':6} ok={d.get('ok')!s:5} gas={x.get('gasUsed')} -> {out}\")
sys.exit(0 if d.get('ok') else 1)
"
