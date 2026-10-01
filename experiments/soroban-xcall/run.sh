#!/usr/bin/env bash
# Deploy the contracts to a running local node, exercise every call direction,
# then Soroban auth through the trusted entrypoint. Start the node first (see
# README), with a `local` CLI profile.
set -euo pipefail
cd "$(dirname "$0")"
export GROUNDHOG=${GROUNDHOG:-groundhog}
G=$GROUNDHOG
S=tools/sorgen/target/release/sorgen
deploy() { # deploy <alias> <artifact>
  "$G" deploy --profile local --artifact "out/$2.rv32a" --alias "$1" --force --json \
    | python3 -c "import json,sys; print(json.load(sys.stdin)['data']['address'])" > "out/addr_$1"
  echo "deployed $1 $(cat "out/addr_$1")"
}
for c in counter caller token router ghcounter ghcaller entry; do deploy "$c" "$c"; done
deploy forger entry # a second, untrusted copy of the entrypoint
for c in token router; do # trust the first entrypoint's tokens
  "$G" call --profile local "$(cat "out/addr_$c")" 0x0000000000000002 "$(cat out/addr_entry)" --force --json >/dev/null
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

echo "== Auth: Soroban auth trees as Groundhog auth tokens minted by the trusted entrypoint"
ep() { ./ep.sh "$@" || true; }
ep entry tests/mint_u.json
ep entry tests/transfer.json             # U authorizes transfer(U, V, 10)
ep entry tests/transfer_noauth.json      # no auth tree: refused
ep entry tests/transfer_wrong_args.json  # tree authorizes a different recipient: refused
ep forger tests/transfer.json            # same tree minted by an untrusted contract: refused
ep entry tests/router_pay.json           # U -> router.pay -> token.transfer (child token forwarded)
ep entry tests/router_pay_no_child.json  # tree lacks the transfer sub-invocation: refused
ep entry tests/router_pay_evil.json      # router redirects funds to itself: refused
ep entry tests/router_pay_twice_one_auth.json # one authorization, two transfers: refused
ep entry tests/router_pay_twice.json     # two authorized transfers: ok
ep entry tests/mint_router.json
ep entry tests/router_pay_self.json      # router spends its own balance (direct caller): ok
ep entry tests/transfer_from_router.json # entrypoint spends router's balance: refused
for w in U V T router; do ep entry "tests/balance_$w.json"; done
