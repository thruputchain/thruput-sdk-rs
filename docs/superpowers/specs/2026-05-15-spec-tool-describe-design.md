# `groundhog-spec-tool describe`: export SDK-shared type specs

## Goal

Expose specs for SDK-defined, cross-cutting types (starting with `LibraryExn`)
through `groundhog-spec-tool` so downstream consumers — primarily the
TypeScript bindings — can decode them without re-implementing the type
definitions.

Today `groundhog-spec-tool` only extracts specs from compiled wasm modules
(the `rewrite` subcommand). SDK-internal types like `LibraryExn` are baked
into every contract and every client but have no public spec emission path.
This design adds one.

## CLI

```
groundhog-spec-tool describe <TypeName> [--json|--xdr] [-o <path>]
groundhog-spec-tool describe --list
```

- `<TypeName>` is one of a fixed set of supported SDK-shared types.
  v1: `LibraryExn`.
- `--list` prints supported type names, one per line, plain text, to stdout.
  Mutually exclusive with the `<TypeName>` positional.
- `--json` and `--xdr` are mutually exclusive output formats. Default `--json`
  regardless of destination.
- `-o <path>` writes to the given file. Default stdout. Binary XDR to stdout
  is permitted (`describe LibraryExn --xdr | xxd` is a valid workflow).
- Extension inference on `-o` is intentionally **not** done; the explicit
  flag is one keystroke and easier to reason about.

## Output

`describe <TypeName>` reuses the existing `ContractSpec` pipeline. The
binary:

1. Collects the `SpecFragmentView` constants for `<TypeName>` and its
   transitive closure of referenced types (manually enumerated — see
   "Type registry" below).
2. Converts each `SpecFragmentView` to `SpecFragment` and feeds the
   collection to a new `canonicalize_type_defs` entry point in
   `groundhog-spec-tool` (see "Required spec-tool change").
3. Emits the resulting `ContractSpec`:
   - `--json`: passes through `render_spec_json`.
   - `--xdr`: passes through `WriteXDR` on `ContractSpec`.

Output shape matches what `rewrite` already produces for the wasm-extraction
path: a `ContractSpec` with an empty `interfaces` array and `type_defs`
populated with the type's closure. TS consumers can parse one schema for
both paths.

## Required spec-tool change

`canonicalize_spec` currently returns `SpecToolError::NoSpecFragments` when
`interfaces` is empty. For the type-only describe path that check is wrong:
we have type defs but no interfaces, by design.

Add a sibling entry point:

```rust
pub fn canonicalize_type_defs(
    raw: &[SpecFragment],
    roots: &[ContractSpecString],
) -> Result<ContractSpec, SpecToolError>
```

It reuses the same `resolve_reachable_type_defs` logic that `canonicalize_spec`
already calls, with `roots` populated from the caller (rather than discovered
from interface signatures), and returns a `ContractSpec` with empty
`interfaces` and resolved `type_defs`.

The existing `canonicalize_spec` keeps its semantics unchanged.

## Required SDK changes

`LibraryExn` and the types it references do not currently derive
`ContractTypeSpec` (the derive that emits per-type `SpecFragmentView`
constants). Add `#[derive(ContractTypeSpec)]` (gated on whatever feature it
is already gated on elsewhere) to:

- `LibraryExn` (union)
- `SyscallExn`
- `HandlingError`
- `CallingError`
- `AuthTokenError`
- `FaultString`
- `StorageError` (gated on `feature = "storage"`)

Plus any transitively-referenced leaf types that don't already derive it.
The exact closure will be confirmed during implementation by walking the
type definitions.

When `groundhog-spec-tool` builds with the main SDK as a dependency, the
spec-feature flag is enabled so these fragments are present.

## Type registry in the binary

A static match in `groundhog-spec-tool/src/main.rs`:

```rust
fn fragments_for(type_name: &str) -> Option<(&'static [SpecFragmentView<'static>], ContractSpecString)> {
    match type_name {
        "LibraryExn" => Some((LIBRARY_EXN_FRAGMENTS, "LibraryExn".into())),
        _ => None,
    }
}
```

`LIBRARY_EXN_FRAGMENTS` is a manually-curated `&'static [SpecFragmentView]`
referencing the `FRAGMENT` constants of `LibraryExn` and each transitively
required type. Adding a future shared type = new match arm + new fragment
slice. `--list` walks the same registry.

## Completeness test

Add a test in `groundhog-spec-tool` that, for every entry in the type
registry:

1. Resolves the `ContractSpec` via `canonicalize_type_defs`.
2. Walks every `ContractType::Named { named }` in every `type_def`
   (including struct fields, union case fields, vec/option/array/tuple
   element types).
3. Asserts each `named` reference resolves to a `type_def` in the same
   output.

This catches missing-leaf bugs at CI time rather than at TS-consume time
when a new type is added to a transitive position.

## Dependency direction

`groundhog-spec-tool` gains a dependency on the main SDK crate
(`groundhog`). The `rewrite` subcommand path is unaffected and remains
SDK-free (it still operates on arbitrary wasm).

We are explicitly deferring any split of `LibraryExn` and its referenced
types into a smaller, SDK-independent crate. If the dependency footprint
becomes a problem later, that split is a follow-up.

## Non-goals

- Automatic enumeration of all `ContractTypeSpec`-deriving types from the
  SDK. The registry is hand-maintained for now.
- Spec emission for contract-defined types — that path is `rewrite`
  (wasm-extraction), unchanged.
- A new public output schema. We reuse the existing `ContractSpec` JSON
  and XDR forms.

## Open questions / follow-ups

- Confirm the exact feature flag used to gate `ContractTypeSpec` derives on
  SDK-internal types (likely the existing `spec` feature; verify during
  implementation).
- Audit the closure of `LibraryExn`'s referenced types for any leaves
  missed in the list above (e.g. error-string newtypes, integer-newtype
  fields) during implementation.
