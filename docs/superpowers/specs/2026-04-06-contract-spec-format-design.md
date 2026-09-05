# Contract Spec Format

## Summary

Add the first full contract-spec format crate and use it as the canonical model
for embedded ABI export.

This slice defines:

- the hand-written wire model for contract specs
- the raw fragment kinds emitted during macro expansion
- the postprocessing responsibilities that assemble one canonical final spec

It does not yet add derive macros for user-defined types, and it does not yet
wire events into the spec.

The intended pipeline is:

1. compile/link emits raw spec fragments into a staging custom section
2. a postprocessing tool reads and validates those fragments
3. the tool assembles one canonical `ContractSpec`
4. the tool writes the final XDR payload into the final wasm custom section
5. the tool may also emit a debug JSON sidecar

## Problem

Groundhog now has crate-level interface registration, but not a full embedded
contract spec model.

To support on-chain ABI fetch, JSON export, and downstream TypeScript codegen,
the embedded spec needs to describe:

- which interfaces a contract exposes
- what methods those interfaces contain
- what each method takes and returns
- what error types the interface may throw
- what named user-defined types those signatures rely on

The resulting spec must be:

- self-contained enough for downstream tooling
- emitted by Rust compilation, not by scraping source files
- friendly to imported traits and imported named types
- embeddable in one final wasm custom section

## Goals

- Add a new crate containing the canonical contract-spec wire model.
- Keep the model close to XDR structure, with contract-specific builtins layered
  on top.
- Represent named user-defined types explicitly in the spec.
- Support:
  - interfaces
  - methods
  - interface-level `throws`
  - named type definitions
- Use name-based references throughout emitted fragments and assembled specs.
- Keep method entrypoint IDs derived from interface metadata and method names,
  not stored in the spec.
- Preserve enough information in the spec to derive eventual XDR
  serialization.
- Continue using compile-time fragment emission plus post-link canonicalization.
- Leave room for a later postprocessor pass that re-derives type shapes and
  checks that they match emitted named type definitions.

## Non-goals

- No events in this slice.
- No derive macros in this slice.
- No replacement of the legacy `src/spec.rs`
  path in this slice.
- No indexed type references in emitted fragments or in the canonical wire
  format.
- No final customization surface for user-defined type symbols yet.
- No attempt to remove the post-link spec pass.
- No attempt to const-build the full transitive spec graph in one place.

## Core Model

Add a new workspace crate named `groundhog-contract-spec`.

That crate owns the canonical wire model and derives `ReadXDR` / `WriteXDR` on
the hand-written types directly. It does not contain proc macros or build
orchestration.

Top-level model:

```rust
struct ContractSpec {
    interfaces: Vec<ContractSpecInterface>,
    type_defs: Vec<ContractTypeDef>,
}

struct ContractSpecInterface {
    name: String,
    version: String,
    throws: Vec<ContractType>,
    methods: Vec<ContractSpecMethod>,
}

struct ContractSpecMethod {
    name: String,
    args: Vec<ContractField>,
    return_type: ContractType,
}

struct ContractField {
    name: String,
    ty: ContractType,
}
```

Notes:

- `throws` lives on the interface, not on each method.
- A method with no return uses `ContractType::Void`.
- Method entrypoint IDs are derived from:
  - interface name
  - interface version
  - method name

They are not stored redundantly in the spec.

## ContractType

`ContractType` is the signature language used by method args, returns, throws,
and named type definitions.

Initial shape:

```rust
enum ContractType {
    Void,
    Bool,
    U32,
    I32,
    U64,
    I64,
    String,
    Bytes,
    Address,

    Option(Box<ContractType>),
    Vec(Box<ContractType>),
    Tuple(Vec<ContractType>),

    Named(String),
}
```

Design notes:

- `Tuple` is first-class because contract APIs may return tuples directly.
- `Named(String)` is the only user-defined type reference form.
- The symbol is a plain string, not an index and not a Rust parser artifact.
- The exact derivation/customization of that symbol is deferred to a later
  derive-macro slice.

The postprocessor may later add an indexed internal representation for
canonicalization, but that is not part of this emitted format.

## Named Type Definitions

Named types are emitted separately in `type_defs` and referenced from
signatures through `ContractType::Named(symbol)`.

The definition model starts with the full XDR-like set from the design sketch:

```rust
enum ContractTypeDef {
    Struct(ContractStructDef),
    Enum(ContractEnumDef),
    Union(ContractUnionDef),
}

struct ContractStructDef {
    name: String,
    fields: Vec<ContractField>,
}

struct ContractEnumDef {
    name: String,
    variants: Vec<ContractEnumVariant>,
}

struct ContractEnumVariant {
    name: String,
    value: i32,
}

struct ContractUnionDef {
    name: String,
    cases: Vec<ContractUnionCase>,
}

struct ContractUnionCase {
    name: String,
    discriminant: i32,
    case_type: Option<ContractType>,
}
```

Notes:

- Enum values and union discriminants are explicit `i32`.
- Union cases may be void via `case_type: None`.
- Struct fields reuse the same `ContractField` record as method args.
- Type definitions are name-based and may arrive unordered in the raw fragment
  stream.

## Type Reflection Strategy

Named type emission should use Rust type resolution, but only for local
per-type descriptions.

The intended model is:

- a type exposes a const-resolved self-description of its own definition
- fields and cases refer to child types through typed `ContractType` refs
- the compiler resolves those child types normally
- that one local definition is serialized into a raw `TypeDef` fragment for the
  staging custom section

This means named type generation should eventually look more like:

```rust
trait ContractTypeSpec {
    const SYMBOL: &'static str;
    const TYPE_REF: ContractTypeView<'static>;
    const TYPE_DEF: ContractTypeDefView<'static>;
}
```

or an equivalent shape built from:

- a typed const self-description
- a local serializer that turns that one definition into a `SpecFragment`

What this strategy does not try to do:

- recursively concatenate the full reachable type graph in const-eval
- dedup transitive type definitions in const-eval
- prove whole-contract closure during macro expansion

Those are postprocessor responsibilities.

This split gives the desired ergonomic property:

- the macro can emit `<T as ContractTypeSpec>::...`
- Rust resolves `T`, including imported types and concrete generic
  instantiations
- the linker section still receives raw bytes
- the postprocessor remains the only place that performs graph assembly,
  deduplication, and missing-type validation

## Fragment Model

Use the existing two-phase spec pipeline:

1. macro expansion emits raw spec fragments into a staging custom section
2. the postprocessor assembles one canonical final spec from those fragments

For this slice, raw fragments should be rich enough to build the full spec but
still local enough that each macro expansion site can emit them independently.

The new spec crate should define a top-level fragment type for the staging
section:

```rust
enum SpecFragment {
    Interface(SpecInterfaceFragment),
    Method(SpecMethodFragment),
    TypeDef(ContractTypeDef),
}

struct SpecInterfaceFragment {
    package_name: String,
    interface_name: String,
    interface_version: String,
    throws: Vec<ContractType>,
}

struct SpecMethodFragment {
    package_name: String,
    interface_name: String,
    interface_version: String,
    method: ContractSpecMethod,
}
```

The staging custom section should be encoded as a concatenated stream of XDR
`SpecFragment` values. The postprocessor reads the section by repeatedly
decoding `SpecFragment` until EOF.

This keeps the raw artifact aligned with the real spec model and avoids an ad
hoc tagged text format for the staging section.

## Emission Rules

### Trait definitions

`#[contract] trait Foo` defines interface metadata, including interface-level
`throws`.

Trait definitions do not by themselves mean that the current crate exports that
interface, so they do not emit exported method fragments for the current crate.

However, trait definitions must make their interface metadata reusable by impl
sites in downstream crates. The existing hidden interface-name/version metadata
pattern should be extended so trait impl codegen can reuse the trait-owned:

- interface name
- interface version
- interface-level throws spec

without recomputing or duplicating those semantics on the impl side.

### Trait impls

`#[contract] impl Foo for Bar` emits the exported interface and method facts for
the current crate:

- one `Interface` fragment keyed to `Foo`'s interface identity
- one `Method` fragment per exported method, keyed to the same identity

The identity and throws metadata come from the trait-owned hidden metadata, not
from the impl item's local syntax.

### Bare impls

`#[contract] impl Bar` emits:

- one implicit `Interface` fragment
- one `Method` fragment per exported method

Its interface identity follows the existing rules:

- explicit `interface = "..."`
- otherwise the self-type-derived interface name
- explicit `version = "..."`
- otherwise the defaulted crate version

Its interface-level `throws` come from the bare impl's own contract config.

### Free functions

`#[contract] fn foo(...)` emits:

- one `Interface` fragment for its interface bucket
- one `Method` fragment for the function itself

Free-function identity follows the existing rules:

- explicit `interface = "..."`
- otherwise the unnamed-interface sentinel used in method hashing
- explicit `version = "..."`
- otherwise the defaulted crate version

Repeated free functions landing in the same `(interface_name, interface_version)`
bucket are expected and will merge in postprocessing.

All `Interface` fragments for a given free-function bucket must agree on the
canonical interface-level `throws` list. If the same bucket is emitted with two
different `throws` lists, postprocessing fails.

### Named types

User-defined structs, enums, and unions eventually need to emit `TypeDef`
fragments.

This slice does not add derive macros yet, but the raw fragment model and
postprocessor must already support type-def fragments so that the first manual
end-to-end implementations can be wired in without redesigning the format.

Those manual implementations should follow the type-reflection strategy above:

- define the type's own symbol and local definition
- wrap only that local definition in `SpecFragment::TypeDef` before emission
- reference child types symbolically through `ContractType`

## Postprocessing

The postprocessor reads raw fragments from the staging section and assembles one
canonical `ContractSpec`.

Responsibilities:

1. parse a stream of XDR `SpecFragment` values
2. filter interface and method fragments to the requested package
3. group methods under `(interface_name, interface_version)`
4. merge repeated free-function interface buckets
5. assemble the final sorted interface list
6. dedup named type definitions by symbol
7. error on conflicting type definitions for the same symbol
8. check closure:
   - every reachable `ContractType::Named(symbol)` must have a matching
     definition
9. serialize the final `ContractSpec` as XDR into the final custom section
10. optionally emit a JSON/debug rendering of the same canonical spec

Type-def fragments are not package-filtered up front. Imported named types are
expected to be resolvable from the linked artifact as a whole. The postprocessor
should start from the kept interfaces and methods, compute the reachable named
symbols, and then require matching definitions from the available `TypeDef`
fragments regardless of which crate originally emitted them.

The postprocessor is also the right layer for later stronger validation, such
as:

- comparing re-derived type definitions against emitted named type definitions
- pruning unreachable type defs
- adding an internal indexed representation for optimization

Those later checks should operate on the canonical assembled spec, not on the
raw `SpecFragment` stream.

## Canonicalization Rules

The canonical assembled `ContractSpec` should be deterministic.

At minimum:

- interfaces sort by `(name, version)`
- methods sort by method name within an interface
- type defs sort by symbol
- struct fields preserve declared order
- enum variants preserve declared order and explicit values
- union cases preserve declared order and explicit discriminants

This gives stable XDR and stable JSON/debug output across rebuilds.

## Build Artifacts

The build pipeline for contract wasm remains:

1. raw spec fragments are emitted into a staging custom section during compile
2. the existing post-link spec tool reads those fragments
3. the tool writes one final embedded custom section
4. the tool may also write a sidecar JSON/debug file

For this slice:

- the staging section format remains an implementation detail
- the staging section payload is a concatenated XDR stream of `SpecFragment`
- the final section payload is the XDR serialization of `ContractSpec`
- the JSON/debug sidecar is for inspection only and is not the canonical wire
  format

## Rollout

1. Add the new `groundhog-contract-spec` crate with the full hand-written wire
   model.
2. Extend raw fragment emission to support:
   - interface fragments
   - method fragments
   - type-def fragments
3. Update the postprocessor to assemble and serialize a full `ContractSpec`.
4. Keep events out of scope for this slice.
5. Leave automatic type re-derivation and comparison as the next validation
   slice once named type fragment emission exists end-to-end.

This keeps the scope focused on getting the full model and artifact pipeline in
place before adding derive ergonomics or deeper validation.
