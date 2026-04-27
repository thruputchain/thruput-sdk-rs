# Mintable Trait Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the `Mintable` trait, `UnsignedSigner<T>`, tree-building `mint_auth_token`, and a `consume()` stub, enabling ergonomic auth token minting and typed consuming (minus the actual ConsumeAuthToken syscall).

**Architecture:** `Mintable` trait has an associated `Unsigned` type and `build_auth_token_node` method. `UnsignedSigner<T>` pairs a consumer address with unsigned data. The `#[contracttype]` macro generates companion unsigned structs in a private module + `Mintable` impls. `mint_auth_token` builds the `AuthTokenNode` tree and serializes it for the syscall. `consume()` stubs the inverse with `ReadXDR` + `ReceiveAuthHandles`.

**Tech Stack:** Rust, `xdrs` (XDR), `heapless`, `groundhog-xdr` 0.9.2 (`AuthTokenNode`, `AuthTokenPayload`, `kAuthTokenMaxChildNodes`), `groundhog-macros` (proc macros)

**Spec:** `docs/superpowers/specs/2026-04-27-mintable-trait-design.md`

---

### Task 1: Add `UnsignedSigner<T>`, `Mintable` trait, and `Signer<T>` impl

**Files:**
- Create: `src/mintable.rs`
- Modify: `src/lib.rs` (add module + re-export)
- Modify: `src/signer.rs` (add Default impl, Mintable impl)
- Modify: `src/types.rs` (re-export auth_token types)

- [ ] **Step 1: Re-export auth_token types from groundhog-xdr**

In `src/types.rs`, add:

```rust
pub use groundhog_xdr::auth_token::*;
```

This makes `AuthTokenNode`, `AuthTokenPayload`, `kAuthTokenMaxChildNodes` etc. available as `crate::types::AuthTokenNode`.

- [ ] **Step 2: Create `src/mintable.rs` with the trait and `UnsignedSigner`**

```rust
use crate::types::{AuthTokenNode, AuthTokenPayload, ContractAddress};
use crate::xdr::*;
use groundhog_xdr::auth_token::kAuthTokenMaxChildNodes;

pub struct UnsignedSigner<T> {
    pub consumer: ContractAddress,
    pub data: T,
}

pub trait Mintable: Sized {
    type Unsigned;

    fn build_auth_token_node(unsigned: Self::Unsigned) -> AuthTokenNode;
}

pub fn encode_payload<T: WriteXDR>(value: &T) -> AuthTokenPayload {
    let buf = crate::call::internal::contract_call_encode(value)
        .unwrap_or_else(|e| {
            crate::exn::library_throw(crate::AuthTokenError::MintEncodeFailure(
                crate::exn::FaultString::from_display(e),
            ))
        });
    let bytes: xdrs::BytesM<{ groundhog_xdr::auth_token::kAuthTokenMaxLen }> =
        buf.as_slice().try_into().unwrap_or_else(|_| {
            panic!("auth token payload exceeds max length")
        });
    AuthTokenPayload::from(bytes)
}

pub fn auth_token_children(
    nodes: alloc::vec::Vec<AuthTokenNode>,
) -> xdrs::VecM<AuthTokenNode, kAuthTokenMaxChildNodes> {
    nodes.try_into().unwrap_or_else(|_| {
        panic!("too many auth token children (max {})", kAuthTokenMaxChildNodes)
    })
}
```

Note: `auth_token_children` requires `alloc` (since `VecM::try_from(Vec<T>)` takes a `Vec`). Gate it behind `#[cfg(feature = "alloc")]` if needed, but it's only used at mint time which is a contract operation.

- [ ] **Step 3: Add `Default` impl for `Signer<T>` in `src/signer.rs`**

Add after the Debug impl:

```rust
impl<T> Default for Signer<T> {
    fn default() -> Self {
        Self {
            handle: u32::MAX,
            _phantom: PhantomData,
        }
    }
}
```

This is used by the `#[contracttype]` macro-generated `build_auth_token_node` to construct the payload value with placeholder signers.

- [ ] **Step 4: Add `Mintable` impl for `Signer<T>` in `src/signer.rs`**

Add:

```rust
use crate::mintable::{Mintable, UnsignedSigner};

impl<T: Mintable> Mintable for Signer<T> {
    type Unsigned = UnsignedSigner<<T as Mintable>::Unsigned>;

    fn build_auth_token_node(
        unsigned: UnsignedSigner<<T as Mintable>::Unsigned>,
    ) -> crate::types::AuthTokenNode {
        let mut node = T::build_auth_token_node(unsigned.data);
        node.consumer_address = unsigned.consumer;
        node
    }
}
```

- [ ] **Step 5: Register module in `src/lib.rs`**

Add `pub mod mintable;` alongside other module declarations.
Add `pub use mintable::*;` alongside other re-exports.

- [ ] **Step 6: Add unit tests in `src/mintable.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::signer::Signer;

    #[test]
    fn primitive_mintable_builds_leaf_node() {
        let node = u64::build_auth_token_node(42u64);
        assert!(node.child_nodes.is_empty());
    }

    #[test]
    fn signer_mintable_sets_consumer() {
        let consumer = ContractAddress::default();
        let node = <Signer<u64>>::build_auth_token_node(UnsignedSigner {
            consumer: consumer.clone(),
            data: 42u64,
        });
        assert_eq!(node.consumer_address, consumer);
        assert!(node.child_nodes.is_empty());
    }
}
```

- [ ] **Step 7: Verify**

Run: `cargo test --lib mintable::tests -- --nocapture`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
cargo fmt
git add src/mintable.rs src/signer.rs src/lib.rs src/types.rs
git commit -m "feat: add Mintable trait, UnsignedSigner, and Signer<T> impl"
```

---

### Task 2: Add `Mintable` impls for primitives, Option, Vec, tuples

**Files:**
- Modify: `src/mintable.rs`

These mirror the `CollectAuthHandles` impls. Primitives and byte types are leaf nodes (payload = XDR of value, no children). Container types delegate.

- [ ] **Step 1: Add primitive impls via macro**

Add to `src/mintable.rs`:

```rust
macro_rules! impl_mintable_leaf {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl Mintable for $ty {
                type Unsigned = $ty;

                fn build_auth_token_node(unsigned: $ty) -> AuthTokenNode {
                    AuthTokenNode {
                        consumer_address: ContractAddress::default(),
                        payload: encode_payload(&unsigned),
                        child_nodes: Default::default(),
                    }
                }
            }
        )+
    };
}

impl_mintable_leaf!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, bool, (),);
```

- [ ] **Step 2: Add impls for common types**

```rust
impl Mintable for ContractAddress {
    type Unsigned = ContractAddress;

    fn build_auth_token_node(unsigned: ContractAddress) -> AuthTokenNode {
        AuthTokenNode {
            consumer_address: ContractAddress::default(),
            payload: encode_payload(&unsigned),
            child_nodes: Default::default(),
        }
    }
}

impl Mintable for crate::types::SHA256Hash {
    type Unsigned = crate::types::SHA256Hash;

    fn build_auth_token_node(unsigned: crate::types::SHA256Hash) -> AuthTokenNode {
        AuthTokenNode {
            consumer_address: ContractAddress::default(),
            payload: encode_payload(&unsigned),
            child_nodes: Default::default(),
        }
    }
}
```

- [ ] **Step 3: Add `Option<T>` impl**

```rust
impl<T: Mintable + WriteXDR + Default> Mintable for Option<T> {
    type Unsigned = Option<T::Unsigned>;

    fn build_auth_token_node(unsigned: Option<T::Unsigned>) -> AuthTokenNode {
        match unsigned {
            Some(inner) => T::build_auth_token_node(inner),
            None => AuthTokenNode {
                consumer_address: ContractAddress::default(),
                payload: encode_payload(&Option::<T>::None),
                child_nodes: Default::default(),
            },
        }
    }
}
```

Note: `Option<T>` as a Mintable is tricky — the Option itself doesn't map cleanly to a single AuthTokenNode. For the initial implementation, consider whether Option<Signer<T>> actually needs to be Mintable (vs just being supported in method params and struct fields via the other traits). If the use case is "optional signer as a struct field that gets minted", the `#[contracttype]` macro handles it field-by-field. The standalone `Option` impl may not be needed. Skip if complex — add only when a test demands it.

- [ ] **Step 4: Add tuple impls**

```rust
macro_rules! impl_mintable_tuple {
    ($(($($name:ident),+)),+ $(,)?) => {
        $(
            impl<$($name: Mintable + WriteXDR + Default),+> Mintable for ($($name,)+) {
                type Unsigned = ($($name::Unsigned,)+);

                #[allow(non_snake_case)]
                fn build_auth_token_node(unsigned: Self::Unsigned) -> AuthTokenNode {
                    let ($($name,)+) = unsigned;
                    // Build child nodes from each element
                    let children: alloc::vec::Vec<AuthTokenNode> = alloc::vec![
                        $($name::build_auth_token_node($name),)+
                    ];
                    // Payload is the tuple with default values (Signers write 0 bytes)
                    let payload_value: Self = Default::default();
                    AuthTokenNode {
                        consumer_address: ContractAddress::default(),
                        payload: encode_payload(&payload_value),
                        child_nodes: auth_token_children(children),
                    }
                }
            }
        )+
    };
}
```

Actually, tuple Mintable impls are complex and may not be needed — tuples are used for calldata args, not for auth token payloads. The `#[contracttype]` structs handle the minting case. **Skip tuple impls for now** — add only if a concrete use case requires them.

- [ ] **Step 5: Verify**

Run: `cargo test --lib mintable -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add src/mintable.rs
git commit -m "feat: add Mintable impls for primitives and common types"
```

---

### Task 3: Generate `Mintable` impls in `#[contracttype]` macro

**Files:**
- Modify: `macros/src/contract_type.rs`

This is the core macro change: for each `#[contracttype]` struct, generate a companion unsigned struct and a `Mintable` impl.

- [ ] **Step 1: Add helper to generate the unsigned companion struct**

In `macros/src/contract_type.rs`, add:

```rust
fn expand_mintable_struct(item: &syn::ItemStruct) -> TokenStream {
    let ident = &item.ident;
    let unsigned_ident = quote::format_ident!("{}Unsigned", ident);

    let unsigned_fields: Vec<TokenStream> = match &item.fields {
        syn::Fields::Named(fields) => fields
            .named
            .iter()
            .map(|f| {
                let name = &f.ident;
                let ty = &f.ty;
                quote! { pub #name: <#ty as ::sdk::Mintable>::Unsigned }
            })
            .collect(),
        _ => {
            return syn::Error::new_spanned(
                item,
                "Mintable is only supported on structs with named fields",
            )
            .into_compile_error();
        }
    };

    // Separate fields into signer and non-signer.
    // The macro doesn't know at syntax level which fields are Signers,
    // so the generated code handles both uniformly:
    // - For each field, call <FieldType as Mintable>::build_auth_token_node
    // - Collect all child nodes
    // - Build payload from a default-constructed Self

    let field_names: Vec<_> = match &item.fields {
        syn::Fields::Named(fields) => fields
            .named
            .iter()
            .map(|f| &f.ident)
            .collect(),
        _ => unreachable!(),
    };

    let field_types: Vec<_> = match &item.fields {
        syn::Fields::Named(fields) => fields
            .named
            .iter()
            .map(|f| &f.ty)
            .collect(),
        _ => unreachable!(),
    };

    // Build child nodes: for each field, build an AuthTokenNode.
    // For non-signer fields, this produces a leaf node (no children).
    // For Signer fields, this produces the subtree.
    let child_builds: Vec<TokenStream> = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(name, ty)| {
            quote! {
                <#ty as ::sdk::Mintable>::build_auth_token_node(unsigned.#name)
            }
        })
        .collect();

    // Construct the payload: build a Self with non-signer values from unsigned,
    // and default Signers (which write 0 bytes in XDR).
    // We can't easily distinguish signer vs non-signer fields at macro level,
    // so we use a different approach: WriteXDR the type where Signer<T> writes
    // 0 bytes. The payload is just the XDR of the type with default signers.
    //
    // Problem: we don't have the original field values to construct Self.
    // The unsigned struct maps field types through Mintable::Unsigned.
    // For non-Signer fields, Unsigned = Self, so unsigned.field IS the value.
    // For Signer fields, Unsigned = UnsignedSigner<...>, not Signer<T>.
    //
    // Solution: construct Self field-by-field. Non-Signer fields: use unsigned.field
    // (since Unsigned = Self for those). Signer fields: use Signer::default().
    // But the macro can't distinguish which is which syntactically.
    //
    // Alternative: use Default::default() for the whole struct payload.
    // Then the payload only has default values. That's wrong — we need the
    // actual non-Signer values.
    //
    // Correct approach: the trait method signature should enable reconstructing
    // the payload. We need a helper that writes the payload from the unsigned data.
    // Let's add a second trait method: encode_payload_from_unsigned.
    //
    // Actually, the simplest approach that works: the generated code knows which
    // fields are the "same type" in Unsigned vs Self. For every field:
    //   If field type T has Unsigned = T (it's not a Signer), we can use the value.
    //   If field type T has Unsigned != T (it IS a Signer), we use T::default().
    // But we can't check this at macro expansion time.
    //
    // The spec says: "Construct a Transfer with the non-Signer values and a default Signer."
    // Since the macro DOES see the field types syntactically, it CAN check if a field
    // type path starts with "Signer". Let's do that.

    let payload_fields: Vec<TokenStream> = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(name, ty)| {
            if is_signer_type(ty) {
                quote! { #name: ::sdk::Signer::default() }
            } else {
                quote! { #name: unsigned.#name }
            }
        })
        .collect();

    // For child nodes: only Signer fields produce meaningful subtrees.
    // Non-signer fields produce leaf nodes that we don't actually need.
    // The spec says children correspond to Signer fields only.
    // So we filter to only Signer fields for children.
    let signer_child_builds: Vec<TokenStream> = field_names
        .iter()
        .zip(field_types.iter())
        .filter(|(_, ty)| is_signer_type(ty))
        .map(|(name, ty)| {
            quote! {
                <#ty as ::sdk::Mintable>::build_auth_token_node(unsigned.#name)
            }
        })
        .collect();

    quote! {
        mod __groundhog_mintable {
            #[allow(unused_imports)]
            use super::*;

            pub struct #unsigned_ident {
                #(#unsigned_fields,)*
            }
        }

        impl ::sdk::Mintable for #ident {
            type Unsigned = __groundhog_mintable::#unsigned_ident;

            fn build_auth_token_node(
                unsigned: Self::Unsigned,
            ) -> ::sdk::AuthTokenNode {
                let children: ::sdk::Vec<::sdk::AuthTokenNode> = ::sdk::vec![
                    #(#signer_child_builds,)*
                ];

                let payload_value = #ident {
                    #(#payload_fields,)*
                };

                ::sdk::AuthTokenNode {
                    consumer_address: ::sdk::ContractAddress::default(),
                    payload: ::sdk::mintable::encode_payload(&payload_value),
                    child_nodes: ::sdk::mintable::auth_token_children(children),
                }
            }
        }
    }
}

fn is_signer_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        type_path
            .path
            .segments
            .last()
            .map(|seg| seg.ident == "Signer")
            .unwrap_or(false)
    } else {
        false
    }
}
```

- [ ] **Step 2: Wire `expand_mintable_struct` into the `expand` function**

In the `Item::Struct` arm of `expand`, add:

```rust
let mintable = expand_mintable_struct(&item);
```

And include `#mintable` in the quote block after `#receive_auth`.

Note: gate behind `#[cfg(feature = "alloc")]` since it uses `Vec` and `vec![]`. Or use `::sdk::Vec` and `::sdk::vec!` which require the `alloc` feature. The examples and tests enable `alloc` via the `dev` feature, so this should work.

- [ ] **Step 3: Add compile-pass test**

Create `tests/compile-pass/signer_mintable.rs`:

```rust
#![allow(unexpected_cfgs)]

use sdk::*;
use sdk::signer::Signer;
use sdk::mintable::{Mintable, UnsignedSigner};

#[contracttype]
pub struct TransferAuth {
    pub amount: u64,
}

#[contracttype]
pub struct Payment {
    pub memo: u32,
    pub auth: Signer<TransferAuth>,
}

fn main() {
    // Verify the Unsigned types exist and are constructible
    let _leaf: <TransferAuth as Mintable>::Unsigned = TransferAuth { amount: 100 };

    let _nested: <Payment as Mintable>::Unsigned =
        __groundhog_mintable::PaymentUnsigned {
            memo: 42,
            auth: UnsignedSigner {
                consumer: ContractAddress::default(),
                data: TransferAuth { amount: 100 },
            },
        };
}
```

Note: the companion struct lives in `__groundhog_mintable` module. The test verifies it's constructible. The type can also be accessed via `<Payment as Mintable>::Unsigned`.

- [ ] **Step 4: Verify**

Run: `cargo test --test compile_pass`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add macros/src/contract_type.rs tests/compile-pass/signer_mintable.rs
git commit -m "feat: #[contracttype] generates Mintable impls with companion unsigned structs"
```

---

### Task 4: Update `mint_auth_token` to use Mintable trait

**Files:**
- Modify: `src/signer.rs`

Replace the current flat `mint_auth_token` with the tree-building version.

- [ ] **Step 1: Replace `mint_auth_token`**

In `src/signer.rs`, replace the existing `mint_auth_token` function:

```rust
pub fn mint_auth_token<T: Mintable>(
    expiry: BlockHeight,
    unsigned: crate::mintable::UnsignedSigner<<T as Mintable>::Unsigned>,
) -> Signer<T> {
    let root_node =
        <Signer<T> as Mintable>::build_auth_token_node(unsigned);

    let buf = contract_call_encode(&root_node).unwrap_or_else(|e| {
        library_throw(AuthTokenError::MintEncodeFailure(
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

The new signature takes `UnsignedSigner` instead of `&T`. It builds the `AuthTokenNode` tree via `Mintable::build_auth_token_node`, serializes it, and calls the syscall.

- [ ] **Step 2: Add `consume` stub**

```rust
impl<T> Signer<T>
where
    T: for<'a> ReadXDR<'a> + crate::ReceiveAuthHandles,
{
    pub fn consume(self) -> T {
        // ConsumeAuthToken syscall not yet available.
        // When it is, this will:
        // 1. Call the syscall to get payload + child handles
        // 2. ReadXDR the payload into T (Signers read as sentinel)
        // 3. ReceiveAuthHandles to fill in child handles
        panic!("ConsumeAuthToken syscall not yet available")
    }
}
```

- [ ] **Step 3: Verify**

Run: `cargo check --features std`
Run: `cargo test --lib`
Run: `cargo test --test compile_pass`

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add src/signer.rs
git commit -m "feat: mint_auth_token builds AuthTokenNode tree via Mintable, add consume stub"
```

---

### Task 5: Update signer example contracts

**Files:**
- Modify: `example-contracts/signer_callee/src/lib.rs`

Add a method that demonstrates minting a signer token.

- [ ] **Step 1: Add a mint example to signer_callee**

Update `example-contracts/signer_callee/src/lib.rs`:

```rust
#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::*;
use sdk::signer::Signer;
use sdk::mintable::{Mintable, UnsignedSigner};

#[contracttype]
pub struct TransferAuth {
    pub amount: u64,
}

#[derive(Default)]
pub struct SignerCallee;

#[contract(storage = false)]
impl SignerCallee {
    pub fn accept_signer(auth: Signer<TransferAuth>) -> u32 {
        auth.handle()
    }

    pub fn accept_mixed(x: u32, auth: Signer<TransferAuth>, y: u32) -> u32 {
        x + y + auth.handle()
    }

    pub fn mint_and_return(
        consumer: &ContractAddress,
        amount: u64,
    ) -> Signer<TransferAuth> {
        let expiry = sdk::block::get_block_height();
        sdk::signer::mint_auth_token::<TransferAuth>(
            expiry,
            UnsignedSigner {
                consumer: consumer.clone(),
                data: TransferAuth { amount },
            },
        )
    }
}
```

- [ ] **Step 2: Verify example compiles**

Run: `cargo check -p signer_callee -p signer_caller --target wasm32-unknown-unknown`

Expected: compiles.

- [ ] **Step 3: Commit**

```bash
cargo fmt
git add example-contracts/signer_callee/src/lib.rs
git commit -m "feat: add mint_and_return example to signer_callee"
```
