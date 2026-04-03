# Reserved Client Method Names Design

## Summary

Add a single codegen-side validation pass that rejects exported contract
methods whose names would collide with generated client helpers.

The validator applies to every `#[contract]` surface:

- traits
- trait impls
- bare impls
- free functions

This turns confusing duplicate-item compiler errors into targeted macro errors
at the offending method.

## Motivation

Generated clients currently reserve several method names implicitly:

- `new`, for the generated client constructor
- `with_gas_limit`, for the generated client config helper
- `try_<method>`, for every generated inspection wrapper

Today, if a contract entrypoint uses one of those names, the user gets a later
Rust error from duplicate inherent items or generated helper conflicts. That is
the wrong failure mode for the normal contract-authoring path.

The SDK should reject these names directly during macro expansion and explain
why.

## Goals

- Reject reserved client-helper names with a targeted macro diagnostic.
- Keep the policy in one helper so the reserved-name set is easy to maintain.
- Apply the same validation consistently across all `#[contract]` entrypoint
  surfaces.
- Preserve the current generated client ergonomics:
  `FooClient::new(&addr)`, `client.with_gas_limit(...)`, `client.method(...)`,
  and `client.try_method(...)`.

## Non-Goals

- Redesign the generated client API to free these names.
- Add user-configurable aliases for generated helpers.
- Change the generated helper names.
- Relax the rule for free functions; even though they do not generate clients
  directly, they still define callable contract entrypoints and should follow
  the same naming rules.

## Reserved Names Policy

The validator rejects:

- the exact method name `new`
- the exact method name `with_gas_limit`
- any method name beginning with `try_`

Rationale:

- `new` collides with the generated client constructor
- `with_gas_limit` collides with the generated client configuration helper
- `try_` is reserved because every exported method generates a corresponding
  `try_<method>` client method

Examples:

- `fn new(...)` is rejected
- `fn with_gas_limit(...)` is rejected
- `fn try_transfer(...)` is rejected
- `fn with_gas_limit_extra(...)` is allowed
- `fn retry_transfer(...)` is allowed

## Validation Placement

Add one shared helper in codegen, conceptually:

```rust
fn validate_reserved_client_method_names(
    methods: impl IntoIterator<Item = impl Method>,
) -> syn::Result<()>
```

The helper should:

1. inspect each exported method name
2. classify it against the reserved-name policy
3. return a `syn::Error` on the offending method span

The helper should be invoked from the shared generation entry paths, before
client or export code is emitted, so the policy is enforced uniformly for:

- trait definitions
- trait impls
- bare impls
- free-function bundles

The policy logic should live in exactly one place. If the reserved-name set
changes later, there should be a single helper to update.

## Diagnostics

Diagnostics should be explicit about both the reserved name and the generated
helper it would conflict with.

Recommended messages:

- for `new`:
  - `` `new` is reserved by the generated client constructor; rename the contract method ``
- for `with_gas_limit`:
  - `` `with_gas_limit` is reserved by the generated client gas-limit helper; rename the contract method ``
- for `try_*`:
  - `` method names beginning with `try_` are reserved by generated client inspection methods ``

These errors should be emitted on the offending method or free function span,
not as a later duplicate-item error in generated code.

## Surface-Specific Behavior

### Traits

Reject reserved names on trait methods, since the trait defines the generated
client surface.

### Trait impls

Reject reserved names on impl methods, even though the trait is the authoritative
client surface. This keeps the exported implementation surface aligned with the
same contract-entrypoint naming policy and avoids inconsistent trait/impl pairs.

### Bare impls

Reject reserved names on inherent contract methods, since these generate the
client directly.

### Free Functions

Reject reserved names on `#[contract] fn` as well. Free functions do not
generate a client type themselves, but they still define exported contract
entrypoints and should obey the same naming policy as other contract surfaces.

## Testing

Add compile-fail coverage for:

- a trait method named `new`
- a bare-impl method named `with_gas_limit`
- a method named `try_transfer`
- a free `#[contract] fn try_ping()`

Each test should assert the targeted macro diagnostic rather than a generic
duplicate-item compiler failure.

Add one compile-pass control case to confirm nearby names still work:

- `with_gas_limit_extra`
- `retry_transfer`

## Documentation

Update the contract macro docs to state that generated clients reserve:

- `new`
- `with_gas_limit`
- the `try_` prefix

Document this near the generated client behavior section, since that is where
the naming rule makes sense to contract authors.

## Implementation Notes

- This is a validation-only change.
- No client API redesign is part of this work.
- No backwards-compatibility accommodations are necessary for the current stage
  of the SDK.
