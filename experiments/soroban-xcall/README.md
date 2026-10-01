# Soroban contracts as Groundhog contracts, with cross-contract calls

This follows `../soroban-native` (Soroban host and contracts compiled to RV32
with no Wasm engine). That design had one gap: without Wasm there is no trap
boundary. A native panic cannot be caught, so `try_call` could not recover
from a failing callee.

Here, **Groundhog's own call frames are the boundary**:

- Each Soroban contract is its own Groundhog contract.
- A Soroban contract's id **is** its Groundhog address.
- Every Soroban call to another contract becomes a Groundhog sub-invocation.

The same mechanism gives calls in every direction between Soroban and native
Groundhog contracts.

## Result

`run.sh` runs the following on `hog-devnode`. Gas is receipt `gasUsed` and
includes the dev wallet.

| Call | Result | Gas |
|---|---|---|
| Groundhog (CLI wallet) → Soroban `counter.increment(5)` | `U32(5)` | 725,476 |
| Groundhog contract `ghcaller.soroban_increment(counter, 2)` → Soroban | `7` | 813,824 |
| Soroban `caller.call_inc(counter, 3)` → Soroban | `U32(10)` | 1,360,549 |
| Soroban `try_call` of `counter.inc_then_panic(100)` (increments, then panics) | caught; counter still `10` | 1,736,054 |
| Soroban `try_call` of `counter.inc_then_err(100)` (increments, then `Err(7)`) | caught `Contract(7)`; counter still `10` | 1,921,477 |
| Soroban `caller.gh_call(ghcounter, increment, 5)` → native Groundhog counter | Groundhog counter `0 → 5` | 598,434 |
| Soroban → Groundhog `ghcaller` → Soroban `counter` | `counter` `10 → 11` | 1,375,920 |
| Soroban `caller` → `ghcaller` → back into `caller` (re-entry) | refused, reverted with `Context(InvalidAction)` | 372,524 |
| `counter.inc_then_panic(1)` called directly | reverted with `WasmVm(InvalidAction)`; no state change | 508,164 |
| `counter.inc_then_err(1)` called directly | reverted with `Contract(7)`; no state change | 694,160 |

- **Error codes match Wasm Soroban.** A plain contract panic surfaces as
  `Error(WasmVm, InvalidAction)`, which is what a trapped Wasm contract
  gives. Contract errors keep their code.
- **Rollback is Groundhog's.** A failed callee's storage writes are undone
  because its Groundhog frame reverts.

## How it works

**Identity.** The Soroban contract id is the hosting Groundhog contract's
32-byte address, so both worlds name a contract the same way.

- There is no create step. The storage shim synthesises the contract's
  instance entry, with executable = the compiled-in native code, until the
  contract first writes it.

**Method 0** is the Soroban ABI:

- **Input:** an XDR `HostFunction::InvokeContract` that must target this
  contract. The contract is not a proxy.
- **Success:** returns the XDR `ScVal` result.
- **Failure:** the Groundhog frame **reverts**, with an XDR `ScVal::Error`
  payload (`contract_abort`).
  - The payload is the Soroban error when the call returned one.
  - For a native panic, the guest's panic hook reverts with the host error
    that caused the panic. The vendored host records it in
    `escalate_error_to_panic`; a plain `panic!` maps to
    `WasmVm/InvalidAction`.
  - So a panic never needs unwinding: it ends this Groundhog frame, and the
    caller's frame carries on.

**Outbound calls.** The vendored host gets an `ExternalContractHandler` hook,
consulted at the top of `call_contract_fn` (after Soroban's re-entry and
reserved-name checks). The guest's `GroundhogRouter` claims every contract id
that isn't its own:

- **Soroban → Soroban:** a Groundhog `Invoke` of the callee's address,
  method 0, with an XDR `HostFunction`.
  - A returned `ScVal` becomes the call's value.
  - An `ScVal::Error` abort becomes that error.
  - A trap becomes `WasmVm/InvalidAction`.
  - The host treats any error like a failed callee, so `try_call` catches it.
- **Soroban → Groundhog:** the reserved function name
  `gh_raw(method: u64, calldata: Bytes) -> Bytes` makes a raw Groundhog call.
  `soroban-caller::gh_call` wraps it.
  - `method` is the Groundhog entrypoint id;
    `sorgen gh-method <Interface> <version> <method>` computes it with the
    SDK's own `contract_entrypoint_id`.
  - `calldata` is the CXS encoding of the arguments
    (`sorgen gh-u64`, `gh-addr-u32`).
- **Groundhog → Soroban:** call the Soroban contract's address, method 0, with
  the `HostFunction` XDR. `gh-caller/` does this from a stock-SDK Groundhog
  contract, hand-encoding the ~70 bytes of XDR, with no Soroban dependency.

**Re-entrancy** is refused by Groundhog (`kSyscallErrorCodeReentrance`). This
matters because a re-entered contract would read storage that its outer
invocation has not written back yet. The refusal reverts cleanly.

## Auth: Soroban auth trees as Groundhog auth tokens

Soroban contracts authorize with `addr.require_auth()`. The host must
confirm that `addr` approved this exact call, often as a sub-invocation of a
call the user signed (user → `router.pay` → `token.transfer(from = user)`).
Here the router and the token run in **different** Groundhog contracts, and
any contract along the way may be malicious. So authorization has to cross
untrusted hops without being forgeable, redirectable or reusable.
Groundhog's auth tokens already provide exactly that.

### Roles

**Trusted entrypoint (`gh-entry/`).** It verifies the Soroban signatures and
sequences nonces (RCQ). Both are assumed done in this experiment. It then
mints one Groundhog auth-token tree per Soroban auth entry, and invokes the
target Soroban contract with the root tokens.

Node layout (`sorgen entry` builds it from a JSON spec):

- `consumer_address`: the Soroban contract the node authorizes.
- `payload`: XDR `ScVal::Vec[authorizer, fn, args, child_fingerprints]`.
  - `pay` is 232 bytes and `transfer` is 188 bytes, under the current
    256-byte limit.
- `child_nodes`: the node's sub-invocations.
- A child fingerprint is `sha256(InvokeContractArgs XDR)[..8]`.
  - Only a token's consumer can open it, so a parent routes child tokens by
    fingerprint without seeing inside them.

**Each Soroban-hosting contract** (guest `auth` module):

- **Consumes** the tokens passed with its invocation and rejects any whose
  runtime-stamped minter isn't the trusted entrypoint.
- **Installs** the entries in the host as **pre-verified**. A new host API,
  `set_preverified_authorization_entries`, reuses Soroban's source-account
  path: no signature or nonce check, but full call-tree enforcement in
  enforcing mode.
- **Adds the direct Groundhog caller** as an authorizer of the root call.
  This is Soroban's invoker-contract rule, and `get_caller()` is
  runtime-provided.
- **Forwards child tokens.** On each outbound Soroban call, the router
  attaches the held child token whose fingerprint matches that exact call.
  If the callee fails, Groundhog hands the token back.

### Why an untrusted contract in the middle can't abuse this

| Attack | Stopped by |
|---|---|
| forge auth | only the entrypoint can mint tokens with itself as minter; the runtime stamps it |
| redirect a sub-authorization to another contract | only the named consumer can open a node |
| change the arguments (e.g. the recipient) | the payload pins the call; the callee's host enforces it against the real call |
| use one authorization twice | each token is consumed once |
| keep a token after a failed sub-call | the runtime returns tokens from failed frames |

### Results (hog-devnode, `run.sh`; U starts with 100)

| Scenario | Outcome | Gas |
|---|---|---|
| U authorizes `token.transfer(U, V, 10)` | ok | 1,204,397 |
| same call, no auth tree | refused `Auth(InvalidAction)` | 379,586 |
| tree authorizes recipient V, call pays T | refused `Auth(InvalidAction)` | 480,755 |
| valid tree, minted by an untrusted copy of the entrypoint | refused `Auth(InvalidInput)` | 68,857 |
| U → `router.pay` → `token.transfer` (child token forwarded across Groundhog contracts) | ok | 2,040,632 |
| same, tree lacks the `transfer` sub-invocation | refused | 925,126 |
| malicious `router.pay_evil` redirects the funds to itself | refused | 933,253 |
| `router.pay_twice` with one authorized transfer | refused (whole call reverts) | 2,239,970 |
| `router.pay_twice` with two authorized transfers | ok | 3,239,535 |
| `router.pay_self` spends the router's own balance (it is the direct caller) | ok | 1,811,989 |
| the entrypoint tries to spend the router's balance | refused | 379,586 |

Final balances reconcile: U 81, V 26, T 0, router 43.

### Auth limits and gaps

- **Token tree shape:** depth ≤ 4 and ≤ 4 children per node
  (`kAuthTokenMaxDepth`, `kAuthTokenMaxChildNodes`). Payloads ≤ 256 bytes.
- **Trusted-entrypoint registration:** set once per contract via method 2
  after deploy. A real deployment would fix it at build or genesis.
- **Signatures and nonces:** not implemented in `gh-entry` (assumed
  verified), and the RCQ sequencing isn't wired in.
- **Invoker auth one level down:** contract-authorized sub-calls one level
  further (`authorize_as_current_contract`) aren't mapped.
- **Raw Groundhog calls:** child tokens are only forwarded on Soroban →
  Soroban calls, not on `gh_raw` calls into native Groundhog contracts.

## Changes vs `../soroban-native`

**soroban-env-host patch (added):**

- `ExternalContractHandler` plus `Host::set_external_contract_handler`, with
  the hook in `call_contract_fn`.
- `take_last_escalated_error()`.
- `Host::set_preverified_authorization_entries`.

**Guest:**

- One Soroban contract per Groundhog contract, chosen by cargo feature
  (`counter`, `caller`).
- Identity, the instance synthesis described above, and the router.
- Errors revert the frame instead of trapping.

**New contracts:**

- `soroban-caller`: Soroban contract that calls other contracts.
- `gh-caller`: native Groundhog contract that calls Soroban.
- `soroban-token`, `soroban-router`: the auth scenarios.
- `gh-entry`: the trusted entrypoint.
- `soroban-counter`: gains `get`, `inc_then_panic` and `inc_then_err`.

**Tooling:**

- `tools/sorgen` builds calldata for any function with typed args, decodes
  results, and computes Groundhog entrypoint ids and calldata.
- `sorgen entry` builds entrypoint calldata with auth trees from
  `tests/*.json`.
- `sc.sh` calls a Soroban function directly; `ep.sh` calls one through an
  entrypoint.

## Costs and limits

- **Every Soroban contract carries its own host:** about 1.68 MB per
  Groundhog contract, against the 1.9 MB ceiling.
- **A Soroban → Soroban call runs two hosts:** about 1.36M gas versus about
  0.73M for a local call. The callee's host setup (~200K) and its storage
  first-touches are paid again per hop.
- **All of `../soroban-native`'s other trade-offs still apply.**

## Not done

- **Auth:** the remaining gaps are listed under "Auth limits and gaps"
  above.
- **Events:** Soroban contract events are dropped. They should be emitted as
  Groundhog events.
- **Typed Groundhog calls:** a Soroban-side client that encodes Groundhog
  calls from a contract spec, instead of raw `gh_raw` bytes.
- **Host size:** the host is duplicated in every Soroban contract. A shared
  "Soroban runtime" contract delegate-called by thin per-contract stubs would
  remove that, if Groundhog's delegate call fits.

## Reproduce

```sh
./build.sh   # both Soroban-hosting contracts, gh-caller, the SDK counter, sorgen
# start a node as in ../soroban-in-groundhog/README.md (needs the raised limits)
./run.sh     # deploys everything, runs the call matrix and the auth scenarios
```
