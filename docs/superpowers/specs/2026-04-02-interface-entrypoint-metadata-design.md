# Standardize Interface Entrypoint Metadata

## Summary

Standardize all generated entrypoint metadata around a single notion of
"effective interface name", and expose public entrypoint identifiers through a
generated per-interface wrapper type instead of through the generated exports
module.

This design has two user-visible goals:

1. Bare impls should default to the self type name as their interface/domain
   separator instead of `_unnamed_interface`.
2. Public entrypoint constants should no longer live in the generated exports
   module. They should be available through a stable, interface-oriented API.

## Motivation

The SDK currently has two related ergonomics problems:

1. The codebase talks about interface names as though every exported contract
   surface has one, but bare impls currently hash their entrypoints using
   `_unnamed_interface` unless the user explicitly writes `interface = "..."`
   on the impl.
2. Public `*_EXPORT_NUM` constants are generated inside the exports module,
   which leaks an internal implementation detail into downstream tests and
   examples.

This yields awkward APIs such as:

- downstream tests depending on `implementation::exports::FOO_EXPORT_NUM`
- users reasoning about trait/interface names while bare impls quietly use a
  different domain separator
- `exports = "..."` effectively shaping part of the public API even though its
  real purpose is internal codegen namespacing

## Goals

- Use one consistent "effective interface name" rule anywhere the SDK derives
  entrypoint metadata.
- Make bare impls default to the self type name when no explicit
  `interface = "..."` override is present.
- Expose public entrypoint identifiers via a generated per-interface wrapper
  type.
- Keep generated wrapper functions available for `__mock` and wasm exports
  without exposing their namespace as part of the normal user API.
- Preserve explicit `interface = "..."` overrides across traits, trait impls,
  bare impls, and free functions.

## Non-goals

- Removing the generated internal exports module entirely.
- Cleaning up `exports = "..."` in this change. It can remain as an escape
  hatch for now.
- Changing the free-function default beyond keeping an unnamed fallback.
- Refactoring unrelated client-generation or testing APIs.

## Approaches Considered

### 1. Flat top-level exported constants

Generate public constants like `IERC20_transfer_EXPORT_NUM` or
`IERC20_TRANSFER_EXPORT_NUM` directly in the surrounding module.

Pros:

- Simple implementation
- Easy for downstream tests to import

Cons:

- Pollutes the surrounding namespace
- Scales poorly with many interfaces in one module
- Continues to blur the distinction between public metadata and internal
  generated wrappers

### 2. Trait-associated constants

Generate public associated consts on the contract trait itself.

Pros:

- Feels conceptually close to the interface

Cons:

- Access requires `<Impl as Trait>::CONST`, not `Trait::CONST`
- Does not give interface-only access for downstream tests
- Keeps users entangled with impl types when they only want interface metadata

### 3. Recommended: per-interface wrapper type plus hidden internal module

Generate a public wrapper type for the entrypoint IDs and keep the internal
wrapper functions in a hidden generated module.

Example:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct IERC20Entrypoints(pub ::sdk::contract::ContractEntrypointID);

impl IERC20Entrypoints {
    pub const transfer: Self = Self(/* ... */);
    pub const mint: Self = Self(/* ... */);
}
```

Pros:

- Clean public API
- Interface-only access for downstream tests
- No public dependence on the generated exports module
- Leaves room for helper impls later

Cons:

- Slightly more codegen than flat constants
- Still requires an internal hidden module for wrapper functions

This is the chosen design.

## Effective Interface Name

Introduce a single codegen concept: the effective interface name.

This should be used consistently for:

- entrypoint ID derivation
- public generated entrypoint wrapper-type naming
- hidden internal exports-module naming
- generated diagnostics that talk about interface naming

Resolution rules:

### Trait definition

- `interface = "..."` if present
- otherwise the trait name

### Trait impl

- `interface = "..."` if present on the impl
- otherwise the trait-side effective interface name via the existing
  trait/impl consistency model

### Bare impl

- `interface = "..."` if present
- otherwise the self type name

### Free function

- `interface = "..."` if present
- otherwise `_unnamed_interface`

## Public Generated Metadata API

For each trait, trait-impl, or bare-impl effective interface, generate one
public wrapper type:

- name: `<EffectiveInterfaceName>Entrypoints`
- representation: `#[repr(transparent)]`
- inner field: `ContractEntrypointID`
- derives:
  - `Clone`
  - `Copy`
  - `Debug`
  - `PartialEq`
  - `Eq`
  - `Hash`
  - `PartialOrd`
  - `Ord`

Associated constants:

- one per exported method
- constant names use the method identifier exactly as written in Rust
- example: `IERC20Entrypoints::transfer`
- value type: `Self`

Recommended helper impls:

- `impl From<<Iface>Entrypoints> for ContractEntrypointID`
- optionally `impl From<&<Iface>Entrypoints> for ContractEntrypointID` if useful

This wrapper type is the intended public surface for downstream tests and other
metadata consumers.

### Free functions

Free `#[contract] fn` items are treated as one-off exported methods, not as a
multi-method interface surface.

They should:

- continue to derive their entrypoint IDs from the effective interface name
- continue to support explicit `interface = "..."` overrides
- generate a hidden internal exports module for the wrapper functions

They should not:

- generate a public `<Interface>Entrypoints` wrapper type

This keeps the public API model aligned with existing behavior: free functions
do not currently generate a shared client or other interface-level metadata
surface. If users want a grouped public interface with entrypoint metadata,
they should use a bare impl or a trait/trait impl.

## Internal Generated Exports Module

The generated exports module still exists, but only as an internal namespace for
generated wrapper functions:

- `foo_extern`
- `foo_internal`

`__mock` still needs a stable path to `foo_internal`, so the wrapper-function
namespace cannot be removed entirely in this change.

The module name should no longer be a normal public-facing name like `exports`
or `<TraitName>_exports` by default. Instead, generate a hidden internal name
derived from the effective interface, for example:

- `__groundhog_exports_IERC20`

For free-function codegen only, append the function name so multiple one-off
exports in the same module do not collide:

- `__groundhog_exports_IERC20_transfer`

If `exports = "..."` is explicitly provided, it may continue to override the
internal module name for this change. The public metadata surface should still
come from the generated wrapper type for trait/impl surfaces, not from that
module.

## Relationship to `exports = "..."`

This change does not remove `exports = "..."`.

For now:

- keep it as an escape hatch for internal wrapper-module naming
- stop relying on the exports module for public `*_EXPORT_NUM` access
- treat the generated wrapper type as the normal public API

That means `exports = "..."` becomes less important to ordinary users without
requiring a broader cleanup in this PR.

## ABI Changes

This design intentionally changes bare-impl entrypoint ID derivation.

Current bare-impl default:

- `_unnamed_interface`

New bare-impl default:

- self type name

Example:

```rust
#[contract]
impl Counter {
    pub fn increment(&mut self) {}
}
```

Before:

- entrypoint domain separator: `_unnamed_interface`

After:

- entrypoint domain separator: `Counter`

This is an ABI change. That is acceptable for now because the current request
explicitly prioritizes consistency over backwards compatibility.

Trait interfaces and explicit `interface = "..."` overrides continue to behave
consistently with their configured effective interface names.

## Testing Strategy

This change should start test-first.

### Red tests to add first

1. Compile-pass test for bare impl default interface naming
   - define a bare impl without `interface = "..."`
   - assert the generated public metadata path uses the self type name
   - example target shape: `CounterEntrypoints::increment`

2. Compile-pass test for explicit interface override
   - define a contract using `interface = "Renamed"`
   - assert the generated public metadata path is
     `RenamedEntrypoints::method`

3. Compile-fail test for old exports-module constant access
   - demonstrate that `exports::FOO_EXPORT_NUM` (or equivalent old path)
     is no longer the supported public surface

4. Runtime cross-call example for bare impl default naming
   - add or update an `example-contracts` pair that exercises a bare impl
     without explicit `interface = "..."`
   - verify cross-call method IDs still line up end to end under the new
     self-type default

### Follow-up verification

- existing explicit-interface example should keep working
- existing trait-interface version/name assertion tests should remain green
- mock registration should continue to use the hidden internal wrapper module

## Implementation Outline

1. Introduce a single helper in codegen for resolving the effective interface
   name.
2. Update bare impl interface naming to default to self type name.
3. Change entrypoint metadata generation so public constants are emitted via the
   new wrapper type instead of inside the exports module.
4. Rename internal exports-module generation to a hidden unique naming scheme.
5. Update `__mock` codegen to reference the hidden internal module.
6. Update docs/examples to use the new public metadata surface.

## Risks

- Hidden internal module naming must be deterministic enough for generated code
  to reference it consistently.
- Moving public constants out of the exports module may break tests that relied
  on the old internal path. That is intentional, but the migration should be
  reflected in examples and docs.
- Bare-impl ABI changes may invalidate ad hoc tests or handwritten callers that
  were built against the old `_unnamed_interface` behavior.

## Open Questions

None for this scoped change.
