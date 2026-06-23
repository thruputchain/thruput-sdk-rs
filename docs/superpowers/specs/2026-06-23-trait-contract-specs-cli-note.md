# CLI note: building interface specs on the rv32 path

After the sdk_rs changes on `geoff/trait-contract-specs`, building a contract
spec from an interface trait is fully rv32-native and wasm-free. Verified
end-to-end with `groundhog-erc20-if`.

## The flow

```sh
# 1. Build the interface crate as a zero-entrypoint rv32 spec-carrier ELF.
cargo build -p groundhog-erc20-if \
  --bin groundhog-erc20-if \
  --no-default-features --features entrypoints \
  --target riscv32im-unknown-none-elf --release \
  -Zbuild-std=core,alloc

# 2. Extract + canonicalize the spec straight from the ELF.
groundhog-spec-tool spec \
  --elf target/riscv32im-unknown-none-elf/release/groundhog-erc20-if \
  --json-out erc20.json
```

## What changed in sdk_rs

- The `#[contract]` macro now emits the `groundhogspecrawv0` fragments for a
  **trait** (interface) definition, not just impls/free-fns.
- Each standard-interface crate gets an `entrypoints = []` feature and a thin
  `[[bin]]` whose `src/main.rs` is `#![no_std] #![no_main] extern crate <lib>;`
  (single `extern crate` — the runtime is force-linked transitively via `sdk`;
  do NOT also `extern crate groundhog_rv32_runtime`, it is not a direct dep).
  `crate-type` stays `["rlib"]` — no cdylib.
- `groundhog-spec-tool` gained `spec --elf` and the library function
  `groundhog_spec_tool::elf::elf_to_spec(&[u8]) -> Result<Option<ContractSpec>>`,
  which reads the `groundhogspecrawv0` section out of the ELF (via the `object`
  crate) and feeds the existing `parse_raw_fragments` → `canonicalize_spec` →
  `spec_to_json` pipeline.
- The workspace root `.cargo/config.toml` gained a
  `[target.riscv32im-unknown-none-elf]` rustflags stanza (linker script,
  `panic=abort`, etc.), copied verbatim from `example-contracts/.cargo/config.toml`.
  Without it the rv32 link fails on `undefined symbol: __stack_top`. It is
  target-gated and cannot affect host or wasm builds.

## Key facts

- **We parse the ELF, not the `.rv32a`.** The spec fragments live in the ELF
  that `cargo build` emits. `rv32-lower` is **not** in the spec path — it only
  packages the deploy artifact and drops the spec from VM memory. Per the rv32
  author toolchain doc, "indexers read the spec, not the artifact."
- The interface compiles to a **zero-entrypoint** contract (a trait registers
  no dispatch methods). The v4 lowerer accepts this, and spec extraction does
  not care about entrypoints. Confirmed: the ERC20 carrier ELF yields the full
  `ERC20` interface with all 10 methods.

## What `groundhog build --target rv32` (groundhog-rpc) should do

This is where the rv32 spec extraction belonged and appears to be missing. Wire
the rv32 build path to call `groundhog_spec_tool::elf::elf_to_spec` on the
freshly built ELF and emit the spec JSON sidecar, exactly as the wasm path emits
its spec. No new extraction logic is needed in the CLI — depend on
`groundhog-spec-tool` and call the new `elf` module.

For the interface case specifically: a trait-only interface compiles to a
zero-entrypoint contract, so relax any "must have ≥1 entrypoint" guard in the
build/lower path for interface builds.

Ergonomic optional add: `groundhog spec-interface -p <crate>` that runs the
rv32 build + `elf_to_spec` and writes the JSON in one step.
