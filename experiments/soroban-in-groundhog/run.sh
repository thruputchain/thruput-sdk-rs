#!/usr/bin/env bash
# Deploy out/sorhog.rv32a to a running local node and drive the Soroban
# lifecycle through it: upload Wasm, create contract, increment(5) twice.
# Start the node first (see README), with a `local` CLI profile.
# GROUNDHOG: CLI binary (default: `groundhog` on PATH).
set -euo pipefail
cd "$(dirname "$0")"
G=${GROUNDHOG:-groundhog}
OUT=out
"$G" deploy --profile local --artifact "$OUT/sorhog.rv32a" --alias sorhog --force --json \
  | python3 -c "import json,sys;d=json.load(sys.stdin);print('deploy', d['ok'], d['data']['address'])"
for step in upload create inc5 inc5; do
  "$G" call --profile local sorhog 0x0000000000000000 "$(cat "$OUT/cd_$step.hex")" \
    --gas-limit "${GAS_LIMIT:-5000000000}" --force --json > "$OUT/r_$step.json" || true
  python3 - "$OUT/r_$step.json" "$step" <<'PY'
import json, sys
d = json.load(open(sys.argv[1])); x = d.get("data") or {}
print(f"{sys.argv[2]:7} ok={d.get('ok')} gasUsed={x.get('gasUsed')} return={x.get('returnData')}")
PY
done
