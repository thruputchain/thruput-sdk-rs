# Selector-Based Error Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `CrossExn<E>` with u64 selector-based error dispatch so contracts can declare multiple error types and clients get a generated error enum with an `Unknown` fallback.

**Architecture:** Every error type gets a u64 selector (SHA-256 of its canonical signature). The wire format changes from `[i32 disc][payload]` to `[u64 selector][payload]`. `CrossExn<E>` is removed. The `#[contract]` macro generates per-client error enums and selector dispatch code. A new `try_call_raw` function replaces `try_call` by returning raw abort bytes without decoding.

**Tech Stack:** Rust proc-macro (`syn`, `quote`, `darling`, `sha2`), `heapless`, XDR serialization (`xdrs`/`cxs`)

**Spec:** `docs/superpowers/specs/2026-03-25-selector-based-error-dispatch.md`

---

### Task 1: Add `const SELECTOR: u64` to the `Exn` trait and implement for `LibraryExn`

**Files:**
- Modify: `src/exn/mod.rs:7-28` (Exn trait definition)
- Modify: `src/exn/library.rs:14-48` (LibraryExn — add manual Exn impl)

This task adds the SELECTOR constant to the Exn trait, removes the blanket impl, and provides a manual implementation for LibraryExn. Other types will get their impls via `#[derive(Exn)]` in a later task; for now we add manual impls to keep things compiling.

- [ ] **Step 1: Add SELECTOR to the Exn trait and remove blanket impl**

In `src/exn/mod.rs`, change both feature-gated `exn_impl` modules. The blanket `impl<T> Exn for T` is removed — each type must explicitly implement `Exn`.

```rust
// src/exn/mod.rs — debugging variant
#[cfg(feature = "debugging")]
mod exn_impl {
    use crate::xdr::*;
    use core::error::Error;
    use core::fmt::Debug;

    pub trait Exn:
        Debug + PartialEq + Error + WriteXDR + ReadXDROwned
    {
        const SELECTOR: u64;
    }
}

// src/exn/mod.rs — non-debugging variant
#[cfg(not(feature = "debugging"))]
mod exn_impl {
    use crate::xdr::*;
    use core::fmt::Debug;

    pub trait Exn: Debug + PartialEq + WriteXDR + ReadXDROwned {
        const SELECTOR: u64;
    }
}
```

- [ ] **Step 2: Compute LibraryExn's well-known selector**

The canonical signature for `LibraryExn` is built from its variant names and field types. Since `LibraryExn` is `#[non_exhaustive]` and has conditional variants (storage feature), we'll compute the selector from the base variants that are always present. Use a hardcoded constant derived offline from `sha2::Sha256::digest(b"LibraryExn::Syscall(SyscallExn)::Handling(HandlingError)::Calling(CallingError)::Called(FaultString)::BytestringWrongSize(u32,u32)::NonNegativeIntUnderflow::NonNegativeIntOverflow")` — take the first 8 bytes as big-endian u64.

Write a `#[test]` in `src/exn/library.rs` that verifies the hardcoded constant matches the SHA-256 computation, then hardcode `LibraryExn::SELECTOR` to that value.

```rust
// Add to src/exn/library.rs, after the LibraryExn enum

impl crate::exn::Exn for LibraryExn {
    // SHA-256 of canonical signature, first 8 bytes as big-endian u64
    // See test below for derivation
    const SELECTOR: u64 = {
        // Computed from:
        // "LibraryExn::Syscall(SyscallExn)::Handling(HandlingError)::Calling(CallingError)::Called(FaultString)::BytestringWrongSize(u32,u32)::NonNegativeIntUnderflow::NonNegativeIntOverflow"
        // We'll fill in the actual value after computing it
        0 // PLACEHOLDER — compute in step 3
    };
}
```

- [ ] **Step 3: Compute the actual selector value**

Write a small test (or use a scratch binary) in `src/exn/library.rs` to compute the value:

```rust
#[cfg(test)]
mod selector_tests {
    use super::*;
    use sha2::{Sha256, Digest};

    #[test]
    fn library_exn_selector_derivation() {
        let sig = "LibraryExn::Syscall(SyscallExn)::Handling(HandlingError)::Calling(CallingError)::Called(FaultString)::BytestringWrongSize(u32,u32)::NonNegativeIntUnderflow::NonNegativeIntOverflow";
        let hash = Sha256::digest(sig.as_bytes());
        let selector = u64::from_be_bytes(hash[..8].try_into().unwrap());
        assert_eq!(LibraryExn::SELECTOR, selector);
    }
}
```

Run: `cargo test -p sdk --lib exn::library::selector_tests -- --nocapture`

This will fail because SELECTOR is 0. Read the test output to get the real value, then update the `const SELECTOR` in the `impl Exn for LibraryExn` block.

- [ ] **Step 4: Run the test again to verify it passes**

Run: `cargo test -p sdk --lib exn::library::selector_tests`
Expected: PASS

- [ ] **Step 5: Add temporary Exn impls for Infallible and any other types that need it**

The codebase uses `Infallible` as the default error type. Add a manual impl:

```rust
// In src/exn/mod.rs, after the exn_impl modules:

impl Exn for core::convert::Infallible {
    const SELECTOR: u64 = 0; // Well-known: should never appear on the wire
}
```

Check what other types implement the old blanket Exn. Search for types used as the `E` parameter in `CrossExn<E>`, `CallError<E>`, etc. These will need manual impls or `#[derive(Exn)]` — but we'll handle them with the derive macro in Task 3. For now, add temporary manual impls to any types in the SDK itself that are used as error types (grep for `Exn` usage).

- [ ] **Step 6: Verify the workspace compiles**

Run: `cargo check -p sdk`

Fix any compilation errors from the removed blanket impl. Types outside the SDK (in example contracts, integration tests) will break — that's expected and will be fixed in later tasks.

- [ ] **Step 7: Commit**

```bash
git add src/exn/mod.rs src/exn/library.rs
git commit -m "feat: add const SELECTOR to Exn trait, impl for LibraryExn"
```

---

### Task 2: Unified `serialize_error` and update `abort()`

**Files:**
- Modify: `src/exn/cross_exn.rs:225-277` (replace serialize functions)
- Modify: `src/abort.rs` (update abort() signature)
- Modify: `src/exn/library.rs:79-83` (update library_abort to use serialize_error)

This task unifies `serialize_contract_error` and `serialize_library_error` into a single `serialize_error` that writes `[u64 selector][payload][debug info]`.

- [ ] **Step 1: Write a test for the new wire format**

Add to `src/exn/cross_exn.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::call::internal::contract_call_encode;
    use crate::exn::library::SyscallExn;
    use crate::sys::{SysErr, SyscallNo};

    // Test error type with known selector
    #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, thiserror::Error)]
    #[repr(i32)]
    enum TestExn {
        #[error("boom")]
        Boom = 1,
    }

    impl crate::exn::Exn for TestExn {
        const SELECTOR: u64 = 0xDEADBEEF_CAFEBABE; // test value
    }

    #[test]
    fn serialize_error_writes_selector_then_payload() {
        // Manually encode what serialize_error should produce
        let exn = TestExn::Boom;
        let expected = contract_call_encode(&(TestExn::SELECTOR, &exn)).unwrap();

        // The first 8 bytes should be the selector
        let selector_bytes = &expected[..8];
        let selector = u64::from_be_bytes(selector_bytes.try_into().unwrap());
        assert_eq!(selector, 0xDEADBEEF_CAFEBABE);

        // The remaining bytes should be the XDR-encoded TestExn
        let payload_bytes = &expected[8..];
        let decoded: TestExn = cxs::decode(payload_bytes).unwrap();
        assert_eq!(decoded, TestExn::Boom);
    }
}
```

- [ ] **Step 2: Run test to verify it passes**

This test only validates the encoding format, not `serialize_error` itself (which calls `contract_abort`). Run:

Run: `cargo test -p sdk --lib exn::cross_exn::tests::serialize_error_writes_selector_then_payload`
Expected: PASS

- [ ] **Step 3: Replace `serialize_contract_error` and `serialize_library_error` with `serialize_error`**

In `src/exn/cross_exn.rs`, replace the four serialize functions (two pairs, debug/non-debug) with a single pair:

```rust
#[cfg(not(feature = "debugging"))]
pub(crate) fn serialize_error(err: &(impl WriteXDR + crate::exn::Exn)) -> ! {
    use crate::call::internal::contract_call_encode;

    let selector = <_ as crate::exn::Exn>::SELECTOR;
    let buf = contract_call_encode(&(selector, err))
        .unwrap_or_else(|e| super::encoding_abort_abort(e));

    crate::contract_abort(buf.as_ref());
}

#[cfg(feature = "debugging")]
#[track_caller]
pub(crate) fn serialize_error(err: &(impl WriteXDR + crate::exn::Exn)) -> ! {
    use crate::call::internal::contract_call_encode;
    use core::panic::Location;

    let selector = <_ as crate::exn::Exn>::SELECTOR;
    let location: Option<LocationString> = Some(format(Location::caller()));
    let backtrace: Option<BacktraceString> =
        Some(format(&::abort::Backtrace::capture()));

    let buf =
        contract_call_encode(&(selector, err, &location, &backtrace))
            .unwrap_or_else(|e| super::encoding_abort_abort(e));

    crate::contract_abort(buf.as_ref());
}
```

Remove `serialize_contract_error` and `serialize_library_error`.

- [ ] **Step 4: Update `abort()` in `src/abort.rs`**

```rust
/// Abort the contract with a contract-specific error.
#[cfg_attr(feature = "debugging", track_caller)]
pub fn abort(err: &impl crate::exn::Exn) -> ! {
    crate::exn::cross_exn::serialize_error(err)
}
```

- [ ] **Step 5: Update `library_abort` in `src/exn/library.rs`**

Change `library_abort` to use `serialize_error`:

```rust
#[cfg_attr(feature = "debugging", track_caller)]
pub fn library_abort<E: Into<LibraryExn>>(e: E) -> ! {
    let exn: LibraryExn = e.into();
    crate::exn::cross_exn::serialize_error(&exn)
}
```

- [ ] **Step 6: Update any remaining callers of the old functions**

Search for `serialize_contract_error` and `serialize_library_error` across the codebase. Update all call sites to use `serialize_error`.

- [ ] **Step 7: Remove old CrossExn tests that test the old wire format**

The existing tests in `cross_exn.rs` (`serialize_abort_matches_derive_for_contract_exn`, etc.) test the old `Inner`-based format. Remove them. The new test from Step 1 validates the new format.

- [ ] **Step 8: Verify compilation**

Run: `cargo check -p sdk`

- [ ] **Step 9: Run tests**

Run: `cargo test -p sdk --lib`
Expected: PASS

- [ ] **Step 10: Commit**

```bash
git add src/exn/cross_exn.rs src/abort.rs src/exn/library.rs
git commit -m "feat: unify error serialization into serialize_error with u64 selector"
```

---

### Task 3: `#[derive(Exn)]` proc macro

**Files:**
- Create: `macros/src/derive_exn.rs` (derive macro implementation)
- Modify: `macros/src/lib.rs` (register the derive macro)

This task creates the `#[derive(Exn)]` derive macro that computes a u64 selector from the type's canonical signature.

- [ ] **Step 1: Write the canonical signature builder**

Create `macros/src/derive_exn.rs`:

```rust
use proc_macro::TokenStream;
use sha2::{Digest, Sha256};
use syn::{Data, DeriveInput, Fields};

pub fn derive(input: DeriveInput) -> TokenStream {
    let name = &input.ident;
    let signature = canonical_signature(&input);
    let selector = compute_selector(&signature);

    let (impl_generics, ty_generics, where_clause) =
        input.generics.split_for_impl();

    quote::quote! {
        impl #impl_generics ::sdk::exn::Exn for #name #ty_generics #where_clause {
            const SELECTOR: u64 = #selector;
        }
    }
    .into()
}

fn canonical_signature(input: &DeriveInput) -> String {
    let name = input.ident.to_string();

    match &input.data {
        Data::Enum(data) => {
            let variants: Vec<String> = data
                .variants
                .iter()
                .map(|v| {
                    let vname = v.ident.to_string();
                    match &v.fields {
                        Fields::Named(fields) => {
                            let ftypes: Vec<String> = fields
                                .named
                                .iter()
                                .map(|f| type_to_string(&f.ty))
                                .collect();
                            format!("{}({})", vname, ftypes.join(","))
                        }
                        Fields::Unnamed(fields) => {
                            let ftypes: Vec<String> = fields
                                .unnamed
                                .iter()
                                .map(|f| type_to_string(&f.ty))
                                .collect();
                            format!("{}({})", vname, ftypes.join(","))
                        }
                        Fields::Unit => vname,
                    }
                })
                .collect();
            format!("{}::{}", name, variants.join("::"))
        }
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => {
                let ftypes: Vec<String> = fields
                    .named
                    .iter()
                    .map(|f| type_to_string(&f.ty))
                    .collect();
                format!("{}({})", name, ftypes.join(","))
            }
            Fields::Unnamed(fields) => {
                let ftypes: Vec<String> = fields
                    .unnamed
                    .iter()
                    .map(|f| type_to_string(&f.ty))
                    .collect();
                format!("{}({})", name, ftypes.join(","))
            }
            Fields::Unit => name,
        },
        Data::Union(_) => {
            panic!("#[derive(Exn)] is not supported for unions")
        }
    }
}

fn type_to_string(ty: &syn::Type) -> String {
    // Use the token stream representation, stripped of whitespace normalization
    quote::quote!(#ty).to_string().replace(' ', "")
}

fn compute_selector(signature: &str) -> u64 {
    let hash = Sha256::digest(signature.as_bytes());
    u64::from_be_bytes(hash[..8].try_into().unwrap())
}
```

- [ ] **Step 2: Register the derive macro in `macros/src/lib.rs`**

Add the module declaration and the proc_macro_derive:

```rust
mod derive_exn;

#[proc_macro_derive(Exn)]
pub fn derive_exn(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_exn::derive(input)
}
```

- [ ] **Step 3: Re-export from the SDK**

In `src/lib.rs`, add alongside the other macro re-exports:

```rust
pub use groundhog_macros::Exn;
```

- [ ] **Step 4: Test the derive macro on a simple enum**

Add a test in `tests/macros.rs` (or a new file if that's cleaner):

```rust
#[cfg(test)]
mod exn_derive_tests {
    use sdk::exn::Exn;
    use sdk::xdr::*;

    #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, sdk::Exn, thiserror::Error)]
    #[repr(i32)]
    enum TestError {
        #[error("a")]
        VariantA = 1,
        #[error("b")]
        VariantB(u64) = 2,
    }

    #[test]
    fn derive_exn_produces_stable_selector() {
        // Selector should be deterministic
        let s1 = TestError::SELECTOR;
        let s2 = TestError::SELECTOR;
        assert_eq!(s1, s2);
        assert_ne!(s1, 0); // Should not be zero (that's Infallible)
    }

    #[test]
    fn derive_exn_selector_matches_manual_sha256() {
        use sha2::{Digest, Sha256};

        // Canonical signature: "TestError::VariantA::VariantB(u64)"
        let sig = "TestError::VariantA::VariantB(u64)";
        let hash = Sha256::digest(sig.as_bytes());
        let expected = u64::from_be_bytes(hash[..8].try_into().unwrap());

        assert_eq!(TestError::SELECTOR, expected);
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p sdk exn_derive`
Expected: PASS

- [ ] **Step 6: Update LibraryExn to use `#[derive(Exn)]` instead of manual impl**

In `src/exn/library.rs`, add `Exn` to the derive list for `LibraryExn`. Remove the manual `impl Exn for LibraryExn` block. Verify the selector test from Task 1 still passes (the derive macro should compute the same signature).

Note: `LibraryExn` has a conditional `#[cfg(feature = "storage")]` variant. The derive macro will include it when the feature is enabled, which means the selector changes based on features. This is acceptable — the feature set is fixed at compile time for both contract and client. If this is problematic, we can add a `#[exn(skip)]` attribute later. For now, proceed.

Run: `cargo test -p sdk --lib exn::library::selector_tests`

If the selector changes (because derive includes the Storage variant when compiled with that feature), update the expected value in the test.

- [ ] **Step 7: Commit**

```bash
git add macros/src/derive_exn.rs macros/src/lib.rs src/lib.rs src/exn/library.rs
git commit -m "feat: add #[derive(Exn)] proc macro for selector computation"
```

---

### Task 4: `RawError` type with `try_as`

**Files:**
- Create: `src/exn/raw_error.rs`
- Modify: `src/exn/mod.rs` (add module + re-export)

- [ ] **Step 1: Write tests for `RawError`**

Create `src/exn/raw_error.rs`:

```rust
use crate::call::internal::contract_call_decode;
use crate::exn::Exn;
use crate::xdr::*;
use core::fmt::{self, Debug};

/// An unrecognized error from a contract call.
///
/// Contains the raw selector and XDR payload bytes for errors that
/// don't match any declared throws type. Use `try_as::<T>()` to
/// attempt decoding as a known type.
pub struct RawError {
    selector: u64,
    payload: heapless::Vec<u8, 500>,
}

impl RawError {
    pub fn new(selector: u64, payload: &[u8]) -> Self {
        let mut buf = heapless::Vec::new();
        // Copy as much as fits; payload came from a 500-byte AbortBuf
        // minus 8 bytes for the selector, so it always fits.
        buf.extend_from_slice(payload).ok();
        Self {
            selector,
            payload: buf,
        }
    }

    /// The u64 selector hash of the error type that was thrown.
    pub fn selector(&self) -> u64 {
        self.selector
    }

    /// The raw XDR payload bytes (without the selector prefix).
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Attempt to decode the raw bytes as a specific error type.
    ///
    /// Returns `Some(T)` if the selector matches `T::SELECTOR` and
    /// the payload decodes successfully. Returns `None` otherwise.
    pub fn try_as<T: Exn>(&self) -> Option<T> {
        if self.selector == T::SELECTOR {
            contract_call_decode(&self.payload).ok()
        } else {
            None
        }
    }
}

impl Debug for RawError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "RawError {{ selector: 0x{:016X}, payload: {} bytes }}",
            self.selector,
            self.payload.len()
        )
    }
}

impl PartialEq for RawError {
    fn eq(&self, other: &Self) -> bool {
        self.selector == other.selector && self.payload == other.payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call::internal::contract_call_encode;

    #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, thiserror::Error)]
    #[repr(i32)]
    enum FakeError {
        #[error("fake")]
        Fake = 1,
    }

    impl Exn for FakeError {
        const SELECTOR: u64 = 0x1234_5678_9ABC_DEF0;
    }

    #[test]
    fn try_as_matching_selector_decodes() {
        let encoded = contract_call_encode(&FakeError::Fake).unwrap();
        let raw = RawError::new(FakeError::SELECTOR, &encoded);
        let decoded = raw.try_as::<FakeError>();
        assert_eq!(decoded, Some(FakeError::Fake));
    }

    #[test]
    fn try_as_wrong_selector_returns_none() {
        let encoded = contract_call_encode(&FakeError::Fake).unwrap();
        let raw = RawError::new(0xFFFF, &encoded);
        let decoded = raw.try_as::<FakeError>();
        assert_eq!(decoded, None);
    }

    #[test]
    fn try_as_corrupt_payload_returns_none() {
        let raw = RawError::new(FakeError::SELECTOR, &[0xFF, 0xFF, 0xFF]);
        let decoded = raw.try_as::<FakeError>();
        assert_eq!(decoded, None);
    }
}
```

- [ ] **Step 2: Add module to `src/exn/mod.rs`**

```rust
mod raw_error;
pub use raw_error::*;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p sdk --lib exn::raw_error`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/exn/raw_error.rs src/exn/mod.rs
git commit -m "feat: add RawError type with try_as escape hatch"
```

---

### Task 5: `try_call_raw` — raw error call function

**Files:**
- Modify: `src/call/call_result.rs` (add `RawCallError`, `into_raw_call_result`)
- Modify: `src/call/calls.rs` (add `try_call_raw`)
- Modify: `src/call/mod.rs` (export new types)

- [ ] **Step 1: Define `RawCallError`**

Add to `src/call/call_result.rs`:

```rust
use crate::exn::FaultString;

/// The raw, untyped error from a contract call.
///
/// The selector and payload are not decoded — the caller (generated client code)
/// is responsible for dispatching on the selector and decoding the payload.
#[derive(Debug, PartialEq)]
pub enum RawCallError {
    /// The called contract aborted with a selector-tagged error.
    Abort {
        /// The u64 selector identifying the error type.
        selector: u64,
        /// The raw bytes after the selector.
        ///
        /// In non-debugging builds this is just the error XDR. In debugging
        /// builds the error XDR is followed by location/backtrace — the
        /// type-specific decoder will read only the bytes it needs and
        /// ignore the trailing debug info. If the decoder is strict about
        /// trailing bytes, the error falls through to `Unknown`, which is
        /// acceptable degradation. Proper debug-info extraction can be
        /// added in a follow-up.
        payload: heapless::Vec<u8, 500>,
    },
    /// The called contract terminated without providing a return value.
    Fault(FaultString),
}
```

Note: We use an owned `heapless::Vec` here rather than a borrowed slice. The borrow would tie to the `ReturnBuf` inside `try_call_raw`, but the `ReturnBuf` is a local — we can't return a reference to it. The copy is from a stack-allocated 500-byte buffer, so it's cheap.

- [ ] **Step 2: Write `into_raw_call_result`**

Add to `src/call/call_result.rs`:

```rust
/// Decode a `ContractResult` into a typed success value or a raw error.
///
/// On success: decodes `T` from the return buffer.
/// On abort: extracts the u64 selector and raw payload without decoding the error.
/// On fault: returns `RawCallError::Fault`.
pub fn into_raw_call_result<T>(
    contract_result: ContractResult,
) -> Result<T, RawCallError>
where
    T: ReadXDROwned,
{
    match contract_result {
        Ok(return_val) => {
            contract_call_decode(return_val.as_slice())
                .map(Ok)
                .unwrap_or_else(|e|
                    library_abort(CallingError::DecodingSuccess(FaultString::from_display(e))))
        }
        Err(ContractError::Abort(abort_buf)) => {
            let bytes = abort_buf.as_slice();
            if bytes.len() < 8 {
                // Not enough bytes for a selector — treat as fault
                library_abort(CallingError::DecodingAbort(
                    FaultString::from_display("abort buffer too short for selector")))
            }

            let selector = u64::from_be_bytes(bytes[..8].try_into().unwrap());
            let payload_bytes = &bytes[8..];

            // The payload includes everything after the selector: error XDR,
            // and in debugging builds, trailing location/backtrace data.
            // The type-specific decoder reads only what it needs.
            let mut payload = heapless::Vec::new();
            payload.extend_from_slice(payload_bytes).ok();

            Err(RawCallError::Abort {
                selector,
                payload,
            })
        }
        Err(ContractError::Fault(fault_val)) => Err(RawCallError::Fault(fault_val)),
    }
}
```

- [ ] **Step 3: Write `try_call_raw` in `src/call/calls.rs`**

```rust
pub fn try_call_raw<ARGS, RET>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &ARGS,
) -> Result<RET, RawCallError>
where
    ARGS: WriteXDR,
    RET: ReadXDROwned,
{
    // Encode the args
    let args_buf = contract_call_encode(args)
        .unwrap_or_else(|e| {
            library_abort(CallingError::EncodingArgs(
                FaultString::from_display(e),
            ))
        });

    // Invoke the method
    let contract_result =
        match contract_call(address, entrypoint, gas_limit, &args_buf) {
            Ok(result) => result,
            Err(syserr) => {
                return Err(RawCallError::Fault(FaultString::from_display(
                    syserr,
                )));
            }
        };

    // Decode success or return raw error
    into_raw_call_result(contract_result)
}
```

- [ ] **Step 4: Export new types from `src/call/mod.rs`**

Add `RawCallError` and `try_call_raw` to the module's public exports. Also keep the existing `try_call`, `CallError`, and `CallResult` for now — they'll be removed in a later task after the client generation is updated.

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p sdk`

- [ ] **Step 6: Commit**

```bash
git add src/call/call_result.rs src/call/calls.rs src/call/mod.rs
git commit -m "feat: add try_call_raw with selector-based RawCallError"
```

---

### Task 6: Config parsing — multiple `throws` types

**Files:**
- Modify: `gen/src/config.rs` (ThrowsList, custom FromMeta)
- Modify: `gen/src/contract.rs` (update ContractSrc::throws return type)
- Modify: `gen/src/client.rs` (update throws usage)
- Modify: `gen/src/spec.rs` (update throws usage)

- [ ] **Step 1: Write tests for ThrowsList parsing**

Add to `gen/src/config.rs` tests:

```rust
#[test]
fn throws_single_ident() {
    let config = parse_config(r#"throws = MyError"#);
    let throws = config.throws_list();
    assert_eq!(throws.len(), 1);
    assert_eq!(throws[0].to_string(), "MyError");
}

#[test]
fn throws_list_syntax() {
    let config = parse_config(r#"throws(SwapError, ERC20Error)"#);
    let throws = config.throws_list();
    assert_eq!(throws.len(), 2);
    assert_eq!(throws[0].to_string(), "SwapError");
    assert_eq!(throws[1].to_string(), "ERC20Error");
}

#[test]
fn throws_list_single() {
    let config = parse_config(r#"throws(SwapError)"#);
    let throws = config.throws_list();
    assert_eq!(throws.len(), 1);
    assert_eq!(throws[0].to_string(), "SwapError");
}

#[test]
fn throws_none() {
    let config = parse_config(r#""#);
    let throws = config.throws_list();
    assert!(throws.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p groundhog-codegen config::tests`
Expected: FAIL (throws_list method doesn't exist yet)

- [ ] **Step 3: Implement ThrowsList and update Config**

In `gen/src/config.rs`:

```rust
use darling::FromMeta;
use darling::util::Flag;
use proc_macro2::Span;

/// A list of one or more thrown error type identifiers.
///
/// Supports both `throws = SingleError` and `throws(ErrorA, ErrorB)` syntax.
#[derive(Clone, Debug)]
pub struct ThrowsList(Vec<syn::Ident>);

impl FromMeta for ThrowsList {
    // Handle `throws = Ident` (NameValue form)
    fn from_value(value: &syn::Lit) -> darling::Result<Self> {
        Err(darling::Error::unexpected_lit_type(value))
    }

    fn from_word() -> darling::Result<Self> {
        Err(darling::Error::unsupported_format("word"))
    }

    // Handle `throws = Ident`
    fn from_expr(expr: &syn::Expr) -> darling::Result<Self> {
        match expr {
            syn::Expr::Path(ep) if ep.path.get_ident().is_some() => {
                let ident = ep.path.get_ident().unwrap().clone();
                Ok(ThrowsList(vec![ident]))
            }
            _ => Err(darling::Error::custom(
                "throws expects an identifier or list of identifiers",
            )),
        }
    }

    // Handle `throws(A, B, C)` (List form)
    fn from_list(items: &[darling::ast::NestedMeta]) -> darling::Result<Self> {
        let mut idents = Vec::new();
        for item in items {
            match item {
                darling::ast::NestedMeta::Meta(syn::Meta::Path(path)) => {
                    if let Some(ident) = path.get_ident() {
                        idents.push(ident.clone());
                    } else {
                        return Err(darling::Error::custom(
                            "throws expects simple identifiers, not paths",
                        )
                        .with_span(path));
                    }
                }
                _ => {
                    return Err(darling::Error::custom(
                        "throws expects identifiers",
                    ));
                }
            }
        }
        if idents.is_empty() {
            return Err(darling::Error::custom("throws() requires at least one type"));
        }
        Ok(ThrowsList(idents))
    }
}

#[derive(Clone, Debug, FromMeta)]
pub struct Config {
    pub exports: Option<String>,
    pub test_setup: Option<String>,
    pub storage: Option<bool>,
    pub interface: Option<String>,
    pub version: Option<String>,
    pub init: Flag,
    pub doc: Option<String>,

    client: Option<syn::Ident>,
    throws: Option<ThrowsList>,
}

impl Config {
    // ... existing methods ...

    /// Returns the list of thrown error types (empty if none declared).
    pub fn throws_list(&self) -> Vec<&syn::Ident> {
        match &self.throws {
            Some(ThrowsList(idents)) => idents.iter().collect(),
            None => vec![],
        }
    }

    /// Returns the first thrown error type (backwards compat for single-throws code).
    pub fn throws(&self) -> Option<&syn::Ident> {
        self.throws.as_ref().and_then(|t| t.0.first())
    }
}
```

- [ ] **Step 4: Update `ContractSrc::throws` to return a list**

In `gen/src/contract.rs`, update the trait and all implementations:

```rust
// In ContractSrc trait:
fn throws_list(&self) -> Vec<&syn::Ident> {
    vec![]
}

// Keep the old single-value accessor for backwards compat during migration:
fn throws(&self) -> Option<&syn::Ident> {
    self.throws_list().first().copied()
}
```

Update `ConfiguredContract`:
```rust
fn throws_list(&self) -> Vec<&syn::Ident> {
    let config_throws = self.config.throws_list();
    if !config_throws.is_empty() {
        config_throws
    } else {
        self.contract.throws_list()
    }
}
```

- [ ] **Step 5: Run config tests**

Run: `cargo test -p groundhog-codegen config::tests`
Expected: PASS

- [ ] **Step 6: Verify full compilation**

Run: `cargo check -p groundhog-codegen && cargo check -p groundhog-macros`

- [ ] **Step 7: Commit**

```bash
git add gen/src/config.rs gen/src/contract.rs
git commit -m "feat: support throws(A, B) list syntax in contract macro config"
```

---

### Task 7: Generate client error enum

**Files:**
- Modify: `gen/src/client.rs` (generate error enum + dispatch code)

This is the core code generation change. The macro generates a per-client error enum and rewrites `try_` methods to use `try_call_raw` with selector dispatch.

- [ ] **Step 1: Write the error enum generation function**

Add to `gen/src/client.rs`:

```rust
fn generate_error_enum(
    client_ident: &syn::Ident,
    throws_list: &[&syn::Ident],
) -> TokenStream {
    let error_enum_ident = format_ident!("{}Error", client_ident);

    // Declared error variants: variant name = type name
    let declared_variants: Vec<TokenStream> = throws_list
        .iter()
        .map(|ident| {
            quote! { #ident(#ident) }
        })
        .collect();

    // Always-present variants
    let base = quote! {
        #[derive(Debug, PartialEq)]
        pub enum #error_enum_ident {
            #(#declared_variants,)*
            Library(::sdk::exn::LibraryExn),
            Fault(::sdk::exn::FaultString),
            Unknown(::sdk::exn::RawError),
        }
    };

    base
}
```

- [ ] **Step 2: Write the selector dispatch generation**

Add to `gen/src/client.rs`:

```rust
fn generate_selector_dispatch(
    error_enum_ident: &syn::Ident,
    throws_list: &[&syn::Ident],
) -> TokenStream {
    let declared_arms: Vec<TokenStream> = throws_list
        .iter()
        .map(|ident| {
            quote! {
                <#ident as ::sdk::exn::Exn>::SELECTOR => {
                    let err = ::sdk::call::internal::contract_call_decode(payload)
                        .unwrap_or_else(|e|
                            ::sdk::exn::library_abort(
                                ::sdk::exn::CallingError::DecodingAbort(
                                    ::sdk::exn::FaultString::from_display(e))));
                    Err(#error_enum_ident::#ident(err))
                }
            }
        })
        .collect();

    quote! {
        match selector {
            #(#declared_arms)*
            <::sdk::exn::LibraryExn as ::sdk::exn::Exn>::SELECTOR => {
                let err = ::sdk::call::internal::contract_call_decode(payload)
                    .unwrap_or_else(|e|
                        ::sdk::exn::library_abort(
                            ::sdk::exn::CallingError::DecodingAbort(
                                ::sdk::exn::FaultString::from_display(e))));
                Err(#error_enum_ident::Library(err))
            }
            _ => {
                Err(#error_enum_ident::Unknown(
                    ::sdk::exn::RawError::new(selector, payload)))
            }
        }
    }
}
```

- [ ] **Step 3: Update `generate_client_fn` to use `try_call_raw`**

Rewrite the method generation in `gen/src/client.rs`. The `try_` method now returns `Result<ReturnType, ClientError>` and does selector dispatch. The non-try method wraps it:

```rust
fn generate_client_fn(
    method: &impl Method,
    contract: &impl ContractSrc,
) -> Result<TokenStream> {
    // ... existing preamble (ident, sig_output, entrypoint, params, etc.) ...

    let client_ident = contract.client_ident().unwrap();
    let error_enum_ident = format_ident!("{}Error", client_ident);
    let throws_list = contract.throws_list();

    let selector_dispatch = generate_selector_dispatch(&error_enum_ident, &throws_list);

    // try_ method: uses try_call_raw + selector dispatch
    let try_method = quote_spanned! {method.sig().span()=>
        #[allow(clippy::result_large_err)]
        #track_caller
        pub fn #try_ident(&mut self, #(#sent_params),* ) -> ::core::result::Result<#return_type, #error_enum_ident>
        {
            let entrypoint = #entrypoint_lit;
            let args = #args_tuple;

            #log_call

            with_call_tracking!(#call_info, {
                match ::sdk::call::try_call_raw(&self.address, entrypoint, self.config.gas_limit, &args) {
                    Ok(val) => Ok(val),
                    Err(::sdk::call::RawCallError::Fault(s)) => Err(#error_enum_ident::Fault(s)),
                    Err(::sdk::call::RawCallError::Abort { selector, ref payload }) => {
                        let payload = payload.as_slice();
                        #selector_dispatch
                    }
                }
            })
        }
    };

    // Non-try method: calls try_ and aborts on error
    let non_try_method = quote_spanned! {method.sig().span()=>
        #track_caller
        pub fn #ident(&mut self, #(#sent_params),* ) #sig_output
        {
            let entrypoint = #entrypoint_lit;
            let args = #args_tuple;

            #log_call

            with_call_tracking!(#call_info, {
                match ::sdk::call::try_call_raw::<_, #return_type>(&self.address, entrypoint, self.config.gas_limit, &args) {
                    Ok(val) => val,
                    Err(raw_err) => {
                        // Re-abort with the raw error bytes
                        panic!("contract call failed: {:?}", raw_err)
                    }
                }
            })
        }
    };

    Ok(quote! {
        #non_try_method
        #try_method
    })
}
```

- [ ] **Step 4: Update `generate` to emit the error enum**

In `gen/src/client.rs`, the top-level `generate` function:

```rust
pub fn generate(
    contract: &(impl ContractSrc + Spanned),
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), contract);

    let client_ident = contract.client_ident().ok_or_else(|| {
        error(&contract.span(), "contract client identifier must be given by 'client = ClientName' attribute")
    })?;

    let throws_list = contract.throws_list();
    let error_enum = generate_error_enum(&client_ident, &throws_list);

    // ... rest of existing generation (define_spec_fn, method_stubs, etc.) ...

    let client = quote! {
        #define_spec_fn
        #error_enum

        pub struct #client_ident {
            // ... existing struct ...
        }

        // ... existing impl ...
    };

    Ok(client)
}
```

- [ ] **Step 5: Verify compilation of the codegen crate**

Run: `cargo check -p groundhog-codegen`

- [ ] **Step 6: Verify compilation of the macros crate**

Run: `cargo check -p groundhog-macros`

- [ ] **Step 7: Commit**

```bash
git add gen/src/client.rs
git commit -m "feat: generate per-client error enum with selector dispatch"
```

---

### Task 8: Remove `CrossExn`, `CallError`, and old `try_call`

**Files:**
- Modify: `src/exn/cross_exn.rs` (remove CrossExn struct, Inner enum; keep serialize_error)
- Modify: `src/call/call_result.rs` (remove CallError, CallResult, into_call_result)
- Modify: `src/call/calls.rs` (remove try_call and call)
- Modify: `src/call/mod.rs` (update exports)
- Modify: `src/lib.rs` (remove re-exports of CallError, CallResult)

- [ ] **Step 1: Remove `CrossExn` and `Inner` from `cross_exn.rs`**

Remove the `CrossExn` struct, `Inner` enum, `INNER_CONTRACT`/`INNER_LIBRARY`/`INNER_UNKNOWN` constants, all `impl` blocks for `CrossExn`, and the `format`/`display_error` helpers. Keep `serialize_error` and its helpers (`LocationString`, `BacktraceString`).

Rename the file if desired, or keep it as `cross_exn.rs` with just the serialize function.

- [ ] **Step 2: Remove `CallError` and `CallResult` from `call_result.rs`**

Remove the `CallError<E>` enum, `CallResult<T, E>` type alias, `into_call_result`, and all `impl` blocks for `CallError`. Keep `RawCallError`, `into_raw_call_result`.

- [ ] **Step 3: Remove `try_call` and `call` from `calls.rs`**

Remove `try_call` and `call` functions. Keep `try_call_raw` and `contract_call`.

- [ ] **Step 4: Update exports in `src/call/mod.rs` and `src/lib.rs`**

Remove `pub use call::{CallError, CallResult};` from `src/lib.rs`. Remove corresponding exports from `src/call/mod.rs`. Add `RawCallError` to exports.

- [ ] **Step 5: Fix all compilation errors**

There will be broken references throughout the codebase. Search for `CallError`, `CallResult`, `CrossExn`, `try_call` (but not `try_call_raw`), and fix each site.

Key places to check:
- `src/exn/cross_exn.rs` itself (methods that referenced `CallError`)
- `gen/src/client.rs` (should already be updated from Task 7)
- Any integration tests or example contracts that use `CallError`/`CallResult`/`CrossExn` directly

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p sdk`

- [ ] **Step 7: Run all SDK tests**

Run: `cargo test -p sdk`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add src/exn/cross_exn.rs src/call/call_result.rs src/call/calls.rs src/call/mod.rs src/lib.rs
git commit -m "refactor: remove CrossExn, CallError, CallResult, try_call"
```

---

### Task 9: Update example contracts and integration tests

**Files:**
- Modify: All example contracts that use `throws`
- Modify: All integration test contracts that define error types
- Modify: Standard interfaces (`standard-interfaces/groundhog-erc20/src/lib.rs`, etc.)

- [ ] **Step 1: Add `#[derive(Exn)]` to all error types in example contracts**

Search for error types across the workspace:

```bash
grep -rn "throws" example-contracts/ standard-interfaces/ integration_test/
```

For each error type referenced in a `throws` attribute, add `Exn` to its derive list. For example, in `example-contracts/exceptions/src/interface.rs`:

```rust
// Before:
#[derive(Error, Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
#[repr(i32)]
pub enum MyErr { ... }

// After:
#[derive(Error, Debug, Clone, PartialEq, ReadXDR, WriteXDR, Exn)]
#[repr(i32)]
pub enum MyErr { ... }
```

Repeat for every error type used with `throws`.

- [ ] **Step 2: Update any code that matches on `CallError` or `CallResult`**

Search for `CallError`, `CallResult`, `CrossExn` in integration tests and examples. Update to use the generated error enum instead.

For example, if test code does:
```rust
match client.try_foo() {
    Err(CallError::Abort(exn)) => ...
}
```

Change to:
```rust
match client.try_foo() {
    Err(FooClientError::MyErr(e)) => ...
}
```

- [ ] **Step 3: Verify example contracts compile**

Run: `cargo check -p exceptions` (and other example contract packages)

- [ ] **Step 4: Verify integration tests compile and pass**

Run: `cargo test` (from workspace root, or target specific integration test packages)

- [ ] **Step 5: Commit**

```bash
git add example-contracts/ standard-interfaces/ integration_test/
git commit -m "chore: update example contracts and tests for selector-based errors"
```

---

### Task 10: Update spec generation for multiple throws types

**Files:**
- Modify: `gen/src/spec.rs` (emit multiple error types in method specs)

- [ ] **Step 1: Update `method_repr` to emit a list of error types**

In `gen/src/spec.rs`, the `err` field currently emits a single `Option<TypeSpec>`. Update it to emit a `Vec<TypeSpec>` or similar structure:

```rust
let errs = contract.throws_list();
let err = if errs.is_empty() {
    quote! { vec![] }
} else {
    let err_specs = errs.iter().map(|e| {
        quote! { <#e as HasTypeSpec>::type_spec() }
    });
    quote! { vec![ #(#err_specs),* ] }
};
```

Update the `Method` struct in the spec module if it uses `err: Option<TypeSpec>` — change to `err: Vec<TypeSpec>`.

- [ ] **Step 2: Verify spec compilation**

Run: `cargo check -p groundhog-codegen --features spec`

- [ ] **Step 3: Commit**

```bash
git add gen/src/spec.rs
git commit -m "feat: spec generation emits multiple error types"
```

---

### Task 11: End-to-end test with multiple throws types

**Files:**
- Create or modify an integration test contract that declares `throws(ErrorA, ErrorB)`

This validates the full pipeline: macro parsing → error enum generation → selector serialization → client dispatch → `try_as` on Unknown.

- [ ] **Step 1: Create a test contract with multiple error types**

Create a small test contract (or extend an existing one in `integration_test/contracts/`) with two error types and `throws(ErrorA, ErrorB)`:

```rust
use sdk::*;
use sdk::xdr::*;

#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, Exn, thiserror::Error)]
#[repr(i32)]
pub enum AlphaError {
    #[error("alpha failed")]
    Failed = 1,
}

#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, Exn, thiserror::Error)]
#[repr(i32)]
pub enum BetaError {
    #[error("beta failed")]
    Failed = 1,
}

#[contract(client = MultiErrClient, throws(AlphaError, BetaError))]
pub trait MultiErr {
    fn throw_alpha(&mut self);
    fn throw_beta(&mut self);
    fn succeed(&self) -> u32;
}
```

- [ ] **Step 2: Write a test that exercises selector dispatch**

```rust
#[test]
fn multi_error_dispatch() {
    // Setup contract and client...

    // Call that throws AlphaError
    match client.try_throw_alpha() {
        Err(MultiErrClientError::AlphaError(AlphaError::Failed)) => {} // expected
        other => panic!("expected AlphaError::Failed, got {:?}", other),
    }

    // Call that throws BetaError
    match client.try_throw_beta() {
        Err(MultiErrClientError::BetaError(BetaError::Failed)) => {} // expected
        other => panic!("expected BetaError::Failed, got {:?}", other),
    }

    // Call that succeeds
    assert_eq!(client.try_succeed().unwrap(), 42);
}
```

- [ ] **Step 3: Write a test for Unknown error with try_as**

```rust
#[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR, Exn, thiserror::Error)]
#[repr(i32)]
pub enum SecretError {
    #[error("secret")]
    Secret = 1,
}

#[test]
fn unknown_error_try_as() {
    // Setup: contract throws SecretError, which is NOT in the throws() list
    // ...

    match client.try_some_method() {
        Err(MultiErrClientError::Unknown(raw)) => {
            let secret = raw.try_as::<SecretError>();
            assert_eq!(secret, Some(SecretError::Secret));
        }
        other => panic!("expected Unknown, got {:?}", other),
    }
}
```

- [ ] **Step 4: Run the test**

Run the integration test.
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add integration_test/
git commit -m "test: end-to-end test for multi-error selector dispatch"
```

---

### Task 12: Clean up and final verification

**Files:**
- Various: any remaining dead code, unused imports, warnings

- [ ] **Step 1: Run full workspace check**

Run: `cargo check --workspace`

Fix any remaining compilation errors.

- [ ] **Step 2: Run full test suite**

Run: `cargo test --workspace`

Fix any test failures.

- [ ] **Step 3: Clean up dead code**

Search for:
- Unused `use` statements referencing removed types (`CrossExn`, `CallError`, `CallResult`, `Inner`)
- Any `#[allow(dead_code)]` that's no longer needed
- Old comments referencing the removed types

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "chore: clean up dead code and unused imports"
```
