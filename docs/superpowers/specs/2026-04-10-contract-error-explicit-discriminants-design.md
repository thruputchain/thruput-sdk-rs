# Contract Error Explicit Discriminants Design

## Goal

Stop parsing integer expressions inside the `#[contract_error]` proc macro. Instead, require explicit discriminants on contract-exported enums and let generated Rust code carry those discriminant expressions into the emitted contract spec.

## Scope

Immediate scope:

- `#[contract_error]` enums only
- enum variants used in contract-spec generation
- removal of proc-macro discriminant parsing for that path

Out of scope:

- changing non-enum `#[contract_error]` handling
- changing alloc diagnostics
- changing typedef identity rules
- changing spec finalization behavior beyond whatever is needed to consume generated discriminant expressions

## Background

The current `#[contract_error]` implementation computes enum discriminants in the proc macro:

- `enum_discriminants()` walks variants
- explicit discriminants are parsed with `parse_i32_expr()`
- implicit discriminants are synthesized with `next + 1`

This creates unnecessary complexity:

- it reimplements part of Rust const evaluation in the macro
- it creates edge cases like `i32::MIN`
- it encourages us to keep expanding the proc-macro parser whenever Rust syntax gets more interesting

At the same time, the XDR derive layer already requires `#[repr(i32)]` for enums and emits Rust code that writes discriminant expressions directly. `#[contract_error]` should follow the same general model instead of pre-evaluating integer syntax in the proc macro.

## Policy Change

Contract-exported enums handled by `#[contract_error]` must use explicit discriminants for every variant.

That means:

- every enum variant must have `= <expr>`
- implicit auto-incremented discriminants are rejected
- explicit discriminants may be const-evaluable Rust expressions

Examples:

Allowed:

```rust
#[contract_error]
enum Error {
    NotFound = 1,
    BadState = -7,
    Limit = i32::MAX,
}
```

Rejected:

```rust
#[contract_error]
enum Error {
    NotFound = 1,
    BadState,
}
```

## Design

### 1. Remove proc-macro discriminant evaluation

Delete the current proc-macro-side discriminant computation path:

- `enum_discriminants()`
- `parse_i32_expr()`

The macro should no longer try to interpret integer syntax itself.

### 2. Require explicit discriminants during macro expansion

When generating the enum spec:

- if any variant is missing a discriminant, emit a macro compile error
- the error should be direct:
  - `#[contract_error] enum variants must use explicit discriminants for contract specs`

This rule applies equally to:

- plain fieldless enums
- payload enums that become spec unions

### 3. Splice user discriminant expressions directly into generated Rust

For each variant:

- read the `syn::Expr` from the variant discriminant
- copy that expression directly into the generated spec tokens

So instead of generating:

```rust
value: 7
```

after proc-macro parsing, the macro may generate:

```rust
value: SOME_CONST_EXPR
```

or:

```rust
discriminant: -2147483648
```

and let Rust type-check and const-evaluate it.

This works because:

- `#[contract_error]` already imposes `#[repr(i32)]` on enum expansions
- the generated contract-spec view is emitted as Rust const data
- Rust is better positioned than the proc macro to validate integer expressions and overflow

### 4. Keep current enum shape rules

This redesign does not change current enum-shape restrictions:

- fieldless enums still become `ContractTypeDefView::Enum`
- payload enums still become `ContractTypeDefView::Union`
- variants with multiple payload fields remain unsupported in contract specs

Only discriminant handling changes.

## Error Behavior

New primary error:

- missing discriminant on any `#[contract_error]` enum variant:
  - `#[contract_error] enum variants must use explicit discriminants for contract specs`

For explicit expressions:

- Rust handles integer typing and overflow through the generated code
- the proc macro no longer needs custom integer-literal parsing errors

## Why Not `mem::discriminant`

`std::mem::discriminant` is not the right mechanism here.

It gives an opaque `Discriminant<T>`, not the numeric `i32` value needed in the spec. The standard library docs explicitly warn that converting `Discriminant<T>` to a primitive is not the supported path. Numeric extraction for primitive-repr enums is done through layout/pointer access on an actual value, but the proc macro does not have a constructed enum value to inspect.

By contrast, copying the discriminant expression into generated Rust:

- requires no unsafe code
- does not require constructing enum values
- lets Rust do const evaluation directly

## Testing

### Macro tests

Add tests for:

- explicit negative boundary values such as `-2147483648`
- rejection of a missing discriminant on a `#[contract_error]` enum

### Compile-fail coverage

Add a compile-fail fixture for:

- a `#[contract_error]` enum with one explicit discriminant and one implicit variant

Expected stderr should point at the missing-discriminant variant and use the new explicit-discriminant message.

## Migration Notes

This is a source-level breaking change for `#[contract_error]` enums that currently rely on implicit discriminants.

Migration is straightforward:

- add explicit `= ...` discriminants to every variant

Because these are exported contract error values, requiring explicit tags is acceptable and preferable for ABI clarity.

## Notes

This is intended as the canonical direction for future exported-enum spec generation too: prefer generated-Rust evaluation of explicit discriminant expressions over proc-macro integer parsing.
