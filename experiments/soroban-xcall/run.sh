#!/usr/bin/env bash
# Deploy the four contracts to a running local node and exercise every call
# direction. Start the node first (see README), with a `local` CLI profile.
set -euo pipefail
cd "$(dirname "$0")"
export GROUNDHOG=${GROUNDHOG:-groundhog}
G=$GROUNDHOG
S=tools/sorgen/target/release/sorgen
for c in counter caller ghcounter ghcaller; do
  "$G" deploy --profile local --artifact "out/$c.rv32a" --alias "$c" --force --json \
    | python3 -c "import json,sys; print(json.load(sys.stdin)['data']['address'])" > "out/addr_$c"
  echo "deployed $c $(cat "out/addr_$c")"
done
gh() { # gh <alias> <method> <args...>: native Groundhog call by method name
  local a=$1 m=$2; shift 2
  (cd "$([ "$a" = ghcaller ] && echo gh-caller || echo ../../example-contracts/counter)" &&
    "$G" call --profile local "$(cat "$OLDPWD/out/addr_$a")" "$m" "$@" --gas-limit 5000000000 --force --json) |
    python3 -c "import json,sys; d=json.load(sys.stdin); x=d.get('data') or {}; print(f\"$a.$m: ok={d['ok']} gas={x.get('gasUsed')} -> {x.get('returnData') or x.get('status')}\")"
}

echo "== Groundhog -> Soroban (CLI wallet, then a Groundhog contract)"
./sc.sh counter increment u32:5
gh ghcaller soroban_increment "$(cat out/addr_counter)" 2
./sc.sh counter get

echo "== Soroban -> Soroban"
./sc.sh caller call_inc addr:@counter u32:3

echo "== Soroban try_call of a failing callee: callee's writes roll back"
./sc.sh caller try_panic addr:@counter u32:100
./sc.sh caller try_err addr:@counter u32:100
./sc.sh counter get

echo "== Soroban -> Groundhog"
gh ghcounter initialize 0
./sc.sh caller gh_call addr:@ghcounter "u64:$($S gh-method Counter 0.4.0 increment)" "bytes:$($S gh-u64 5)"
gh ghcounter get

echo "== Soroban -> Groundhog -> Soroban"
M=$($S gh-method GhCaller 0.1.0 soroban_increment)
./sc.sh caller gh_call addr:@ghcaller "u64:$M" "bytes:$($S gh-addr-u32 "$(cat out/addr_counter)" 1)"
./sc.sh counter get

echo "== Re-entry (Soroban caller -> ghcaller -> Soroban caller) is refused"
./sc.sh caller gh_call addr:@ghcaller "u64:$M" "bytes:$($S gh-addr-u32 "$(cat out/addr_caller)" 1)" || true

echo "== Failing calls revert with the Soroban error"
./sc.sh counter inc_then_panic u32:1 || true
./sc.sh counter inc_then_err u32:1 || true
./sc.sh counter get
