# Note: building interface specs on the rv32 path

## The extraction already exists in groundhog-rpc

`groundhog build --target rv32` already extracts and emits the contract spec
from the rv32 ELF. No new extraction code is needed. The wiring (in
`../../groundhog-rpc`):

- `crates/tools/cli/src/build.rs::build_rv32_project` — compiles the rv32 ELF,
  lowers it, then calls `contract_spec::finalize_rv32_spec(&elf, &rv32, …)` and
  emits `spec_json` + `spec_xdr`.
- `crates/tools/cli/src/contract_spec.rs::load_rv32_contract_spec_from_bytes` —
  parses the ELF with **the lowerer's own dependency-free reader**
  (`groundhog_rv32_lower::elf::parse_elf`), slices the `groundhogspecrawv0`
  section bytes, and canonicalizes via `groundhog_spec_tool::parse_raw_fragments`
  + `canonicalize_spec`.
- `crates/rv32/lower/src/lower.rs` — the lowerer itself only *drops* the spec
  sections (`SPEC_METADATA_SECTIONS`) from the VM image; it deliberately has no
  spec-tool dependency.

So ELF parsing on the rv32 path uses `groundhog_rv32_lower::elf::parse_elf`, not
the `object` crate. sdk_rs does **not** add its own ELF reader (an `object`-based
reader was prototyped and removed — it duplicated this path, and sdk_rs cannot
depend on `groundhog-rv32-lower` anyway without a cross-repo cycle, since
groundhog-rpc already depends on `groundhog-spec-tool`).

## What sdk_rs provides

- `#[contract]` on a trait now emits the `groundhogspecrawv0` fragments (same
  format as impls).
- `groundhog-erc20-if` builds into a zero-entrypoint rv32 spec-carrier via an
  `entrypoints = []` feature + a thin `[[bin]]` (`extern crate groundhog_erc20_if;`).
  `crate-type` stays `["rlib"]`.
- The workspace-root `.cargo/config.toml` gained a riscv32-gated linker stanza
  (copied from `example-contracts`); target-gated, cannot affect host/wasm.

## Open items for groundhog-rpc (owned there; integration tests live there)

1. **Verify the lowerer accepts a zero-entrypoint interface ELF.**
   `build_rv32_project` lowers *before* `finalize_rv32_spec`, so if
   `groundhog_rv32_lower::lower` rejects an interface carrier (e.g. on the
   `.text`/entry-layout rules), the spec never gets emitted on the `build` path.
2. **Consider an extract-without-lower path for interfaces.** An interface is
   not a deployable contract; producing a `.rv32a` for it is meaningless. A
   `groundhog spec-interface -p <crate>` (or `build --spec-only`) that runs the
   cargo build + `finalize_rv32_spec` while skipping lowering would be the
   ergonomic entry point for interface specs.
