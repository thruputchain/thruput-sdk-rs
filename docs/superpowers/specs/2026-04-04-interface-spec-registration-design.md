# Interface Spec Registration

## Summary

Add the first contract-spec export slice: interface registration.

This slice does not attempt full method, event, or type reflection yet. It
only establishes the pipeline for getting contract interface identity into the
final wasm artifact in a way that:

- is driven by Rust macro expansion, not source scraping
- works for trait impls, bare impls, and free functions
- preserves the exact interface identity used in method-id derivation
- allows a post-link pass to validate, merge, dedupe, and rewrite the final
  embedded section

The output model is:

1. compile/link emits raw appendable interface fragments into a staging custom
   section
2. a postprocessing tool reads those fragments from the built wasm
3. the tool validates and merges them
4. the tool writes one final canonical custom section and optionally a sidecar
   ABI artifact such as JSON

This is the first implementation slice of crate-global contract spec export.

## Problem

Groundhog currently has no crate-global contract spec artifact embedded in the
compiled wasm.

That means there is no standard way to:

- fetch an ABI-like description from a deployed contract
- generate a JSON artifact at build time from the same source of truth
- aggregate multiple exported interfaces in one contract crate
- support free-function interfaces and trait-based interfaces uniformly

The desired long-term model is:

- one contract crate corresponds to one deployed contract
- that contract may expose multiple interfaces
- the contract spec must be self-contained enough for downstream tooling
- the on-chain wasm should carry the spec

The immediate slice here is just interface identity registration, because that
is the smallest piece that proves the build and embedding architecture.

## Goals

- Emit interface identity metadata for every exported contract surface.
- Always include this metadata in contract wasm builds.
- Support:
  - `#[contract] impl Trait for Type`
  - `#[contract] impl Type`
  - `#[contract] fn foo(...)`
- Preserve the exact interface identity that participates in method-id hashing:
  - interface name literal string
  - interface version literal string
- Support imported traits carrying their own interface name/version metadata.
- Support free functions grouped into named interfaces or the unnamed interface
  bucket.
- Run a post-link pass that merges and dedupes raw fragments and writes one
  final canonical custom section.
- Allow the same post-link pass to optionally emit a convenience JSON artifact.

## Non-goals

- No full method/event/type spec in this change.
- No XDR final format design in this change.
- No native-only runtime spec registry analogous to the mock registry.
- No attempt to remove the post-link pass.
- No mixing of this machinery with the mock-registration implementation.
- No support for multiple impl-backed contract types in one crate.

## Contract Model

The contract model remains:

- one crate corresponds to one deployed contract
- that contract may expose any number of interfaces
- interfaces may come from trait impls, bare impls, and free functions
- free functions may coexist with one impl-backed contract type
- a crate may not export methods from two different impl self types

Examples allowed:

- free functions only
- one bare impl only
- one type implementing multiple contract traits
- one impl-backed contract type plus free functions

Example rejected:

- `impl Foo for Alpha`
- `impl Bar for Beta`

within the same crate, when both impls export contract methods and `Alpha` and
`Beta` are different self types.

## Interface Identity

For this slice, an interface is identified only by the values that participate
in method-id derivation other than the method name itself:

```rust
struct InterfaceId {
    interface_name: String,
    interface_version: String,
}
```

No source-kind tag is part of the semantic identity.

### Trait impls

Trait impl exports must use the trait's actual interface metadata, not an
independently recomputed copy on the impl side.

That means trait impl fragment emission should use the same hidden associated
consts already emitted on contract traits:

- `__GROUNDHOG_IFACE_NAME`
- `__GROUNDHOG_IFACE_FULL_VERSION`

This ensures that:

- imported traits carry their interface identity naturally
- custom interface names and explicit versions survive through impls
- the raw fragment identity matches the actual entrypoint identity

### Bare impls

Bare impl exports use the existing configured/defaulted interface identity:

- explicit `interface = "..."`
- otherwise the self-type-derived interface name
- explicit `version = "..."`
- otherwise the default crate version

### Free functions

Free-function exports use:

- explicit `interface = "..."`
- otherwise the unnamed-interface sentinel already used in entrypoint hashing
- explicit `version = "..."`
- otherwise the default crate version

Free functions with the same `(interface_name, interface_version)` pair are
expected to merge in postprocessing.

## Two-Phase Build Model

### Phase 1: raw fragment emission at compile/link time

Each export-capable `#[contract]` item emits a raw interface fragment into a
staging custom section.

Contributors:

- `#[contract] impl Trait for Type`
- `#[contract] impl Type`
- `#[contract] fn foo(...)`

Non-contributors:

- `#[contract] trait Trait`

Trait definitions provide reusable interface metadata, but do not by
themselves mean that the current crate exports that interface.

The staging section should be appendable. One export contributes one fragment
without needing global crate knowledge.

Section name:

- staging/raw: `groundhogspecrawv0`

The raw fragment format does not need to be final. For this slice it should be
simple and inspectable by humans. A line-oriented or similarly simple tagged
record format is acceptable.

Example shape:

```text
iface\t<package>\t<self_type_or_empty>\t<interface_name>\t<interface_version>\n
```

The only hard requirement is that the post-link tool can parse it reliably.

### Phase 2: post-link normalization

A postprocessing tool runs on the built wasm and:

1. reads the raw fragments from `groundhogspecrawv0`
2. validates crate-level invariants
3. merges and dedupes interface identities
4. writes one final canonical section
5. strips the raw staging section
6. optionally writes a sidecar JSON artifact

Section name:

- final/canonical: `groundhogspecv0`

This gives the deployed artifact one clean embedded section while still
letting macro expansion remain local and append-only.

## Why A Post-Link Pass Is Needed

Unlike the mock-registration flow, spec embedding must exist in the final wasm
before runtime. A runtime registry function cannot synthesize a custom section
after deployment.

An appendable raw section is easy to emit from local macro expansion sites.
However, the final contract artifact should still be normalized:

- free-function duplicates should be merged
- multiple impl-backed self types should be rejected
- section layout should be canonical and stable

That normalization is naturally a whole-wasm concern, so it belongs in a
post-link step.

## Raw Fragment Contents

Each raw fragment should contain enough information to support both validation
and final interface-list generation.

Conceptually:

```rust
struct RawInterfaceFragment {
    package_name: &'static str,
    self_type_key: Option<&'static str>,
    interface_name: &'static str,
    interface_version: &'static str,
}
```

Notes:

- `package_name` is emitted as `env!("CARGO_PKG_NAME")`
- `self_type_key` is `None` for free functions
- `self_type_key` is `Some(...)` for trait impls and bare impls
- `self_type_key` is only for validation; it is not part of the final ABI

The exact serialized representation is an implementation detail.

## Post-Link Validation Rules

When processing one contract wasm:

1. Parse all raw interface fragments.
2. Reject malformed fragments.
3. Collect all distinct non-`None` `self_type_key` values.
4. If more than one distinct impl-backed self type exists, fail.
5. Build the final set of interface identities from all fragments.
6. Merge duplicates by exact `(interface_name, interface_version)`.
7. If the final set is empty, fail.
8. Sort the final set deterministically.
9. Emit one final canonical section.

This means:

- free functions with the same interface name/version merge
- unnamed free functions merge into one unnamed-interface entry
- free functions can coexist with one impl-backed contract type
- impl-backed duplicates are rejected at the whole-contract level

## Final Section Shape

The final canonical section only needs to contain the interface list for this
slice.

Conceptually:

```rust
struct ContractInterfaceList {
    interfaces: Vec<InterfaceId>,
}
```

The concrete byte format can stay intentionally simple for now, so long as it
is deterministic and easy to inspect.

Example:

```text
gh_iface_v1
ExplicitNamedInterface\t0.3.0
VersionedCallee\t1.2.3
_unnamed_interface\t0.3.0
```

This is only a placeholder transport format for the first slice. The eventual
full contract spec can replace it with a better structured representation once
methods, events, and types are added.

## Convenience Artifact

The same post-link tool that rewrites the wasm should also be able to emit a
sidecar artifact such as JSON.

That artifact is not the canonical source of truth. It is a convenience view
for:

- tests
- tooling
- generating TypeScript bindings
- inspecting the intermediate contract ABI without re-extracting from wasm

The canonical deployable artifact remains the final wasm custom section.

## Free Function Behavior

Free functions must work in this pipeline from the first implementation.

Behavior:

- `#[contract(interface = "Erc20")] fn transfer(...)` contributes interface
  `("Erc20", version)`
- another free function with the same interface name/version contributes a
  duplicate raw fragment that later merges
- `#[contract] fn foo(...)` contributes the unnamed-interface sentinel with the
  defaulted version

This matches the current entrypoint behavior and preserves compatibility with
existing low-level/free-function interface patterns.

## Relation To Future Full Spec Export

This interface-registration slice is intentionally narrow, but the build model
should scale directly into full contract spec export.

Future fragment kinds can be added alongside interface fragments:

- interface definitions
- method definitions
- event definitions
- named type definitions

The same post-link pass can then:

- resolve references
- validate completeness
- dedupe named definitions
- write the final full contract spec section

This slice proves the pipeline before the richer reflection work lands.

## Implementation Sketch

1. Add a spec-only raw fragment emitter in codegen for:
   - trait impls
   - bare impls
   - free functions
2. Emit raw fragments into `groundhogspecrawv0`.
3. Preserve trait interface metadata by reading hidden associated consts for
   trait impls.
4. Add a post-link wasm tool that:
   - parses raw fragments
   - validates impl-backed self-type uniqueness
   - merges and sorts interface identities
   - writes `groundhogspecv0`
   - removes `groundhogspecrawv0`
   - optionally emits JSON
5. Hook that tool into the existing contract wasm build path.
6. Add example-contract tests covering:
   - trait impl interfaces
   - bare impl interfaces
   - free functions with explicit interface grouping
   - unnamed free functions
   - imported traits carrying explicit interface names/versions

## Testing Strategy

The first slice should verify both the raw and final paths.

Compile/build tests:

- a contract with multiple imported traits emits the expected raw fragments
- a free-function-only crate emits unnamed or explicit interface fragments
- a crate with two impl-backed self types is rejected by postprocessing

Artifact tests:

- built wasm contains only `groundhogspecv0` after postprocessing
- the final section decodes to the expected sorted interface list
- JSON sidecar matches the same canonical interface list

Regression tests:

- named imported trait interface remains named correctly
- explicit trait version survives into the final section
- free functions grouped under one named interface produce one final interface
  entry

## Open Questions

- The exact raw fragment encoding can stay simple for now, but should be chosen
  so future fragment kinds can be added without ambiguity.
- The exact final section text/binary format is intentionally deferred until
  method/event/type export is designed.
- Whether the post-link tool should live in this repository as a Rust crate or
  as part of an existing dev/build utility is an implementation choice, not a
  design constraint.
