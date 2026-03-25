# Selector-Based Error Dispatch

## Problem

When a contract implements multiple traits (or calls into other contracts internally), different error types can appear on the wire. The current system hardcodes a single `throws = ErrorType` per trait and uses `CrossExn<E>` to wrap it with a small i32 discriminant (`Contract=0`, `Library=1`, `Unknown=2`). This means:

1. If a contract method propagates an error from an internal call (e.g., an ERC20 token call fails inside a swap method), the client tries to decode the abort bytes as the wrong type and hits a decode mismatch.
2. There's no way to declare multiple error types on a trait.
3. The client can't match on a rich set of possible errors — it's one type or nothing.

## Design

Replace the current `CrossExn<E>` wire format and static error typing with a **selector-based dispatch** system where every error type is identified by a u64 hash of its canonical signature.

### Wire Format

**Current:**
```
[i32 discriminant: 0=Contract, 1=Library, 2=Unknown][XDR payload][debug info]
```

**New:**
```
[u64 selector][XDR payload][debug info]
```

Every error type — user-defined, `LibraryExn`, all of them — is a peer on the wire, identified by its u64 selector. The `CrossExn<E>` wrapper and its `Inner` enum are removed.

Debug info (location, backtrace) remains gated behind `cfg(feature = "debugging")` and follows the payload as before.

### The `Exn` Trait

The `Exn` trait gains a compile-time selector constant:

```rust
pub trait Exn: Debug + PartialEq + WriteXDR + ReadXDROwned {
    const SELECTOR: u64;
}
```

(With the `debugging` feature, `Exn` additionally requires `Error`, as it does today.)

The current blanket impl (`impl<T> Exn for T where T: Debug + PartialEq + WriteXDR + ReadXDROwned`) is removed — each type must explicitly `#[derive(Exn)]` to provide its selector. This is a breaking change but aligns with the derive-based approach.

`Infallible` gets a manual `Exn` impl with a well-known selector. In practice it should never appear on the wire.

### Selector Derivation

Selectors are computed at compile time via `#[derive(Exn)]`. The derive macro:

1. Builds a **canonical signature string** from the type definition:
   - Enum: `"ERC20Error::InsufficientBalance(u64)::InsufficientAllowance"`
   - Struct: `"MathError(u32)"`
   - Format: `TypeName` followed by `::VariantOrFieldName(field_types)` for each variant/field
2. Computes SHA-256 of the signature string
3. Takes the first 8 bytes as a big-endian u64

The signature intentionally excludes module paths and crate names. This means:
- Moving a type between modules or crates doesn't change its selector (stable)
- Renaming the type, adding/removing variants, or changing field types does change the selector (correct, since the wire format changed — this is a SemVer-breaking change)

Two independent crates defining identically-named types with identical signatures will produce the same selector. With u64 (2^64 space), accidental collisions are effectively impossible.

### `#[derive(Exn)]` Requirements

The derive macro requires:
- The type must already derive `Debug`, `PartialEq`, `WriteXDR`, and `ReadXDROwned`
- Enum types must be `#[repr(i32)]` (existing requirement for XDR serialization)
- With `debugging` feature: must implement `Error` (typically via `#[derive(thiserror::Error)]`)

Example:
```rust
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, Exn, Error)]
#[repr(i32)]
pub enum SwapError {
    #[error("insufficient liquidity")]
    InsufficientLiquidity = 1,
    #[error("slippage exceeded")]
    SlippageExceeded = 2,
}
```

### `LibraryExn`

`LibraryExn` implements `Exn` with a well-known selector (derived from its signature like any other type). The separate `serialize_library_error` function is removed — library errors go through the same unified `serialize_error` path.

### Macro Syntax: Multiple `throws` Types

The `#[contract]` macro's `throws` attribute accepts a list of types:

```rust
#[contract(client = SwapClient, throws(SwapError, ERC20Error))]
pub trait Swap { ... }
```

The existing `throws = SingleError` syntax is kept for backwards compatibility but both forms normalize to a `Vec<syn::Ident>` internally. The `= X` form will be deprecated in a future version.

Config parsing in `gen/src/config.rs` changes:
```rust
// Before
throws: Option<syn::Ident>,

// After
throws: Option<ThrowsList>,  // custom FromMeta impl accepting both forms
```

### Generated Client Error Enum

For a trait with `throws(SwapError, ERC20Error)`, the macro generates:

```rust
pub enum SwapClientError {
    // Declared throws types — variant name = type name
    SwapError(SwapError),
    ERC20Error(ERC20Error),

    // Always present
    Library(LibraryExn),
    Fault(FaultString),
    Unknown(RawError),
}
```

**Variant naming:** The variant name is the type name exactly. No stripping, no transformation.

**Always-present variants:**
- `Library(LibraryExn)` — SDK internal errors, syscall errors
- `Fault(FaultString)` — contract panicked (not an XDR-encoded abort; separate code path from `ContractError::Fault`)
- `Unknown(RawError)` — unrecognized selector; catch-all

**No-throws case:** When no `throws` is declared:
```rust
pub enum MyClientError {
    Library(LibraryExn),
    Fault(FaultString),
    Unknown(RawError),
}
```

**Single-throws case:** `throws(SwapError)` generates the full enum with one declared variant.

**Naming collision:** If two throws types have the same identifier (from different modules), the macro emits a compile error about duplicate variant names.

### `RawError` and `try_as`

```rust
pub struct RawError {
    pub selector: u64,
    payload: heapless::Vec<u8, 500>,  // same bound as AbortBuf
}

impl RawError {
    pub fn try_as<T: Exn>(&self) -> Option<T> {
        if self.selector == T::SELECTOR {
            contract_call_decode(&self.payload).ok()
        } else {
            None
        }
    }

    pub fn selector(&self) -> u64 {
        self.selector
    }
}
```

`RawError` owns its bytes (copied from the borrowed abort buffer). This copy only happens on the unknown-error fallback path.

### SDK Call Plumbing

**New `try_call_raw`** replaces `try_call` as the function generated client methods call:

```rust
pub enum RawCallError<'a> {
    Abort {
        selector: u64,
        payload: &'a [u8],
        #[cfg(feature = "debugging")]
        location: Option<LocationString>,
        #[cfg(feature = "debugging")]
        backtrace: Option<BacktraceString>,
    },
    Fault(FaultString),
}

pub fn try_call_raw<ARGS, RET>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &ARGS,
) -> Result<RET, RawCallError<'_>>
where
    ARGS: WriteXDR,
    RET: ReadXDROwned,
```

`try_call_raw`:
1. Encodes args and invokes the syscall (same as today)
2. On success: decodes `RET` from the return buffer (same as today)
3. On abort: reads the u64 selector and returns a borrowed slice of the remaining payload — **does not attempt to decode the error**
4. On fault: returns `RawCallError::Fault` (same as today)

The old `try_call<ARGS, RET, EXN>` is removed. Error dispatch is the generated client's responsibility.

**Generated `try_` method** does selector dispatch:

```rust
pub fn try_swap(&mut self, ...) -> Result<ReturnType, SwapClientError> {
    match sdk::call::try_call_raw(&self.address, entrypoint, gas, &args) {
        Ok(val) => Ok(val),
        Err(RawCallError::Fault(s)) => Err(SwapClientError::Fault(s)),
        Err(RawCallError::Abort { selector, payload, .. }) => {
            match selector {
                SwapError::SELECTOR => {
                    let err = contract_call_decode(payload).unwrap_or_else(/* abort */);
                    Err(SwapClientError::SwapError(err))
                }
                ERC20Error::SELECTOR => {
                    let err = contract_call_decode(payload).unwrap_or_else(/* abort */);
                    Err(SwapClientError::ERC20Error(err))
                }
                LibraryExn::SELECTOR => {
                    let err = contract_call_decode(payload).unwrap_or_else(/* abort */);
                    Err(SwapClientError::Library(err))
                }
                _ => Err(SwapClientError::Unknown(
                    RawError::new(selector, payload)
                )),
            }
        }
    }
}
```

**Generated non-try method** (the one that aborts on error) unwraps through the new enum, re-serializing the error via `serialize_error`.

### Serialization (Contract Side)

The two current functions `serialize_contract_error` and `serialize_library_error` are unified into one:

```rust
pub fn serialize_error(err: &impl Exn) -> ! {
    use crate::call::internal::contract_call_encode;

    #[cfg(not(feature = "debugging"))]
    {
        let buf = contract_call_encode(&(err.SELECTOR, err))
            .unwrap_or_else(|e| encoding_abort_abort(e));
        crate::contract_abort(buf.as_ref());
    }

    #[cfg(feature = "debugging")]
    {
        let location: Option<LocationString> = Some(format(Location::caller()));
        let backtrace: Option<BacktraceString> = Some(format(&Backtrace::capture()));
        let buf = contract_call_encode(&(err.SELECTOR, err, &location, &backtrace))
            .unwrap_or_else(|e| encoding_abort_abort(e));
        crate::contract_abort(buf.as_ref());
    }
}
```

The public `abort()` function calls `serialize_error` — its signature changes to require `Exn` (which includes the selector) rather than just `WriteXDR`.

### User-Facing Ergonomics

**Contract author:**
```rust
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, Exn, Error)]
#[repr(i32)]
pub enum SwapError {
    #[error("insufficient liquidity")]
    InsufficientLiquidity = 1,
}

#[contract(client = SwapClient, throws(SwapError, ERC20Error))]
pub trait Swap {
    fn swap(&mut self, amount: u64) -> u64;
}
```

**Client caller:**
```rust
let mut client = SwapClient::new(&address);

match client.try_swap(100) {
    Ok(val) => { /* success */ }
    Err(SwapClientError::SwapError(e)) => { /* handle swap error */ }
    Err(SwapClientError::ERC20Error(e)) => { /* handle token error */ }
    Err(SwapClientError::Library(LibraryExn::Syscall(s))) => { /* syscall */ }
    Err(SwapClientError::Unknown(raw)) => {
        // Escape hatch: try decoding as a known type
        if let Some(math_err) = raw.try_as::<InternalMathError>() {
            // handle
        }
    }
    Err(SwapClientError::Fault(f)) => { /* contract panicked */ }
}
```

## What Gets Removed

- `CrossExn<E>` struct and its `Inner` enum
- `INNER_CONTRACT`, `INNER_LIBRARY`, `INNER_UNKNOWN` constants
- `serialize_contract_error` and `serialize_library_error` (replaced by `serialize_error`)
- `try_call<ARGS, RET, EXN>` (replaced by `try_call_raw`)
- `CallError<E>` enum (replaced by generated per-client error enums)
- `CallResult<T, E>` type alias (replaced by per-client `Result` types)
- `CrossExn::from_call_err` and related conversion methods

## What Stays

- `abort()` public API (signature changes to require `Exn`)
- `ContractError::Abort(AbortBuf)` / `ContractError::Fault(FaultString)` — the raw syscall-level result type
- `LibraryExn` and its variants (now implements `Exn` with a well-known selector)
- `FaultString` type
- `contract_call_encode` / `contract_call_decode` utilities
- The 500-byte `AbortBuf` / `EncodeBuf` size bound

## Breaking Changes

- Wire format is not backwards compatible (i32 discriminant → u64 selector)
- All error types must add `#[derive(Exn)]`
- `throws = X` still works but `throws(X)` is the canonical form
- `try_call` removed in favor of `try_call_raw`
- `CallError<E>` and `CallResult<T, E>` removed — client code using these directly must migrate to generated error enums
- `abort()` requires `Exn` instead of `WriteXDR`
