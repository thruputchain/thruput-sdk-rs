# Re-design: SDK support for cross-contract errors

Status: proposal.

## Problem

`#[contract(client = C, throws(A, B))]` generates a per-client error enum
`CError` with one variant per declared throw type, plus `Library`, `Fault`,
`Unknown`, `BadDecode`, `Invalid`, and four encode/decode variants
(`crates/macros/src/common/client.rs`, `generate_error_enum`). `try_*`
methods return `Result<T, CError>`.

Pre-downcasting into that enum bakes one interface's `throws` list into the
caller's error type. An extension error that a base interface does not (and
should not) declare therefore has no variant, and must be dug out of
`Unknown`. That is the `ERC20PausableError::EnforcedPause` case: it is
deliberately absent from `ERC20`'s `throws` contract, mirroring
OpenZeppelin's `IERC20`, so an `ERC20Client` caller writes

```rust
// example-contracts/erc20_pausable/tests/test.rs:172
Err(ERC20ClientError::Unknown(raw)) => {
    assert_eq!(
        raw.try_as::<ERC20PausableError>(),
        Some(ERC20PausableError::EnforcedPause),
    );
}
```

Adding `try_as::<E>()` directly to the enum removes the `Unknown` detour but
does not fix the real problem: handling a base-interface error and an
extension error *in one place* still needs a cascade of `if let`, because the
two live at different levels of the type.

The underlying observation is that the enum is redundant. A Groundhog
exception is already self-describing on the wire — `[CXS(selector)][payload]`
— and the selector identifies the type independent of any interface's
`throws` list. The enum re-encodes a subset of that information into Rust
variants, and in doing so loses the open-world property the wire format has.

## Proposal

Drop the generated enum. `try_*` returns the SDK's existing untyped call error,
and typed matching can be optionally facilitated with a new `match_exn!` macro.

### 1. `try_*` returns `CallError`

`crates/sdk/src/call/raw_call_error.rs` already defines

```rust
pub enum RawCallError<X = RawException> {
    Invalid,
    Exception(X),
    Fault(FaultString),
    UnparseableReturn(FaultString),
    UnparseableAbort(FaultString),
    UnparseableAuthHandles(FaultString),
    EncodingArgs(FaultString),
}
```

with a `ReThrow` impl. Every non-exception variant of the generated enum is
already here; the generated enum's `Library`, `Unknown`, `BadDecode` and
declared variants all collapse into `Exception(RawException)`. So the change
is not "invent a new error type", it is "stop instantiating `X` as a
generated enum":

```rust
pub type CallError = RawCallError;   // alongside the existing `CallFault`
```

and the generated `try_*` body's `on_exn` closure becomes `|exn|
exn.to_owned()` — the same closure for every method on every client.

### 2. `match_exn!`

```rust
match token.try_transfer(&to, amount) {
    Ok(()) => { /* ... */ }
    Err(e) => match_exn!(e, {
        ERC20Error::InsufficientBalance   => { /* ... */ }
        ERC20PausableError::EnforcedPause => { /* ... */ }
        LibraryExn::NonNegativeIntOverflow => { /* ... */ }
        _ => e.re_throw(),
    }),
}
```

The division of labour is the point: everything structural (`Ok`, `Fault`,
`Invalid`, the encode/decode failures) stays an ordinary Rust `match`, with
real exhaustiveness checking, and the macro's only job is typed downcasting.
A caller that wants to treat a fault specially just adds an arm to the outer
match, before the catch-all:

```rust
match token.try_transfer(&to, amount) {
    Ok(()) => { /* ... */ }
    Err(CallError::Fault(f)) => { /* ... */ }
    Err(e) => match_exn!(e, { /* ... */ }),
}
```

Base-interface and extension errors now sit side by side as peers. Neither
is privileged, and neither depends on what the interface declared.

A final `_` arm is **required**; omitting it is a `compile_error!`. The
macro's arms are tried in sequence rather than checked for exhaustiveness,
so there is no way to prove the typed arms cover everything — and an
exception whose selector matches nothing is the normal case, not an edge
one. Making the author spell out what happens then keeps the control flow
visible. Propagating is the explicit `_ => e.re_throw()`, not a silent
default.

### 3. How it lowers

Each arm becomes an `if let` over `try_as`, tried in order, first match
wins, inside a labelled block:

```rust
'match_exn: {
    let __scrutinee = &e;
    let __exn = MatchExn::as_exn(__scrutinee);
    if let Some(ERC20Error::InsufficientBalance) = __try_as(__exn) {
        break 'match_exn ({ /* ... */ });
    }
    if let Some(ERC20PausableError::EnforcedPause) = __try_as(__exn) {
        break 'match_exn ({ /* ... */ });
    }
    break 'match_exn ({ e.re_throw() })
}
```

The macro never needs to name the exception type: **the arm's pattern pins
`T` in `try_as::<T>()` by ordinary type inference**, because a path pattern
like `ERC20Error::InsufficientBalance` unifies the scrutinee type with
`ERC20Error`. This is what makes the syntax work without type annotations,
turbofish, or a `Type => { variants }` grouping level.

`&$e` rather than `$e` matters: it extends the lifetime of a temporary
scrutinee (`match_exn!(client.try_foo().unwrap_err(), {...})`), and leaves a
named scrutinee merely borrowed, so `e` is still usable inside `_`.

This is plain `macro_rules!`, so it lives in `sdk` and costs no proc-macro
expansion. A working prototype of the lowering compiles and passes on
edition 2024; validated behaviours:

| Behaviour | Works |
|---|---|
| Type inferred from the arm pattern | yes |
| Struct-variant bindings (`Err { have }`) | yes |
| Guards (`Pat if cond =>`), falling through to later arms on guard failure | yes |
| Top-level alternates within one type (`A::X \| A::Y`) | yes |
| `v @ Pat` whole-value bindings | yes |
| Unit-struct exceptions (`Unauthorized =>`) | yes |
| Expression position, non-`()` value | yes |
| `return` / `?` escaping from an arm body | yes |
| Non-exception `CallError`s (`Fault`, `Invalid`) fall to `_` | yes |
| Unrecognised selector falls to `_` | yes |
| Temporary scrutinee | yes (via `&$e`) |
| Scrutinee still usable in `_` | yes |

Two mechanical constraints follow from inference and should be documented as
such, not treated as style choices:

- **Every typed arm must be path-rooted** (`Type::Variant`, `Type::Variant {
  .. }`, or a unit-struct `Type`). A bare identifier is a binding, so `T`
  would be unconstrained and inference fails. The untyped escape hatch is
  `_`, where the scrutinee is in scope.
- **`|` cannot cross types.** `ERC20Error::A | ERC20PausableError::B` is one
  `try_as` with two `T`s. Use two arms.

### 4. `MatchExn`

A small trait decides what `match_exn!` accepts:

```rust
pub trait MatchExn { fn as_exn(&self) -> Option<RawExceptionRef<'_>>; }
```

implemented for `CallError` (`Some` only for `Exception`), `RawException`
(via its existing private `as_ref_exn`, made `pub(crate)`), and
`RawExceptionRef<'_>` (a reborrow — its fields are `Copy`). Returning by
value, not by reference, keeps all three impls trivial. The last one means
advanced callers can match inside `try_call_raw_with`'s `on_exn` closure
against the borrowed scratch — zero-copy, no `to_owned` at all.

## What this deletes

- `generate_error_enum` and `generate_selector_dispatch` in
  `crates/macros/src/common/client.rs`, plus the per-method
  `Display`/`ReThrow` impls they emit.
- `generate_selector_uniqueness_asserts` — the O(n²) const-assert pass
  exists only to stop two `throws` entries producing duplicate match arms in
  the selector dispatch. With no dispatch to generate, it has nothing to
  protect.
- `ThrowsList::new`'s reserved-name check (`Library`, `Fault`, `Unknown`)
  and duplicate-variant-name check in `crates/macros/src/common/config.rs`,
  and `ThrownType::variant()` — with no enum, a `throws` entry is a type,
  not a variant name. `throws = alpha::E` and `throws = beta::E` stop
  colliding.

`throws` itself stays. It independently feeds the contract spec via
`throws_spec_tokens` and spec canonicalization, and remains the interface's
documented error contract.

## Costs

**Size: unchanged.** `RawException` carries a fixed `SCRATCH_MAX` inline
buffer without `alloc` (~4 KiB), but the generated enum already embeds one
in `Unknown`/`BadDecode`, so today's `Result<T, CError>` is already that
size. `#[allow(clippy::result_large_err)]` is still needed, exactly as now.

**Copying: slightly more.** Today a *declared* exception decodes straight
out of the scratch without `to_owned`; only `Unknown`/`BadDecode` copy.
Under this design every caught exception copies its wire bytes once. The
copy is payload-sized (`extend_from_slice(wire)`), not 4 KiB — the 4 KiB is
capacity, and the stack slot exists either way. Callers that care can use
the `RawExceptionRef` path in §4 and copy nothing.

**Decode work: less.** Today the selector dispatch eagerly decodes the
declared type whether or not the caller inspects it; a caller that only
re-throws pays for a decode it discards. Here decoding is on demand.

**Code size: expected smaller, unmeasured.** Every `try_*` method currently
monomorphises a selector-dispatch match with one decode arm per declared
type. That collapses to one shared `to_owned` closure.

**`BadDecode` stops being a distinct variant.** `try_as` returns `None`
both for "wrong selector" and "right selector, undecodable payload", so the
two collapse into `_`. This is deliberate: `match_exn!` users do not need
the distinction, and it remains available by hand — comparing
`raw.selector()` against `E::SELECTOR` in the `_` arm separates the two
cases without any new API.

**Repeated decodes across arms.** Two arms naming the same exception type
each run their own `try_as`, so the payload decodes twice. Selector
comparison guards each attempt, so unrelated types cost nothing, and the
duplicate work only occurs on an error path that already matched. If it
ever matters, the macro can fold syntactically-adjacent arms sharing a path
root into a single `try_as` plus an inner `match`; that is a pure
optimisation, invisible in the source, and can be added later without
changing any call site.

**Loss of a compile-time nudge.** Today, adding a type to a `throws` list
adds an enum variant, so callers that `match` exhaustively must update.
Under this design, `throws` changes are invisible to caller code. That
decoupling is exactly the point for the pausable case — but it does mean
nothing tells a caller a new error exists, and nothing checks that a
`match_exn!` arm names a type the callee can actually throw. A lint that
cross-checks arms against the callee's spec is conceivable later; it is not
part of this proposal.

## Backward-compatible route

The two changes are separable, and `match_exn!` can ship first without
breaking anything:

1. Add `match_exn!` and `MatchExn`, and implement `MatchExn` for the
   *generated* client error enums as well. Keep the enums.
2. Migrate call sites to `match_exn!` incrementally.
3. Later, flip `try_*` to `Result<T, CallError>` and delete the enums.

Because the outer `match` handles structure and the macro handles types,
step 3 becomes mostly *deletion* at each call site — the `match_exn!` block
is unchanged; the arms above it that named `CError::Unknown(..)` or
`CError::Library(..)` go away. That is the main argument for this shape over
one where the macro swallows `Ok`/`Err` too.

The wrinkle is step 1's `try_as` on a generated enum. For an *undecoded*
payload (`Unknown`/`BadDecode`) it is the existing `RawException::try_as`.
For a *declared* variant, the value is already decoded as `Ty`, and Rust
cannot see that `T == Ty` even though `T::SELECTOR == Ty::SELECTOR` proves
it. Options:

- **(a) Re-encode and decode.** `WriteXDR` + `ReadXDROwned` are both
  available; the cost lands only on error paths, only during the migration
  window. Recommended — it keeps `match_exn!` source-compatible across the
  flip.
- **(b) `core::any::Any` downcast.** Cheaper, but adds `T: 'static + Clone`
  bounds to a method that otherwise needs neither.
- **(c) Cover only `Unknown`/`BadDecode`.** Simplest, but during migration
  declared errors still match through the outer enum, so the syntax is not
  uniform and step 3 is a rewrite rather than a deletion.

## Migration surface

`*ClientError` appears ~60 times across 17 files: `example-contracts/`
(counter, multi_throws, exceptions, storage_errors, erc20_pausable,
erc1271_caller, imported_error_*, versioned_callee, named_interface_callee,
invoke_flags_test), `standard-tests/` (erc20-test, erc20-native-test),
`crates/sdk/tests/compile-pass/throws_paths.rs`, and
`crates/sdk/src/call/mod.rs`. The developers-site page
`reference/error-handling.md` ("Cross-Contract Errors", "Undeclared errors")
and `reference/cross-contract-calls.md` both need rewriting.

New tests worth adding: `compile-fail` cases for a non-path-rooted arm
(should say "use `_`"), a missing `_` arm, and cross-type `|`; `compile-pass`
for guards, `@` bindings, and expression position.
