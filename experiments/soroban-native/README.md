# Soroban contracts compiled natively to RV32 (no Wasm engine)

This is a follow-up to `../soroban-in-groundhog`. That experiment ran Soroban
contracts as Wasm inside wasmi, inside the Groundhog VM. Here **the Soroban
host and the Soroban contract are both compiled straight to RISC-V** into a
single Groundhog contract. No Wasm is parsed or interpreted at runtime.

**Result:** it works on a real `hog-devnode`. `increment` costs **0.82M gas
instead of 3.79M (4.6× cheaper)**, and the artifact is **1.68 MB instead of
1.73 MB**.

## Result

The Soroban contract is the same `soroban-sdk` 28 counter
(`soroban-counter/`), built as an rlib. Its contract ID and its return values
(5, 10, 15) match the Wasm run in `../soroban-in-groundhog`, which itself
matched native Soroban.

Gas per Soroban call, measured on hog-devnode (receipt `gasUsed`, includes
the dev wallet):

| Step | Wasm in wasmi (prev.) | Native RV32 | + 1 key/entry | + memo shim, code in binary | + Soroban metering off |
|---|---|---|---|---|---|
| create contract | 5,546,587 | 891,881 | 894,593 | 875,306 | **639,670** |
| `increment(5)` (1st) | 3,567,841 | 1,154,229 | 1,079,448 | 983,013 | **744,697** |
| `increment(5)` (2nd) | 3,785,547 | 1,367,332 | 1,253,243 | 1,117,281 | **828,323** |
| upload Wasm | 3,035,904 | n/a, compiled in | | | |

The native Groundhog counter `increment` is about 40K gas, so this is still
about 20× more.

Artifact sizes (default limits: 256 KiB code, 512 KiB contract; ceiling
1.9 MB):

| | `.text` | artifact |
|---|---|---|
| Wasm in wasmi (prev.) | 1,544,072 | 1,725,832 |
| native, wasmi still linked | 1,593,048 | 1,780,112 |
| native, `no-wasm-vm` | 1,543,056 | 1,728,057 |
| native, `no-wasm-vm` + `no-soroban-metering` (this build) | 1,494,032 | 1,679,145 |

All runs use 2 MiB of data memory, the minimum found for the Wasm variant. The native build was not re-probed for a lower minimum.

### Where the 0.83M gas goes now (2nd `increment`, `tools/prof.py`)

| Phase | Gas |
|---|---|
| `Budget::default()` | 70,886 |
| host construction + ledger info | 87,008 |
| register native contracts | 39,588 |
| invoke + ledger-change computation | 587,874 |
| ↳ storage shim (3 reads) | 159,354 |
| ↳ native contract code, including the host calls it makes | 182,427 |
| write-back of changes | 19,745 |
| calldata decode, result encode, wallet | ~23,000 |

The two ledger reads that reach storage each pay hogd's first-touch DB cost
(`kDBRPCCost` = 37,300 gas), so ~75K of the shim cost is inherent to touching
storage.

## How it works

### Native function table

On non-wasm targets `soroban_sdk::Env` wraps `soroban_env_host::Host`.
`#[contractimpl]` emits a native entrypoint `__<Type>__<fn>__invoke_raw` for
each method. The guest builds a host `ContractFunctionSet` over those
entrypoints with a small macro:

```rust
native_contract!(CounterFns, soroban_counter::Counter {
    increment => __Counter__increment__invoke_raw(0),
});
```

### Binding contracts to native code

Before each invocation, the function set is registered with
`Host::register_native_contract_as_wasm` under a pseudo Wasm hash,
`sha256("sorhog-native:" + label)`. The normal Soroban lifecycle is unchanged:

- `CreateContract` with `ContractExecutable::Wasm(<pseudo hash>)` binds a
  contract instance to the native code (`sorgen create-native soroban-counter`).
- `InvokeContract` calls it.

### Storage shim

The shim is the same `SnapshotSource` design as before, with three changes:

- **One key per small entry:** the entry header is folded into chunk 0, so
  entries up to 1,016 bytes use exactly one Groundhog key (one 37.3K
  first-touch instead of two).
- **Memoised per transaction:** Soroban asked for the same 3 keys 8 times per
  call (during the call, then again when assembling the footprint).
- **Native code entries come from the binary:** `ContractCode` entries for
  native pseudo hashes are synthesised, never read or written.

### Profiling

Method 1 (`invoke_profiled`) is method 0 plus gas checkpoints from
`sdk::stats::gas_remaining()`. They are appended to the return data and
decoded by `tools/prof.py`.

## Patches (vendored, see `patches/`)

**soroban-env-host 28.0.2:**

- **VM timer:** the wall-clock `VmInstantiationTimer` is compiled out on
  `target_os = "zkvm"`. This is unchanged from the first experiment.
- **Setup hook:** `invoke_host_function_in_recording_mode_with_setup` runs a
  setup closure on the fresh `Host` before invoking, which is where native
  contracts get registered. The existing function delegates to it.
- **`no-wasm-vm` feature:** Wasm upload and Wasm execution return
  `Error(WasmVm, InvalidAction)`. Only native contracts can run.
  - This drops wasmi execution, but **wasmparser (~220 KB) is still linked**
    through code-entry cost and rent paths. Cutting that needs more surgery.
- **`no-soroban-metering` feature:** `Budget::charge`/`bulk_charge` return
  immediately, since Groundhog already meters every RV32 instruction.
  - This removed ~290K gas per call and ~50 KB of code.
  - Soroban's CPU/memory limits are therefore not enforced. Groundhog gas and
    the fixed VM memory are the only bounds.

**soroban-sdk 28.0.0:**

- **`Env::__from_host`:** a hidden constructor so an embedder can call
  `invoke_raw` functions without `testutils`. Full `testutils` pulls in
  `ctor` (load-time constructors the RV32 runtime never runs), `serde_json`,
  ed25519 and other unneeded dependencies.
- **`native-embed` feature:** enables `soroban-env-host/testutils`, which is
  needed for `ContractFunctionSet` and native dispatch and only adds
  `arbitrary`. It also routes host errors through
  `escalate_error_to_panic`, as the sdk does under `testutils`.

## Trade-offs vs. Wasm

- **Contracts are fixed at build time:** a contract must be compiled into
  this Groundhog contract.
  - There is no `UploadContractWasm`. New or upgraded Soroban code means
    deploying (or upgrading) the Groundhog contract.
  - Each Groundhog contract carries the host (~1.5 MB) plus the contracts in
    it, against a 1.9 MB ceiling. Room for contract code is therefore about
    200 KB per Groundhog contract.
- **No Wasm sandbox:** native contracts run in the same address space as the
  host. Isolation is the Groundhog VM's, around the host *and* the contracts
  together. That is fine for trusted, curated contracts, but it is not a
  substitute for the Wasm boundary with untrusted code.
- **Panics end the transaction:** native panics are not caught, because
  `panic = "abort"` is used and there is no unwinding. A contract panic or
  host error aborts the whole Groundhog transaction, so Soroban `try_call`
  cannot recover from a callee failure.
- **Different cost semantics from Stellar:** costs follow Groundhog gas over
  native code, not Soroban's Wasm cost model.

All gaps listed in `../soroban-in-groundhog/README.md` still apply: auth
mapping, rent and archival, no classic state or SAC, no Soroban↔Groundhog
calls.

## Next levers (not done)

- **Per-call fixed cost (~200K):** `Budget::default`, host construction and
  native registration. A cheaper budget constructor helps when metering is
  off. Registration could avoid storage work entirely.
- **XDR in the shim:** entries are decoded from XDR on every first read.
- **wasmparser:** cutting it from `no-wasm-vm` builds would save ~220 KB.
- **Unused host pieces:** feature-gating BLS12-381/BN254 and other host
  functions a deployment does not need would save size.

## Reproduce

```sh
./build.sh          # vendors + patches soroban crates, builds and lowers the guest
# start a node as in ../soroban-in-groundhog/README.md, then:
./run.sh            # deploy, create-native, increment x2, with gas profiles
```
