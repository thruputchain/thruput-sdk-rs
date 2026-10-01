#!/usr/bin/env bash
# Deploy out/sorhog-native.rv32a to a running local node, bind a Soroban
# contract to the compiled-in counter, and increment it twice. Each call goes
# through method 1 (invoke_profiled) and prints a gas breakdown.
# Start the node first (see README), with a `local` CLI profile.
set -euo pipefail
cd "$(dirname "$0")"
G=${GROUNDHOG:-groundhog}
OUT=out
"$G" deploy --profile local --artifact "$OUT/sorhog-native.rv32a" --alias sornative --force --json \
  | python3 -c "import json,sys;d=json.load(sys.stdin);print('deploy', d['ok'], d['data']['address'])"
for step in create_native inc5 inc5; do
  "$G" call --profile local sornative 0x0000000000000001 "$(cat "$OUT/cd_$step.hex")" \
    --gas-limit "${GAS_LIMIT:-5000000000}" --force --json > "$OUT/r_$step.json" || true
  echo "== $step"
  python3 tools/prof.py "$OUT/r_$step.json"
done
