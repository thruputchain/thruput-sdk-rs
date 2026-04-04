# Crate-Global Mock Registration

## Summary

Add a crate-global contract export registry for test mock registration, and use
it to generate one mock-registration entrypoint per contract crate.

This is the first implementation slice of the broader contract-spec export
design. It solves an existing ergonomics problem today, while proving the core
architecture needed later for ABI/spec collection and wasm custom-section
embedding.

The key rule is:

- one contract crate corresponds to one deployed contract surface
- the crate may export any number of interfaces
- those interfaces may come from trait impls, bare impls, and free functions
- free functions are allowed alongside one impl-backed contract type
- a crate may not export methods from two different impl self types

## Problem

Today mock registration is fragmented at the individual `#[contract]`
expansion-site level.

Examples:

- A trait impl generates its own `__mock`, accessed as
  `<Type as Trait>::__mock`.
- A bare impl generates its own `Type::__mock`.
- A crate with multiple exported trait impls must manually chain those
  registrations in tests.

Current multi-trait test setup looks like:

```rust
let contract = env.register_mock(|c| {
    <Stateless as interface1::Interface>::__mock(c);
    <Stateless as interface2::Interface>::__mock(c);
});
```

This has two problems:

1. It is user-visible glue code for something the SDK already knows.
2. It exposes the same missing abstraction that the future ABI collector will
   need: there is no crate-global view of all exported contract fragments.

## Goals

- Provide one crate-global mock registration path for a contract crate.
- Preserve existing trait-impl, bare-impl, and free-function contract exports.
- Support crates that implement multiple interfaces on one contract type.
- Support free-function exports grouped into the same crate-level contract
  surface.
- Reject crates that export methods from more than one impl self type.
- Reuse the same conceptual registry seam later for contract-spec collection.

## Non-goals

- No JSON or XDR contract-spec export in this change.
- No wasm custom-section embedding in this change.
- No event/type reflection in this change.
- No support for multiple impl-backed contract types in one crate.
- No redesign of entrypoint generation or client generation.

## Approaches Considered

### 1. Status quo: manual chaining of generated `__mock` functions

Pros:

- No new machinery

Cons:

- Solves neither the ergonomics problem nor the future ABI collector problem
- Forces users to understand generated internal fragments
- Scales poorly as contracts accumulate interfaces

### 2. Per-type aggregator

Generate a single `Type::__mock_all` for each impl-backed contract type.

Pros:

- Better than manual chaining for trait impls

Cons:

- Does not naturally cover free-function exports
- Pushes the abstraction toward "type = contract", while the intended model is
  "crate = contract"
- Does not give the future ABI collector the right seam

### 3. Recommended: crate-global export registry

Each local exported contract fragment registers itself into a crate-global
registry, and the SDK consumes that registry to build one mock contract for the
crate.

Pros:

- Matches the desired semantic model: one crate = one contract
- Covers trait impls, bare impls, and free functions uniformly
- Directly exercises the registry seam needed later for ABI collection
- Allows one public mock-registration API instead of user-written glue

Cons:

- Requires new collection machinery
- Requires validating crate-level invariants, not just per-item invariants

This is the chosen design.

## Contract Model

For this design, a contract crate is defined as:

- zero or more free-function exports
- zero or one impl-backed contract type
- any number of exported interfaces contributed by that type

Allowed examples:

- free functions only
- one bare impl only
- one type implementing multiple `#[contract]` traits
- one impl-backed contract type plus free-function exports

Rejected example:

- `impl Foo for Alpha`
- `impl Bar for Beta`

inside the same crate, where `Alpha` and `Beta` are different self types and
both contribute exported contract methods.

## Registry Model

### Local fragment registration

Each local export-capable `#[contract]` item contributes one crate-global mock
registration fragment.

Contributors:

- `#[contract] impl Trait for Type`
- `#[contract] impl Type`
- `#[contract] fn foo(...)`

Non-contributors:

- `#[contract] trait Trait`

Trait definitions remain reusable interface definitions, but do not by
themselves say that the current crate exports that interface.

### Registry entry contents

Each registration fragment should contain enough information to:

- register its exported entrypoints into a `MockContract`
- report whether it is associated with an impl self type
- identify the package that emitted it

Conceptually:

```rust
pub struct ContractMockFragment {
    pub package_name: &'static str,
    pub self_type_key: Option<&'static str>,
    pub register: fn(&mut sdk::MockContract),
}
```

Notes:

- `package_name` is emitted at the macro expansion site from
  `env!("CARGO_PKG_NAME")`, so it names the contract crate, not the SDK crate.
- `self_type_key` is `None` for free functions.
- `self_type_key` is `Some(...)` for trait impls and bare impls.
- The exact representation of `self_type_key` is an implementation detail; it
  only needs to support equality for crate-level validation.

### Registry backend

This design intentionally does not require choosing the concrete registry
backend yet.

Any backend is acceptable if it provides:

- distributed registration from many macro expansion sites
- access from normal host-side Rust code
- compatibility with future reuse for ABI/spec collection

Examples include a linker-section/distributed-slice approach or an equivalent
generated static registry mechanism.

The semantic design should not depend on which backend is chosen.

## Collection Rules

When the crate-global mock collector runs for package `P`:

1. Gather all registry fragments with `package_name == P`.
2. Partition fragments by `self_type_key`.
3. Ignore `None` keys when enforcing the one-contract-type rule.
4. If the set of non-`None` keys has more than one distinct member, fail.
5. Register every fragment, including free-function fragments, into one
   `MockContract`.

This means free functions participate in the mock contract even if the crate
also has one impl-backed contract type.

## Generated APIs

### New crate-global mock helper

Expose one SDK helper that builds the complete mock for a package:

```rust
pub fn register_mock_package(
    env: &TestEnvironmentHandle,
    package_name: &str,
) -> MockContract
```

Behavior:

- creates one `MockContract`
- collects all fragments for `package_name`
- validates the one-impl-backed-type rule
- registers every fragment into the mock
- returns the prepared mock contract

This is the primitive API used by tests and by later higher-level generated
helpers.

### Convenience macro/helper at callsite

Tests in a contract crate should be able to write:

```rust
let contract = sdk::register_mock_package(&env, env!("CARGO_PKG_NAME"));
```

This avoids naming a particular impl type and lines up with the "crate =
contract" model.

### Existing generated `__mock` functions

Keep the existing per-fragment generated `__mock` functions for now:

- `<Type as Trait>::__mock`
- `Type::__mock`

Reasons:

- preserves backward compatibility during migration
- lets tests opt into the old API temporarily
- provides a useful implementation building block while the new collector is
  introduced

The new crate-global collector will call the same per-fragment registration
logic internally.

## Codegen Changes

### Trait impls

For `#[contract] impl Trait for Type`:

- continue generating the per-fragment `__mock`
- additionally emit one crate-global mock fragment registration
- set `self_type_key = Some(<Type-key>)`

### Bare impls

For `#[contract] impl Type`:

- continue generating `Type::__mock`
- additionally emit one crate-global mock fragment registration
- set `self_type_key = Some(<Type-key>)`

### Free functions

For `#[contract] fn foo(...)`:

- emit one crate-global mock fragment registration
- set `self_type_key = None`

Free functions have no self type, but they still contribute exported
entrypoints and therefore belong in the crate-global mock contract.

### Trait definitions

For `#[contract] trait Trait`:

- do not emit crate-global mock registration

Traits are interface definitions, not local export declarations.

## Error Handling and Diagnostics

If the collector finds more than one impl-backed contract self type for the
same package, `register_mock_package()` should panic with a direct error
describing the rule:

> A contract crate may export methods from at most one impl-backed contract
> type. Free-function exports are allowed, but impl-backed exports in this
> crate were found for both `Alpha` and `Beta`.

If no fragments are found for the package, `register_mock_package()` should
panic with a direct error:

> No local `#[contract]` exports were registered for package `my_crate`.

This catches incorrect calls to the collector and empty/non-contract crates.

## Testing

Add tests covering:

- single-trait impl crate: one call to crate-global registration works
- multi-trait single-type crate: one call replaces manual chaining
- bare-impl crate: crate-global registration works
- free-function-only crate: crate-global registration works
- mixed free-functions plus one impl-backed type: crate-global registration
  works
- crate with two different impl self types: registration fails with the
  expected error

Target existing examples where possible:

- `example-contracts/stateless` for multi-trait single-type registration
- `example-contracts/erc20_low_level` for free-function exports
- existing bare-impl examples for bare-impl coverage

## Relationship to Future Contract Spec Export

This mock registration registry is intentionally the first slice of a larger
architecture.

Later, the same export fragment model can be extended to support:

- crate-global ABI/spec collection
- interface grouping
- event/type reflection
- JSON export
- XDR export
- wasm custom-section embedding

The important invariant established here is that the SDK can discover all local
exported contract fragments for a crate without requiring the user to manually
list them.
