# Crate-path override for `#[derive(ContractTypeSpec)]`

## Problem

`#[derive(ContractTypeSpec)]` emits literal `::sdk::…` paths. Any crate that
`sdk` itself depends on therefore cannot use the derive — naming `::sdk` from
below `sdk` is a dependency cycle. Such a crate has to hand-write
`ContractTypeSpec`, `ContractTypeRefSpec`, the linker-anchor fn, and the raw
typedef link-section fragment.

Hand-written spec consts are a drift risk. They encode the spec wire format by
hand; if the derive's encoding changes, they silently stop matching.

`crates/u256` (PR #336) is exactly this case and is the motivating consumer.

## Key observation

Everything the derive emits under `::sdk::` is a re-export of a crate *below*
`sdk`:

- `::sdk::__contract_spec::…` — re-export of `groundhog-contract-spec`
- `::sdk::__xdrs::txs::ConstWriter` — re-export of `xdrs`

Nothing in this derive is owned by `sdk`. A consuming crate can name both
targets directly, so the whole problem is a path problem, not a dependency
problem.

## Design

### Attribute

`#[derive(ContractTypeSpec)]` gains a `crate = "..."` argument:

```rust
#[derive(ContractTypeSpec)]
#[contract_type_spec(crate = "::groundhog_u256::__sdk_paths")]
pub struct U256(...);
```

The consuming crate supplies a shim module matching the two names the macro
expects:

```rust
#[doc(hidden)]
pub mod __sdk_paths {
    pub use groundhog_contract_spec as __contract_spec;
    pub use xdrs as __xdrs;
}
```

The default is `::sdk`, so every existing call site is untouched and its output
is byte-identical.

### Parsing

`ContractTypeSpecArgs` grows a field:

```rust
#[derive(Debug, Default, FromMeta)]
struct ContractTypeSpecArgs {
    #[darling(default)]
    export: bool,
    #[darling(default, rename = "crate")]
    krate: Option<syn::Path>,
}
```

`crate` is a Rust keyword, hence the `rename`. darling's `FromMeta for
syn::Path` accepts the string form.

Repeated `#[contract_type_spec(...)]` attributes keep OR-ing `export`, as
today. A second, *differing* `crate` value is an error rather than silent
last-wins.

### Threading

A `krate: &syn::Path` parameter flows through the emitters:
`emit_raw_typedef_fragment`, `struct_type_def_tokens`, `enum_type_def_tokens`,
`struct_field_tokens`, `field_anchor_tokens`, `field_anchor_token`,
`enum_variant_case_type_tokens`, `enum_anchor_tokens`.

Field types route through `common::contract::contract_type_view_tokens`, which
is shared with `derive_event`. Rather than change that signature and disturb
out-of-scope macros, add `contract_type_view_tokens_in(krate, ty)` and
`lower_via_type_ref_trait_in(krate, ty)`, and keep `contract_type_view_tokens`
as a thin wrapper passing `::sdk`.

`contract_type_name_impl` in `lib.rs` takes the same parameter; its two callers
(`#[contract_type]`, `#[contract_exception]`) pass the default.

## Testing

- `crates/macros/tests/crate_override.rs` — dev-depends on
  `groundhog-contract-spec` and `xdrs`, with no `sdk` in the graph. Defines the
  shim module, derives with the override, and asserts the resulting `TYPE_DEF`
  and encoded fragment bytes. That it compiles at all is the proof a below-sdk
  crate works.
- Unit tests: default output still emits `:: sdk ::`; overridden output emits
  the given path and contains no `sdk` token anywhere; `export` works alone and
  combined with `crate`.

## Out of scope

The `#[contract_type]` attribute macro. It also emits impls of traits genuinely
owned by `crates/sdk` (`CollectAuthHandles`, `ReceiveAuthHandles`,
`MintableType`), which a lower crate cannot implement. That is a dependency
problem, not a path problem, and would need either a suppression flag or those
traits moving down. No consumer needs it today.

Also unchanged: `contract_exception.rs`, `derive_event.rs`, `storage.rs`,
`deletable.rs`, `contract.rs`.

`crates/u256` is not on `main` — PR #336 is still open. This PR ships the macro
change only; u256 adopts the derive in its own PR.

## Handoff to PR #336

The derive names an unnamed field `_0`. `U256` is a tuple struct whose
hand-written spec declares a **named** field `limbs`, so adopting the derive as
written changes the field name — a wire-visible ABI change for a type non-Rust
SDKs will consume.

Cleanest resolution, which needs no macro change: make `U256` a named-field
struct, `pub struct U256 { pub(crate) limbs: [u32; LIMBS] }`. The good public
name survives and only `.0` call sites inside the crate shift.

## Constraints

- `link_section` names stay exactly `groundhogspecrawv1` and
  `groundhogspecanchortextv0` (`groundhog_spec_tool::RAW_SECTION_NAME` and the
  spec-tool tests guard drift).
- `#[used]` stays riscv32-only; on wasm it would force fragment bytes into
  linear memory and trip the spec-tool leak guard.
- New dev-dependency edges mean the six `Cargo.lock` files may need syncing
  (repo root, `example-contracts/`, `example-contracts/cross_crate_collision/`,
  `example-contracts/multi_root_collision/`, `integration_test/`,
  `logging-test/`). CI builds `--locked`.
