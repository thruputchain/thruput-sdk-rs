# Error Handling Ergonomics

Three improvements to the SDK's error handling story: an `or_throw` extension trait for cross-contract calls, a `__private` thiserror re-export to eliminate a transitive dependency, and struct support in `#[contract_error]`.

## 1. `OrThrow` trait

An extension trait on `Result` that replaces the error with a contract exception.

### API

```rust
// src/throw.rs

pub trait OrThrow<T> {
    fn or_throw<E: Exn>(self, exn: E) -> T;
}

impl<T, Err> OrThrow<T> for Result<T, Err> {
    #[cfg_attr(feature = "debugging", track_caller)]
    fn or_throw<E: Exn>(self, exn: E) -> T {
        match self {
            Ok(v) => v,
            Err(_) => throw(exn),
        }
    }
}
```

Re-exported from `lib.rs` via the existing `pub use throw::*`.

### Usage

```rust
// Before:
match fib.try_compute(n) {
    Ok(result) => result,
    Err(_) => throw(CrossCallError::FibCallFailed),
}

// After:
fib.try_compute(n).or_throw(CrossCallError::FibCallFailed)
```

## 2. `ReThrow` trait and `OrReThrow`

A trait for types that can re-throw themselves as contract exceptions, plus an extension trait on `Result` for transparent error propagation.

### API

```rust
// src/re_throw.rs (new file)

pub trait ReThrow {
    fn re_throw(self) -> !;
}

pub trait OrReThrow<T> {
    fn or_re_throw(self) -> T;
}

impl<T, E: ReThrow> OrReThrow<T> for Result<T, E> {
    fn or_re_throw(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => e.re_throw(),
        }
    }
}
```

### SDK-provided `ReThrow` impls

`RawException` already has a `re_throw(&self)` method. The `ReThrow` trait formalizes this; `RawException` implements it by delegating to its existing method. `RawCallError` dispatches to either `RawException::re_throw` or `panic!`.

```rust
// src/exn/raw_error.rs
impl ReThrow for RawException {
    fn re_throw(self) -> ! {
        // delegates to existing RawException::re_throw(&self)
        RawException::re_throw(&self)
    }
}

// src/call/call_result.rs
impl ReThrow for RawCallError {
    fn re_throw(self) -> ! {
        match self {
            RawCallError::Exception(exn) => exn.re_throw(),
            RawCallError::Fault(f) => panic!("{f}"),
        }
    }
}
```

Semantics:
- **Exception** — re-serializes the original selector+payload back across the boundary.
- **Fault** — panics. A fault is not a structured exception; rethrowing it stays a fault.

### Generated `ReThrow` for client error enums

The code generator emits `impl ReThrow for ClientError` for each generated client:

```rust
impl ReThrow for FibClientError {
    fn re_throw(self) -> ! {
        match self {
            Self::FibError(e) => ::sdk::throw(e),         // typed Exn
            Self::Library(e) => ::sdk::throw(e),           // LibraryExn
            Self::Unknown(raw) => raw.re_throw(),          // RawException
            Self::Fault(f) => panic!("{f}"),               // fault stays fault
        }
    }
}
```

### Usage

```rust
// Transparent propagation — rethrow whatever the callee threw:
fib.try_compute(n).or_re_throw()
```

### Code generator simplification

The generated non-try client methods currently match on `RawCallError` manually:

```rust
// Before:
match ::sdk::call::try_call_raw::<_, ReturnType>(&self.address, ep, gas, &args) {
    Ok(val) => val,
    Err(::sdk::call::RawCallError::Exception(ref exn)) => exn.re_throw(),
    Err(::sdk::call::RawCallError::Fault(f)) => panic!("{}", f),
}

// After:
::sdk::call::try_call_raw::<_, ReturnType>(&self.address, ep, gas, &args).or_re_throw()
```

## 3. `__private` thiserror re-export

The `#[contract_error]` macro generates `#[derive(thiserror::Error)]` under `cfg(feature = "dev")`, forcing every contract crate to list `thiserror` in its `Cargo.toml`. The SDK already depends on `thiserror` unconditionally.

### Change

Add to `src/lib.rs`:

```rust
#[doc(hidden)]
pub mod __private {
    pub use ::thiserror;
}
```

Change `macros/src/contract_error.rs` to emit `::sdk::__private::thiserror::Error` instead of bare `thiserror::Error`:

```rust
// Before:
#[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, thiserror::Error)]

// After:
#[derive(Debug, Clone, PartialEq, ::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::__private::thiserror::Error)]
```

Contracts drop `thiserror` from their `Cargo.toml`.

## 4. `#[contract_error]` struct support

Currently `#[contract_error]` only accepts enums. The `Exn` trait bounds (`Debug + PartialEq + WriteXDR + ReadXDROwned`) are not enum-specific. The `#[exn]` attribute macro already handles structs for selector generation; `#[contract_error]` should too.

### Macro changes

Branch on `Item::Enum` vs `Item::Struct` in `macros/src/contract_error.rs`:

**Enums (unchanged):**
- `#[repr(i32)]`
- Strip `#[error(...)]` from variants in non-dev builds
- Full derive set including conditional `thiserror::Error`

**Structs:**
- No `#[repr(i32)]`
- Strip `#[error(...)]` from struct-level and field-level attrs in non-dev builds
- Same derive set otherwise
- Same `impl Exn` with selector

### `strip_error_attrs` adjustment

Current function operates on `ItemEnum` variants. Add a parallel path for `ItemStruct`:
- Strip `#[error(...)]` from the struct's own attrs
- Strip `#[source]`, `#[from]` from field attrs (thiserror field attributes)

### Usage

```rust
#[contract_error]
#[error("not authorized")]
pub struct Unauthorized;

#[contract_error]
#[error("insufficient balance: {balance}")]
pub struct InsufficientBalance {
    pub balance: u64,
}
```

## Files touched

| File | Change |
|------|--------|
| `src/throw.rs` | Add `OrThrow` trait + impl |
| `src/re_throw.rs` | New file: `ReThrow` trait, `OrReThrow` trait + impl |
| `src/lib.rs` | Add `mod re_throw`, `pub use re_throw::*`, add `__private` module |
| `src/call/call_result.rs` | Add `impl ReThrow for RawCallError` |
| `macros/src/contract_error.rs` | Struct support, `thiserror` path change |
| `gen/src/client.rs` | Emit `impl ReThrow for ClientError`, simplify non-try methods |
| Example contract `Cargo.toml` files | Remove `thiserror` dependency |
