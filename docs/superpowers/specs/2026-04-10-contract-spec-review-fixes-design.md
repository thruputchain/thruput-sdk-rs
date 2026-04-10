# Contract Spec Review Fixes Design

## Goal

Land two narrowly scoped review fixes in the contract-spec pipeline without reopening alloc diagnostics, symbol identity, or entrypoint export behavior.

## Scope

In scope:

- accept `-2147483648` in `#[contract_error]` discriminants
- reject conflicting typedefs only after reachable-type resolution in the spec tool

Out of scope:

- alloc-diagnostic redesign
- typedef symbol identity redesign
- entrypoint version semantics
- new end-to-end fixture crates

## Fix 1: `#[contract_error]` Must Accept `i32::MIN`

### Problem

`#[contract_error]` currently parses negative discriminants by parsing the positive integer literal as `i32` and then negating it. That fails for `-2147483648`, even though it is a valid `i32`.

### Design

Keep the change local to `macros/src/contract_error.rs`.

For unary-negated integer literals:

- parse the literal body as `i64`
- apply checked negation in `i64`
- convert the result to `i32` with `try_from`

For all other cases:

- keep requiring literal integer discriminants
- keep rejecting out-of-range values with the existing `i32`-fit diagnostic

### Expected Behavior

- `-2147483648` is accepted
- `-2147483649` is rejected
- non-literal discriminants are still rejected

## Fix 2: Typedef Conflicts Should Be Reachability-Scoped

### Problem

The spec tool currently groups raw typedef fragments by symbol and rejects duplicate symbols before it knows which types are reachable from exported interfaces, methods, and throws. That makes unreachable collisions fail finalization even when they would not appear in the final `ContractSpec`.

### Design

Keep raw fragment collection unchanged in `groundhog-spec-tool/src/lib.rs`.

Change conflict handling so that:

- raw scan collects all defs per symbol
- canonical interface and method assembly proceeds unchanged
- reachable named types are resolved from interfaces, methods, throws, and recursively referenced typedef fields/cases
- duplicate/conflict checks happen only when a symbol is actually pulled into the reachable set

Resolution rules for a reachable symbol:

- zero defs: `MissingTypeDef`
- one unique def: accept it
- repeated identical defs: accept them
- multiple distinct defs: `ConflictingTypeDef`

### Expected Behavior

- reachable conflicting typedefs still fail
- unreachable conflicting typedefs are ignored
- missing reachable typedefs still fail

## Error Surface

No public macro API or spec-tool API changes are required.

The only user-visible behavior changes are:

- `#[contract_error]` now accepts the valid `i32::MIN` boundary
- spec finalization ignores unreachable typedef conflicts while preserving existing reachable conflict and missing-type errors

## Verification

### Macro crate

- add a focused unit test for parsing `-2147483648`

### Spec tool

- keep the existing reachable-conflict test expecting `ConflictingTypeDef`
- change the unreachable-conflict test to expect successful canonicalization with no emitted typedefs
- keep the missing-typedef test unchanged

## Notes

This is intentionally a review-fix patch, not a cleanup pass. It should not change alloc diagnostics, named-type identity, or interface/export semantics.
