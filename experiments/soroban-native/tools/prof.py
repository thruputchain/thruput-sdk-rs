#!/usr/bin/env python3
"""Decode the gas profile appended by method 1 (invoke_profiled)."""
import json, struct, sys

NAMES = {0: "entry", 1: "heap+calldata", 2: "decode HostFunction", 3: "Budget::default",
         4: "host setup (in hook)", 5: "register natives", 6: "invoke+ledger changes",
         7: "write-back", 8: "encode result"}
d = json.load(open(sys.argv[1]))
if not d.get("ok"):
    x = d.get("data") or {}
    print(f"FAILED gasUsed={x.get('gasUsed')} status={x.get('status')} {d.get('error', {}).get('message', '')}")
    sys.exit(1)
b = bytes.fromhex(d["data"]["returnData"][2:])
nreads = b[-1]
reads = [b[-1 - 3 * nreads + 3 * i: -1 - 3 * nreads + 3 * i + 3] for i in range(nreads)]
b = b[: -1 - 3 * nreads]
shim_gas, shim_calls, contract_gas = struct.unpack("<QQQ", b[-24:])
body = b[:-24]
# Find the mark table: a count byte n followed by n (id, gas) records whose
# ids run 0..n-1.
for n in range(31, 0, -1):
    pos = len(body) - 9 * n - 1
    if pos >= 0 and body[pos] == n and all(body[pos + 1 + 9 * i] == i for i in range(n)):
        break
marks = [(body[pos + 1 + 9 * i], struct.unpack("<Q", body[pos + 2 + 9 * i: pos + 10 + 9 * i])[0]) for i in range(n)]
total = d["data"]["gasUsed"]
print(f"total gasUsed {total:>10}   result ScVal XDR 0x{body[:pos].hex()}")
for (a, ga), (b_, gb) in zip(marks, marks[1:]):
    print(f"  {NAMES.get(b_, b_):28} {ga - gb:>10}")
print(f"  (before entry / after last mark: wallet, dispatch, return) {total - (marks[0][1] - marks[-1][1]):>10}")
print(f"storage shim: {shim_gas} gas over {shim_calls} reads; native contract code: {contract_gas} gas")
KINDS = {0: "Account", 1: "Trustline", 6: "ContractData", 7: "ContractCode", 9: "Ttl"}
for t, found, inst in reads:
    print(f"  read {KINDS.get(t, t)}{' (instance)' if inst else ''}: {'hit' if found else 'MISS'}")
