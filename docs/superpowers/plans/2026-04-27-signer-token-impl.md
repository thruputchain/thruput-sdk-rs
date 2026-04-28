# Signer Token SDK Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `Signer<T>` as a first-class SDK type for minting, receiving, consuming, and forwarding opaque auth tokens across contract calls.

**Architecture:** `Signer<T>` is a newtype around `u32` with `PhantomData<T>`, serialized as a plain `u32` in XDR. A new `CollectAuthHandles` trait extracts signer handle IDs from argument tuples so the generated client code can pass them through the `Invoke` syscall's auth token channel alongside regular calldata. The callee decodes signers from calldata like any other argument.

**Tech Stack:** Rust, `xdrs` (XDR serialization), `heapless` (no-alloc buffers), `groundhog-macros` / `groundhog-codegen` (proc macros)

**Spec:** `docs/superpowers/specs/2026-04-27-signer-token-design.md`

**Prerequisite:** The `groundhog-xdr` crate must be bumped to a version that includes the `MintAuthToken` syscall wrapper (syscall 33). The `GetCalldata` syscall may have gained a second output slice; the SDK ignores it for now (passes an empty buffer). If the `groundhog-xdr` bump is not yet available, Tasks 1-5 can proceed with a stub `mint_auth_token` that panics; Task 6 wires the real syscall once available.

---

### Task 1: Define the `Signer<T>` type with XDR serialization

**Files:**
- Create: `src/signer.rs`
- Modify: `src/lib.rs` (add module declaration + re-export)

- [ ] **Step 1: Create `src/signer.rs` with the `Signer<T>` struct**

```rust
use core::marker::PhantomData;
use crate::xdr::*;

#[derive(Clone, Copy)]
pub struct Signer<T> {
    handle: u32,
    _phantom: PhantomData<T>,
}

impl<T> Signer<T> {
    pub(crate) fn from_handle(handle: u32) -> Self {
        Self {
            handle,
            _phantom: PhantomData,
        }
    }

    pub(crate) fn handle(&self) -> u32 {
        self.handle
    }
}

impl<T> core::fmt::Debug for Signer<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Signer")
            .field("handle", &self.handle)
            .finish()
    }
}
```

- [ ] **Step 2: Add `WriteXDR` and `ReadXDR` impls that delegate to `u32`**

Append to `src/signer.rs`:

```rust
impl<T> WriteXDR for Signer<T> {
    #[inline(always)]
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        self.handle.write_xdr(w)
    }
}

impl<'r, T> ReadXDR<'r> for Signer<T> {
    #[inline(always)]
    fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
        let handle = u32::read_xdr(r)?;
        Ok(Self::from_handle(handle))
    }
}
```

- [ ] **Step 3: Register the module in `src/lib.rs`**

Add to the module declarations (near line 80, alongside other public modules):

```rust
pub mod signer;
```

Add to the re-exports (near line 108, alongside other `pub use` lines):

```rust
pub use signer::*;
```

- [ ] **Step 4: Add unit tests for XDR roundtrip**

Append to `src/signer.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signer_xdr_roundtrip() {
        let signer: Signer<u64> = Signer::from_handle(42);

        let mut buf = [0u8; 32];
        let written = cxs::encode_into(&signer, &mut buf).unwrap();
        assert_eq!(written, 4);

        let decoded: Signer<u64> = cxs::decode(&buf[..written]).unwrap();
        assert_eq!(decoded.handle(), 42);
    }

    #[test]
    fn signer_xdr_as_tuple_member() {
        let args: (u64, Signer<u32>, i32) =
            (100, Signer::from_handle(7), -1);

        let mut buf = [0u8; 64];
        let written = cxs::encode_into(&args, &mut buf).unwrap();

        let decoded: (u64, Signer<u32>, i32) =
            cxs::decode(&buf[..written]).unwrap();
        assert_eq!(decoded.0, 100);
        assert_eq!(decoded.1.handle(), 7);
        assert_eq!(decoded.2, -1);
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib signer::tests -- --nocapture`

Expected: both tests PASS.

- [ ] **Step 6: Commit**

```bash
git add src/signer.rs src/lib.rs
git commit -m "feat: add Signer<T> type with XDR serialization"
```

---

### Task 2: Define the `CollectAuthHandles` trait with primitive impls

**Files:**
- Create: `src/collect_auth_handles.rs`
- Modify: `src/lib.rs` (add module declaration + re-export)

- [ ] **Step 1: Create `src/collect_auth_handles.rs` with the trait and buffer type**

```rust
const AUTH_HANDLES_MAX: usize = 32;

pub type AuthHandleBuf = heapless::Vec<u32, AUTH_HANDLES_MAX>;

pub trait CollectAuthHandles {
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf);
}
```

- [ ] **Step 2: Add no-op impls for primitive types via a macro**

Append to `src/collect_auth_handles.rs`:

```rust
macro_rules! impl_collect_noop {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl CollectAuthHandles for $ty {
                #[inline(always)]
                fn collect_auth_handles(&self, _out: &mut AuthHandleBuf) {}
            }
        )+
    };
}

impl_collect_noop!(
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
    bool,
    (),
);
```

- [ ] **Step 3: Add impls for byte/slice types**

Append to `src/collect_auth_handles.rs`:

```rust
impl CollectAuthHandles for [u8] {
    #[inline(always)]
    fn collect_auth_handles(&self, _out: &mut AuthHandleBuf) {}
}

impl CollectAuthHandles for str {
    #[inline(always)]
    fn collect_auth_handles(&self, _out: &mut AuthHandleBuf) {}
}

impl<const N: usize> CollectAuthHandles for [u8; N] {
    #[inline(always)]
    fn collect_auth_handles(&self, _out: &mut AuthHandleBuf) {}
}
```

- [ ] **Step 4: Add impls for `&T`, `Option<T>`, and generic slices**

Append to `src/collect_auth_handles.rs`:

```rust
impl<T: CollectAuthHandles + ?Sized> CollectAuthHandles for &T {
    #[inline(always)]
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf) {
        (**self).collect_auth_handles(out);
    }
}

impl<T: CollectAuthHandles> CollectAuthHandles for Option<T> {
    #[inline(always)]
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf) {
        if let Some(inner) = self {
            inner.collect_auth_handles(out);
        }
    }
}

impl<T: CollectAuthHandles> CollectAuthHandles for [T] {
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf) {
        for item in self {
            item.collect_auth_handles(out);
        }
    }
}
```

- [ ] **Step 5: Add impls for `xdrs` container types**

Append to `src/collect_auth_handles.rs`:

```rust
impl<T: CollectAuthHandles, const N: usize> CollectAuthHandles
    for xdrs::VecM<T, N>
{
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf) {
        self.as_ref().collect_auth_handles(out);
    }
}

impl<const N: usize> CollectAuthHandles for xdrs::BytesM<N> {
    #[inline(always)]
    fn collect_auth_handles(&self, _out: &mut AuthHandleBuf) {}
}
```

- [ ] **Step 6: Add `Signer<T>` impl — in `src/signer.rs`**

Add to `src/signer.rs` (after the XDR impls):

```rust
use crate::collect_auth_handles::{AuthHandleBuf, CollectAuthHandles};

impl<T> CollectAuthHandles for Signer<T> {
    #[inline(always)]
    fn collect_auth_handles(&self, out: &mut AuthHandleBuf) {
        out.push(self.handle)
            .expect("auth handle buffer overflow");
    }
}
```

- [ ] **Step 7: Register module in `src/lib.rs`**

Add alongside other module declarations:

```rust
pub mod collect_auth_handles;
```

Add alongside other re-exports:

```rust
pub use collect_auth_handles::*;
```

- [ ] **Step 8: Add unit tests**

Append to `src/collect_auth_handles.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::signer::Signer;

    #[test]
    fn primitives_collect_nothing() {
        let mut buf = AuthHandleBuf::new();
        42u64.collect_auth_handles(&mut buf);
        true.collect_auth_handles(&mut buf);
        (-1i32).collect_auth_handles(&mut buf);
        assert!(buf.is_empty());
    }

    #[test]
    fn signer_collects_handle() {
        let mut buf = AuthHandleBuf::new();
        let s: Signer<u32> = Signer::from_handle(99);
        s.collect_auth_handles(&mut buf);
        assert_eq!(buf.as_slice(), &[99]);
    }

    #[test]
    fn option_some_collects() {
        let mut buf = AuthHandleBuf::new();
        let s: Option<Signer<u32>> = Some(Signer::from_handle(5));
        s.collect_auth_handles(&mut buf);
        assert_eq!(buf.as_slice(), &[5]);
    }

    #[test]
    fn option_none_collects_nothing() {
        let mut buf = AuthHandleBuf::new();
        let s: Option<Signer<u32>> = None;
        s.collect_auth_handles(&mut buf);
        assert!(buf.is_empty());
    }

    #[test]
    fn slice_of_signers() {
        let mut buf = AuthHandleBuf::new();
        let signers: &[Signer<u32>] = &[
            Signer::from_handle(1),
            Signer::from_handle(2),
            Signer::from_handle(3),
        ];
        signers.collect_auth_handles(&mut buf);
        assert_eq!(buf.as_slice(), &[1, 2, 3]);
    }
}
```

- [ ] **Step 9: Run tests**

Run: `cargo test --lib collect_auth_handles::tests -- --nocapture`

Expected: all tests PASS.

- [ ] **Step 10: Commit**

```bash
git add src/collect_auth_handles.rs src/signer.rs src/lib.rs
git commit -m "feat: add CollectAuthHandles trait with primitive and Signer impls"
```

---

### Task 3: Add tuple impls for `CollectAuthHandles`

**Files:**
- Modify: `src/collect_auth_handles.rs`

Tuples are how the generated code wraps method arguments. The macro generates `let args = (arg1, arg2, ...);` — so tuples must implement `CollectAuthHandles`.

- [ ] **Step 1: Add the tuple impl macro**

Add to `src/collect_auth_handles.rs` (after the primitive impls, before `#[cfg(test)]`):

```rust
macro_rules! impl_collect_tuple {
    ($(($($name:ident),+)),+ $(,)?) => {
        $(
            impl<$($name: CollectAuthHandles),+> CollectAuthHandles for ($($name,)+) {
                #[inline(always)]
                #[allow(non_snake_case)]
                fn collect_auth_handles(&self, out: &mut AuthHandleBuf) {
                    let ($($name,)+) = self;
                    $($name.collect_auth_handles(out);)+
                }
            }
        )+
    };
}

impl_collect_tuple!(
    (A),
    (A, B),
    (A, B, C),
    (A, B, C, D),
    (A, B, C, D, E),
    (A, B, C, D, E, F),
    (A, B, C, D, E, F, G),
    (A, B, C, D, E, F, G, H),
    (A, B, C, D, E, F, G, H, I),
    (A, B, C, D, E, F, G, H, I, J),
    (A, B, C, D, E, F, G, H, I, J, K),
    (A, B, C, D, E, F, G, H, I, J, K, L),
    (A, B, C, D, E, F, G, H, I, J, K, L, M),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N),
);
```

This matches the existing tuple arities in `groundhog-contract-spec/src/lib.rs:265-279`.

- [ ] **Step 2: Add tuple tests**

Add to the `#[cfg(test)]` module in `src/collect_auth_handles.rs`:

```rust
    #[test]
    fn mixed_tuple_collects_only_signers() {
        let mut buf = AuthHandleBuf::new();
        let args: (u64, Signer<u32>, i32, Signer<bool>) =
            (100, Signer::from_handle(10), -1, Signer::from_handle(20));
        args.collect_auth_handles(&mut buf);
        assert_eq!(buf.as_slice(), &[10, 20]);
    }

    #[test]
    fn single_element_tuple() {
        let mut buf = AuthHandleBuf::new();
        let args: (Signer<u32>,) = (Signer::from_handle(42),);
        args.collect_auth_handles(&mut buf);
        assert_eq!(buf.as_slice(), &[42]);
    }

    #[test]
    fn tuple_with_no_signers() {
        let mut buf = AuthHandleBuf::new();
        let args: (u64, i32, bool) = (1, 2, true);
        args.collect_auth_handles(&mut buf);
        assert!(buf.is_empty());
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib collect_auth_handles::tests -- --nocapture`

Expected: all tests (including new tuple tests) PASS.

- [ ] **Step 4: Commit**

```bash
git add src/collect_auth_handles.rs
git commit -m "feat: add CollectAuthHandles tuple impls"
```

---

### Task 4: Wire auth handles through `try_call_raw` and `contract_call`

**Files:**
- Modify: `src/call/calls.rs`
- Modify: `src/call/internal/encode.rs`

This task threads the `CollectAuthHandles` output through the call stack so that `raw_Invoke` receives the auth token ID list.

- [ ] **Step 1: Add an auth-handle encoding helper in `src/call/internal/encode.rs`**

Add after the existing `contract_call_encode` function:

```rust
use crate::collect_auth_handles::{AuthHandleBuf, CollectAuthHandles};

pub fn collect_auth_handle_bytes<T: CollectAuthHandles>(
    args: &T,
) -> AuthHandleBuf {
    let mut handles = AuthHandleBuf::new();
    args.collect_auth_handles(&mut handles);
    handles
}

pub fn auth_handles_as_bytes(handles: &[u32]) -> &[u8] {
    // Safety: &[u32] → &[u8] is always valid; u8 has alignment 1.
    unsafe {
        core::slice::from_raw_parts(
            handles.as_ptr() as *const u8,
            handles.len() * core::mem::size_of::<u32>(),
        )
    }
}
```

- [ ] **Step 2: Update `try_call_raw` to collect and pass auth handles**

In `src/call/calls.rs`, change `try_call_raw` to add the `CollectAuthHandles` bound and collect handles:

Replace the current `try_call_raw` function (lines 12-30) with:

```rust
pub fn try_call_raw<ARGS, RET>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &ARGS,
) -> Result<RET, RawCallError>
where
    ARGS: WriteXDR + crate::CollectAuthHandles,
    RET: ReadXDROwned,
{
    let args_buf = contract_call_encode(args).unwrap_or_else(|e| {
        library_throw(CallingError::EncodingArgs(FaultString::from_display(e)))
    });

    let auth_handles = super::internal::collect_auth_handle_bytes(args);

    let contract_result = contract_call(
        address,
        entrypoint,
        gas_limit,
        &args_buf,
        &auth_handles,
    )?;

    into_raw_call_result(contract_result)
}
```

- [ ] **Step 3: Update `contract_call` to accept and pass auth handles to `raw_Invoke`**

In `src/call/calls.rs`, change the `contract_call` signature and body. Replace lines 37-97:

```rust
#[inline(never)]
#[allow(clippy::result_large_err)]
pub fn contract_call<A>(
    address: &ContractAddress,
    entrypoint: ContractEntrypointID,
    gas_limit: u64,
    args: &A,
    auth_token_ids: &crate::collect_auth_handles::AuthHandleBuf,
) -> Result<ContractResult, RawCallError>
where
    A: AsRef<[u8]> + ?Sized,
{
    let auth_bytes =
        super::internal::auth_handles_as_bytes(auth_token_ids.as_slice());

    // Perform the cross-contract call
    let mut return_len = 0;
    let mut auth_token_id_list_return_len = 0;
    let entrypoint = u64::from_be_bytes(entrypoint.into_inner());
    let result = sys::raw_Invoke(
        address,
        entrypoint,
        gas_limit,
        args.as_ref(),
        0, // TODO(geoff): invoke flags go here
        auth_bytes,
        &mut return_len,
        &mut auth_token_id_list_return_len,
    );

    let is_ok = match result {
        Ok(()) => true,
        Err(syserr) if syserr.is_aborted() => false,
        Err(syserr) => {
            return Err(RawCallError::Fault(FaultString::from_display(
                syserr,
            )));
        }
    };

    // Ok or exception: get the return data from the VM ...

    let mut return_buf = ReturnBuf::new();
    let mut auth_token_id_list_buf = ReturnBuf::new();
    if let Err(fault) = fill_return_buf(
        return_len,
        auth_token_id_list_return_len,
        &mut return_buf,
        &mut auth_token_id_list_buf,
    ) {
        return Err(if is_ok {
            RawCallError::UnparseableReturn(fault)
        } else {
            RawCallError::UnparseableAbort(fault)
        });
    }

    let contract_result = if is_ok {
        Ok(return_buf)
    } else {
        Err(ContractError::Abort(return_buf))
    };

    Ok(contract_result)
}
```

- [ ] **Step 4: Fix any callers of `contract_call` that pass args directly**

Search for other callers of `contract_call` that may need updating. The main caller is `try_call_raw` (already updated). Check `src/dev/` and integration tests for direct `contract_call` usage:

Run: `grep -rn "contract_call(" src/ --include="*.rs" | grep -v "encode\|decode\|result"` to find callers.

Any direct caller that passed encoded bytes needs to also pass an `&AuthHandleBuf::new()` for the auth handles (empty, since they're encoding manually).

- [ ] **Step 5: Verify it compiles**

Run: `cargo check`

Expected: compiles with no errors. Existing code that calls `try_call_raw` with args that impl `WriteXDR` also needs those types to impl `CollectAuthHandles`. Since the generated client code uses tuples of contract types, and we've added tuple impls, this should work for all existing contracts (whose args are primitives/`#[contracttype]` structs — see Task 6 for contracttype).

If the check fails because some type doesn't impl `CollectAuthHandles`, add the missing impl.

- [ ] **Step 6: Run existing tests**

Run: `cargo test --lib`

Expected: all existing tests still PASS (no behavioral change — auth handle list is empty for existing code).

- [ ] **Step 7: Commit**

```bash
git add src/call/calls.rs src/call/internal/encode.rs
git commit -m "feat: wire auth handle collection through contract_call to raw_Invoke"
```

---

### Task 5: Update client codegen to require `CollectAuthHandles`

**Files:**
- Modify: `gen/src/client.rs`

The generated client methods call `try_call_raw` with `&args`. Since `try_call_raw` now requires `ARGS: WriteXDR + CollectAuthHandles`, the args tuple must implement both. No explicit codegen changes are needed for the call site — the trait bound propagates automatically through `try_call_raw`.

However, we should verify the generated code compiles and add a compile-pass test with a `Signer` parameter.

- [ ] **Step 1: Add a compile-pass test with `Signer<T>` method parameter**

Create `tests/compile-pass/signer_method_param.rs`:

```rust
#![allow(unexpected_cfgs)]

use sdk::*;
use sdk::signer::Signer;

#[derive(Default)]
pub struct SignerTestContract;

#[contract(storage = false)]
impl SignerTestContract {
    pub fn with_signer(&self, amount: u64, auth: Signer<u32>) -> u32 {
        let _ = (amount, auth);
        0
    }

    pub fn multiple_signers(
        &self,
        a: Signer<u32>,
        x: i32,
        b: Signer<bool>,
    ) -> u32 {
        let _ = (a, x, b);
        0
    }

    pub fn no_signers(&self, x: u64) -> u64 {
        x
    }
}

fn main() {
    let client = SignerTestContractClient::new(&ContractAddress::default());
    // Verify client methods accept Signer params
    let _ = &client;
}
```

- [ ] **Step 2: Run compile-pass tests**

Run: `cargo test compile_pass`

Expected: PASS. The new test file compiles successfully alongside existing tests.

- [ ] **Step 3: Commit**

```bash
git add tests/compile-pass/signer_method_param.rs
git commit -m "test: add compile-pass test for Signer<T> in contract methods"
```

---

### Task 6: Add `CollectAuthHandles` derive to `#[contracttype]`

**Files:**
- Modify: `macros/src/contract_type.rs`

When `Signer<T>` appears inside a `#[contracttype]` struct or enum, that type must also impl `CollectAuthHandles`. The `#[contracttype]` macro already has access to the item AST, so we generate the impl inline.

- [ ] **Step 1: Add a helper to generate `CollectAuthHandles` impl for structs**

In `macros/src/contract_type.rs`, add a helper function:

```rust
fn expand_collect_auth_handles_struct(item: &syn::ItemStruct) -> TokenStream {
    let ident = &item.ident;
    let field_calls: Vec<TokenStream> = match &item.fields {
        syn::Fields::Named(fields) => fields
            .named
            .iter()
            .map(|f| {
                let name = &f.ident;
                quote! { self.#name.collect_auth_handles(out); }
            })
            .collect(),
        syn::Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let idx = syn::Index::from(i);
                quote! { self.#idx.collect_auth_handles(out); }
            })
            .collect(),
        syn::Fields::Unit => vec![],
    };

    quote! {
        impl ::sdk::CollectAuthHandles for #ident {
            fn collect_auth_handles(
                &self,
                out: &mut ::sdk::collect_auth_handles::AuthHandleBuf,
            ) {
                #(#field_calls)*
            }
        }
    }
}
```

- [ ] **Step 2: Add a helper for enums**

Add to `macros/src/contract_type.rs`:

```rust
fn expand_collect_auth_handles_enum(item: &syn::ItemEnum) -> TokenStream {
    let ident = &item.ident;
    let arms: Vec<TokenStream> = item
        .variants
        .iter()
        .map(|variant| {
            let vident = &variant.ident;
            match &variant.fields {
                syn::Fields::Named(fields) => {
                    let names: Vec<_> =
                        fields.named.iter().map(|f| &f.ident).collect();
                    let calls: Vec<TokenStream> = names
                        .iter()
                        .map(|n| quote! { #n.collect_auth_handles(out); })
                        .collect();
                    quote! {
                        Self::#vident { #(#names),* } => { #(#calls)* }
                    }
                }
                syn::Fields::Unnamed(fields) => {
                    let bindings: Vec<syn::Ident> = (0..fields.unnamed.len())
                        .map(|i| quote::format_ident!("f{}", i))
                        .collect();
                    let calls: Vec<TokenStream> = bindings
                        .iter()
                        .map(|b| quote! { #b.collect_auth_handles(out); })
                        .collect();
                    quote! {
                        Self::#vident(#(#bindings),*) => { #(#calls)* }
                    }
                }
                syn::Fields::Unit => {
                    quote! { Self::#vident => {} }
                }
            }
        })
        .collect();

    quote! {
        impl ::sdk::CollectAuthHandles for #ident {
            fn collect_auth_handles(
                &self,
                out: &mut ::sdk::collect_auth_handles::AuthHandleBuf,
            ) {
                match self {
                    #(#arms)*
                }
            }
        }
    }
}
```

- [ ] **Step 3: Wire the helpers into the `expand` function**

In `macros/src/contract_type.rs`, modify the `expand` function. In the `Item::Enum` arm, after the existing `quote!` block, add the `CollectAuthHandles` impl. In the `Item::Struct` arm, do the same.

For enums, change:

```rust
            let ident = &item.ident;
            let name = expand_contract_type_name(ident, &args);
            // ... existing code ...
            quote! {
                #[derive(::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
                #[xdrs(crate = "::sdk::__xdrs")]
                #[repr(i32)]
                #export_attr
                #item

                #name
            }
```

To:

```rust
            let ident = &item.ident;
            let name = expand_contract_type_name(ident, &args);
            let collect_auth = expand_collect_auth_handles_enum(&item);
            // ... existing code ...
            quote! {
                #[derive(::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
                #[xdrs(crate = "::sdk::__xdrs")]
                #[repr(i32)]
                #export_attr
                #item

                #name

                #collect_auth
            }
```

For structs, similarly:

```rust
            let ident = &item.ident;
            let name = expand_contract_type_name(ident, &args);
            let collect_auth = expand_collect_auth_handles_struct(&item);
            // ... existing code ...
            quote! {
                #[derive(::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
                #[xdrs(crate = "::sdk::__xdrs")]
                #export_attr
                #item

                #name

                #collect_auth
            }
```

- [ ] **Step 4: Add a compile-pass test with `Signer` inside a `#[contracttype]` struct**

Create `tests/compile-pass/signer_in_contracttype.rs`:

```rust
#![allow(unexpected_cfgs)]

use sdk::*;
use sdk::signer::Signer;

#[contracttype]
pub struct TransferRequest {
    pub amount: u64,
    pub auth: Signer<u32>,
}

#[derive(Default)]
pub struct SignerStructContract;

#[contract(storage = false)]
impl SignerStructContract {
    pub fn transfer(&self, req: TransferRequest) -> u32 {
        let _ = req;
        0
    }
}

fn main() {
    let _client =
        SignerStructContractClient::new(&ContractAddress::default());
}
```

- [ ] **Step 5: Run compile-pass tests**

Run: `cargo test compile_pass`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add macros/src/contract_type.rs tests/compile-pass/signer_in_contracttype.rs
git commit -m "feat: derive CollectAuthHandles in #[contracttype] macro"
```

---

### Task 7: Add `mint_auth_token` syscall wrapper

**Files:**
- Modify: `src/signer.rs`

This wraps the `MintAuthToken` syscall (number 33). Requires `groundhog-xdr` version with `uninit_MintAuthToken` available. If the crate hasn't been bumped yet, skip this task and return to it after the bump.

- [ ] **Step 1: Verify the syscall is available**

Run: `grep -rn "MintAuthToken" src/ --include="*.rs"` and `grep -rn "kMintAuthToken" src/ --include="*.rs"`

If `uninit_MintAuthToken` or `raw_MintAuthToken` is not found, check whether a `groundhog-xdr` version bump is needed in `Cargo.toml`. If the syscall wrapper does not exist in the current `groundhog-xdr`, this task is blocked.

- [ ] **Step 2: Add `mint_auth_token` function to `src/signer.rs`**

Add to `src/signer.rs`:

```rust
use crate::call::internal::contract_call_encode;
use crate::exn::{FaultString, HandlingError, library_throw, syscall_throw};
use crate::sys::{self, SyscallNo};
use crate::xdr::*;
use groundhog_xdr::types::BlockHeight;

pub fn mint_auth_token<T: WriteXDR>(
    expiry: BlockHeight,
    data: &T,
) -> Signer<T> {
    let buf = contract_call_encode(data).unwrap_or_else(|e| {
        library_throw(HandlingError::EncodingSuccess(
            FaultString::from_display(e),
        ))
    });

    let mut token_id: core::mem::MaybeUninit<u32> =
        core::mem::MaybeUninit::uninit();

    sys::uninit_MintAuthToken(expiry, buf.as_ref(), &mut token_id)
        .unwrap_or_else(|e| syscall_throw(SyscallNo::kMintAuthToken, e));

    let handle = unsafe { token_id.assume_init() };
    Signer::from_handle(handle)
}
```

Note: The exact syscall wrapper name (`uninit_MintAuthToken`) and the `SyscallNo::kMintAuthToken` variant depend on the generated code from `groundhog-xdr`. Adjust the names to match whatever the code generator produces.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`

Expected: compiles. If the syscall names don't match, adjust to the actual generated names.

- [ ] **Step 4: Commit**

```bash
git add src/signer.rs
git commit -m "feat: add mint_auth_token syscall wrapper"
```

---

### Task 8: Update `get_calldata` for new `GetCalldata` signature (if needed)

**Files:**
- Modify: `src/contract_op.rs`

If the `groundhog-xdr` bump changed the `GetCalldata` syscall to have a second output span (auth token IDs), the existing `get_calldata()` / `try_get_calldata()` will fail to compile. Update them to pass an empty buffer for the new output.

- [ ] **Step 1: Check if `GetCalldata` signature changed**

Run: `cargo check 2>&1 | head -30`

If it compiles, skip this task. If there's a compile error about `uninit_GetCalldata` argument count, proceed.

- [ ] **Step 2: Update `try_get_calldata` in `src/contract_op.rs`**

Replace the `try_get_calldata` function with a version that passes an empty buffer for the second output:

```rust
pub fn try_get_calldata() -> SysResult<Calldata> {
    let mut calldata: Calldata = MUBuf::new();

    let mut written: MaybeUninit<RuntimeMemoryLength> = MaybeUninit::uninit();
    // Pass an empty buffer for the auth token ID list output (ignored)
    let mut auth_token_ids_written: MaybeUninit<RuntimeMemoryLength> =
        MaybeUninit::uninit();
    sys::uninit_GetCalldata(
        calldata.spare_capacity_mut(),
        &mut written,
        &mut [],  // empty buffer for auth token IDs
        &mut auth_token_ids_written,
    )?;
    let written = unsafe { written.assume_init() };

    unsafe {
        calldata.set_len(written as usize);
    }

    Ok(calldata)
}
```

Note: The exact shape of the updated `uninit_GetCalldata` depends on the `groundhog-xdr` code generator output. Adjust parameter order and types to match.

- [ ] **Step 3: Verify**

Run: `cargo check`

Expected: compiles.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib`

Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/contract_op.rs
git commit -m "fix: update get_calldata for new GetCalldata syscall signature"
```
