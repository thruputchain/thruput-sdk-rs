# Soroban inside a Groundhog contract (feasibility experiment)

**Question:** can a full Soroban environment (`soroban-env-host`, the same
host stellar-core runs, including the wasmi VM) run inside one Groundhog RV32
contract, with Soroban's ledger storage shimmed onto Groundhog storage?

**Answer: yes, it runs.** It works on a real `hog-devnode` built from hogd,
with no changes to hogd. Measured costs:

- **Size:** 1.73 MB, about 3.4x the 512 KiB default contract size.
- **Memory:** 2 MiB, about 32x the 64 KiB default.
- **Gas:** about 90x a native Groundhog contract for the same counter logic.

## Result

The flow was: deploy `sorhog.rv32a`, then send it Soroban `HostFunction`s.
The Soroban contract is a stock `soroban-sdk` 28 counter, compiled to Wasm.

| Step (Soroban `HostFunction`) | Groundhog gas | Result (matches native Soroban) |
|---|---|---|
| `UploadContractWasm` (3.8 KB Wasm) | 3,035,904 | Wasm hash `d41360bf…` |
| `CreateContractV2` | 5,546,587 | contract `ac396965…` |
| `InvokeContract increment(5)` | 3,567,841 | `U32(5)` |
| `InvokeContract increment(5)` | 3,785,547 | `U32(10)` |
| native Groundhog `counter.increment(5)` (baseline) | 39,652 | |

- **Gas basis:** all gas figures are the receipt `gasUsed` and include the dev
  wallet overhead.
- **Native check:** `tools/native-check` runs the identical calldata through
  `soroban-env-host` natively and gets identical hashes, ids and return values.

### Artifact and limits

| | Needed | Default | Hard ceiling |
|---|---|---|---|
| `.text` (`max_rv32_code_bytes`) | 1,544,072 B | 256 KiB | 4 MiB |
| artifact (`max_contract_size_bytes`) | 1,725,832 B | 512 KiB | **1,900,000 B** |
| data memory (`max_rv32_memory_bytes`) | 2 MiB (1 MiB faults) | 64 KiB | 16 MiB |

- **No hogd changes:** all three limits are network-config parameters, so the
  experiment needs none.
- **Little headroom:** the contract-size ceiling (`kMaxBytesTransaction`)
  leaves about 170 KB to spare. Adding features or using a bigger Soroban
  release could push the artifact past what a single transaction can carry.
- **Deploy works with the stock CLI:** `groundhog deploy --artifact` handles
  the 1.73 MB artifact as-is.

## How it works

`guest/` is a raw-entrypoint Groundhog contract.

- **Method 0** takes an XDR `HostFunction` as calldata. It runs
  `e2e_invoke::invoke_host_function_in_recording_mode` and returns the
  XDR `ScVal`.
- **Storage shim:** recording mode discovers the footprint on the fly.
  `HogSnapshot` is a `SnapshotSource` that answers every Soroban ledger read
  from Groundhog bytestring storage.
- **Write-back:** after the call, every non-read-only `LedgerEntryChange` is
  written back to Groundhog storage, or deleted.
- **Chunking:** ledger entries are larger than one bytestring (1 KiB), so each
  entry is split into chunks:
  - a header at `sha256("sorhog" || LedgerKey XDR)` holds `len` and `live_until`;
  - chunk *i* is at `sha256(header || i)`.

### Getting `std` + Soroban onto the Groundhog VM

`soroban-env-host` needs `std`, and there is no `std` for
`riscv32im-unknown-none-elf`.

- **Target:** the guest is built for Rust's `riscv32im-risc0-zkvm-elf`
  instead, with `-Zbuild-std`. That target is RV32IM, single-threaded, and
  has a minimal `std` platform layer.
- **Same constraints as Groundhog's target:** atomics lower to libcalls, there
  are no A/F/C extensions, and codegen is static.
- **Lowering:** the linked ELF lowers with Groundhog's own `rv32-lower`
  unchanged. `.text` is pure RV32IM with no relocations and only allowlisted
  sections.
- **Link flags:** the canonical `example-contracts` link flags apply.

Glue the guest supplies:

- **`__atomic_*` libcalls:** plain loads and stores (`src/atomics.rs`). They
  are exact because the VM is single-threaded.
- **zkvm platform hooks:** `sys_write`/`sys_log` → `ConsoleLog`,
  `sys_panic` → log + `unimp`, and `sys_rand` → fixed bytes.
- **Allocator:** a `talc` free-list allocator over the SDK heap span. std's
  zkvm `System` allocator never frees.
- **`getrandom`:** a custom backend that errors. Soroban seeds its own PRNG.
- **Linker script:** derived from the SDK script with CODE/DATA widened to the
  VM ceilings (`build.rs`). The host does not fit the SDK's 1 MiB CODE region.

Upstream changes this needed:

- **sdk_rs (this branch):** `groundhog-rv32-runtime` installs its
  `#[panic_handler]` and bump allocator only on `target_os = "none"`, so a
  std guest can link the SDK.
  - For normal contracts nothing changes: `counter.rv32a` is byte-identical
    with and without the change, and `groundhog-rv32-runtime` unit tests pass.
- **soroban-env-host, patch only:** the wall-clock `VmInstantiationTimer`
  (telemetry only) is compiled out on `target_os = "zkvm"`, as upstream
  already does for wasm. See
  `patches/soroban-env-host-28.0.2-vm-timer.patch`.
  - Without it, the guest panics with "time not implemented on this platform".

### Pitfalls hit along the way

- **`std` HashMap randomness:** `RandomState` asks the platform for random
  keys. A trapping `sys_rand` faulted the upload, so it now returns fixed
  bytes. Real randomness would be a consensus hazard, and Soroban does not
  depend on map iteration order.
- **Cascading errors:** once the upload fails, every later step fails with
  `Error(Storage, MissingValue)`, because the Wasm was never stored.

## What this does *not* do (gaps before this is more than a demo)

- **Auth:** every call runs as one fixed Soroban source account with
  *recording* auth. Making `require_auth` mean something needs real work:
  - map Groundhog callers/signers onto Soroban `Address`es;
  - run *enforcing* auth;
  - verify `SorobanAuthorizationEntry` signatures. That is ed25519 in
    software inside the guest, unless routed to Groundhog's
    `VerifySignature` syscall.
- **Two meters:** Soroban's own CPU/memory `Budget` (`Budget::default()`)
  runs on top of Groundhog gas. Its limits are not tied to the Groundhog gas
  limit.
- **No module cache:** every transaction starts from fresh VM memory, so the
  Wasm is re-parsed, re-validated and re-instantiated on every call.
  - This probably explains much of the ~3.5M gas per call, but it was not
    profiled.
  - A persistent cache would need a serialised compiled-module format, which
    wasmi 0.31 does not have.
- **Ledger semantics:**
  - Ledger sequence is `1000 + block height` and timestamp is 0.
  - TTLs are stored, but no rent is charged and nothing is archived.
  - The network id is a constant.
- **No Stellar classic state:** there are no accounts, trustlines or
  Stellar Asset Contract backing. The native-token and SAC paths will not
  work.
- **No Soroban↔Groundhog interop:** a Soroban contract cannot call a
  Groundhog contract, or the reverse.
- **One contract holds all Soroban state:** every Soroban contract's state
  lives under this single Groundhog contract's storage.

## Reproduce

Prerequisites:

- the groundhog registry configured (see the sdk_rs README);
- a `wasm32v1-none` stable target;
- a `groundhog-rpc` checkout next to `sdk_rs`, for `rv32-lower`;
- a `hog-devnode` built from hogd (`make devnode`).

```sh
./build.sh                       # -> out/sorhog.rv32a, out/counter.wasm, out/cd_*.hex

groundhog node --hog-devnode-binary /path/to/hog-devnode \
  --network-config max_rv32_code_bytes=4M \
  --network-config max_rv32_memory_bytes=16M \
  --network-config max_contract_size_bytes=1900000 \
  --dev-account-hog 1000000      # in another terminal

groundhog config new local --network local --rpc-url http://127.0.0.1:9545 \
  --default-key dev-1 --use --global
./run.sh                         # deploy, upload, create, increment x2

(cd tools/native-check && cargo run --release -- ../../out/cd_*.hex)  # reference
```

`MEMORY_BYTES`/`STACK_BYTES` override the guest layout (defaults: 2 MiB and
256 KiB).

Build notes from this run:

- **Protocol token:** the CLI and hog-devnode must agree on the protocol
  token. With a groundhog-rpc HEAD CLI against hogd HEAD, pass
  `--hogd-protocol hogp/<hogd's protocol submodule commit>`.
- **LLVM tools:** a container with clang 21 and an older default `llvm-ar`
  needs `PATH=/usr/lib/llvm-21/bin:$PATH` when building hogd.
