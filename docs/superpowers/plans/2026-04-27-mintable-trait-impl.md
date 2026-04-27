# Mintable Trait Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the `Mintable` trait, `UnsignedSigner<T>`, tree-building `mint_auth_token`, and a `consume()` stub, enabling ergonomic auth token minting and typed consuming (minus the actual ConsumeAuthToken syscall).

**Architecture:** `Mintable` trait has an associated `Unsigned` type, a `mint_into_parent` method (handles per-field dispatch without syntactic type detection), and a default `build_auth_token_node` implementation. `UnsignedSigner<T>` pairs a consumer address with unsigned data. The `#[contracttype]` macro generates companion unsigned structs in a private module + `Mintable` impls. No syntactic `is_signer_type` check — each type handles itself via trait dispatch.

**Tech Stack:** Rust, `xdrs` (XDR), `heapless`, `groundhog-xdr` 0.9.2 (`AuthTokenNode`, `AuthTokenPayload`, `kAuthTokenMaxChildNodes`), `groundhog-macros` (proc macros)

**Spec:** `docs/superpowers/specs/2026-04-27-mintable-trait-design.md`

---

### Task 1: Add `UnsignedSigner<T>`, `Mintable` trait, and core impls

**Files:**
- Create: `src/mintable.rs`
- Modify: `src/lib.rs` (add module + re-export)
- Modify: `src/signer.rs` (add Default impl, Mintable impl)
- Modify: `src/types.rs` (re-export auth_token types)

- [ ] **Step 1: Re-export auth_token types from groundhog-xdr**

In `src/types.rs`, add at the end:

```rust
pub use groundhog_xdr::auth_token::*;
```

This makes `AuthTokenNode`, `AuthTokenPayload`, `kAuthTokenMaxChildNodes`, `kAuthTokenMaxLen` available.

- [ ] **Step 2: Create `src/mintable.rs` with the trait, `UnsignedSigner`, and helpers**

```rust
use crate::types::{AuthTokenNode, AuthTokenPayload, ContractAddress};
use crate::xdr::*;
use groundhog_xdr::auth_token::kAuthTokenMaxChildNodes;

pub struct UnsignedSigner<T> {
    pub consumer: ContractAddress,
    pub data: T,
}

pub trait Mintable: Sized + WriteXDR {
    type Unsigned;

    /// Process this value for inclusion in a parent auth token node.
    /// Returns the value to place in the payload struct and pushes
    /// any child AuthTokenNodes into `children`.
    ///
    /// - Primitives: return the value, push nothing
    /// - Signer<T>: push the child subtree, return Signer::default()
    /// - Structs: call mint_into_parent on each field
    fn mint_into_parent(
        unsigned: Self::Unsigned,
        children: &mut alloc::vec::Vec<AuthTokenNode>,
    ) -> Self;

    /// Build a complete AuthTokenNode from unsigned data.
    /// Default impl uses mint_into_parent to collect children
    /// and WriteXDR for the payload.
    fn build_auth_token_node(unsigned: Self::Unsigned) -> AuthTokenNode {
        let mut children = alloc::vec::Vec::new();
        let payload_value = Self::mint_into_parent(unsigned, &mut children);
        AuthTokenNode {
            consumer_address: ContractAddress::default(),
            payload: encode_payload(&payload_value),
            child_nodes: auth_token_children(children),
        }
    }
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
        panic!(
            "too many auth token children (max {})",
            kAuthTokenMaxChildNodes
        )
    })
}
```

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

- [ ] **Step 4: Add `Mintable` impl for `Signer<T>` in `src/signer.rs`**

```rust
use crate::mintable::{Mintable, UnsignedSigner};

impl<T: Mintable> Mintable for Signer<T> {
    type Unsigned = UnsignedSigner<<T as Mintable>::Unsigned>;

    fn mint_into_parent(
        unsigned: UnsignedSigner<<T as Mintable>::Unsigned>,
        children: &mut alloc::vec::Vec<crate::types::AuthTokenNode>,
    ) -> Self {
        let mut node = T::build_auth_token_node(unsigned.data);
        node.consumer_address = unsigned.consumer;
        children.push(node);
        Signer::default()
    }

    fn build_auth_token_node(
        unsigned: UnsignedSigner<<T as Mintable>::Unsigned>,
    ) -> crate::types::AuthTokenNode {
        let mut node = T::build_auth_token_node(unsigned.data);
        node.consumer_address = unsigned.consumer;
        node
    }
}
```

Note: `Signer<T>` overrides `build_auth_token_node` because the default impl would create a wrapper node around the subtree. Instead, `Signer<T>` just delegates to `T` and sets the consumer.

The `WriteXDR` bound on `Mintable` is satisfied because `Signer<T>` already impls `WriteXDR` (writes 0 bytes).

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
    fn primitive_mint_into_parent_no_children() {
        let mut children = alloc::vec::Vec::new();
        let val = u64::mint_into_parent(42u64, &mut children);
        assert_eq!(val, 42);
        assert!(children.is_empty());
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

    #[test]
    fn signer_mint_into_parent_pushes_child() {
        let mut children = alloc::vec::Vec::new();
        let consumer = ContractAddress::default();
        let val = <Signer<u64>>::mint_into_parent(
            UnsignedSigner {
                consumer,
                data: 42u64,
            },
            &mut children,
        );
        assert_eq!(val.handle(), u32::MAX); // default sentinel
        assert_eq!(children.len(), 1);
    }
}
```

- [ ] **Step 7: Verify**

Run: `cargo test --lib mintable::tests -- --nocapture`

Expected: all 4 tests PASS.

- [ ] **Step 8: Commit**

```bash
cargo fmt
git add src/mintable.rs src/signer.rs src/lib.rs src/types.rs
git commit -m "feat: add Mintable trait, UnsignedSigner, and Signer<T> impl"
```

---

### Task 2: Add `Mintable` impls for primitives and common types

**Files:**
- Modify: `src/mintable.rs`

- [ ] **Step 1: Add primitive impls via macro**

Add to `src/mintable.rs` before the tests:

```rust
macro_rules! impl_mintable_leaf {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl Mintable for $ty {
                type Unsigned = $ty;

                fn mint_into_parent(
                    unsigned: $ty,
                    _children: &mut alloc::vec::Vec<AuthTokenNode>,
                ) -> Self {
                    unsigned
                }
            }
        )+
    };
}

impl_mintable_leaf!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, bool, (),);

impl Mintable for ContractAddress {
    type Unsigned = ContractAddress;

    fn mint_into_parent(
        unsigned: ContractAddress,
        _children: &mut alloc::vec::Vec<AuthTokenNode>,
    ) -> Self {
        unsigned
    }
}

impl Mintable for crate::types::SHA256Hash {
    type Unsigned = crate::types::SHA256Hash;

    fn mint_into_parent(
        unsigned: crate::types::SHA256Hash,
        _children: &mut alloc::vec::Vec<AuthTokenNode>,
    ) -> Self {
        unsigned
    }
}
```

- [ ] **Step 2: Verify**

Run: `cargo test --lib mintable -- --nocapture`

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
cargo fmt
git add src/mintable.rs
git commit -m "feat: add Mintable impls for primitives and common types"
```

---

### Task 3: Generate `Mintable` impls in `#[contracttype]` macro

**Files:**
- Modify: `macros/src/contract_type.rs`

The macro generates a companion unsigned struct and a `Mintable` impl. No syntactic type detection — every field dispatches through `<FieldType as Mintable>::mint_into_parent`.

- [ ] **Step 1: Add `expand_mintable_struct` helper**

In `macros/src/contract_type.rs`, add:

```rust
fn expand_mintable_struct(item: &syn::ItemStruct) -> TokenStream {
    let ident = &item.ident;
    let unsigned_ident = quote::format_ident!("{}Unsigned", ident);

    let fields = match &item.fields {
        syn::Fields::Named(fields) => &fields.named,
        _ => {
            return syn::Error::new_spanned(
                item,
                "Mintable derive requires named fields",
            )
            .into_compile_error();
        }
    };

    let unsigned_fields: Vec<TokenStream> = fields
        .iter()
        .map(|f| {
            let name = &f.ident;
            let ty = &f.ty;
            quote! { pub #name: <#ty as ::sdk::Mintable>::Unsigned }
        })
        .collect();

    let mint_fields: Vec<TokenStream> = fields
        .iter()
        .map(|f| {
            let name = &f.ident;
            let ty = &f.ty;
            quote! {
                #name: <#ty as ::sdk::Mintable>::mint_into_parent(
                    unsigned.#name, children,
                )
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

            fn mint_into_parent(
                unsigned: Self::Unsigned,
                children: &mut ::sdk::Vec<::sdk::AuthTokenNode>,
            ) -> Self {
                Self {
                    #(#mint_fields,)*
                }
            }
        }
    }
}
```

The `build_auth_token_node` uses the default impl from the trait — no override needed.

- [ ] **Step 2: Wire into the `expand` function**

In the `Item::Struct` arm, add:

```rust
let mintable = expand_mintable_struct(&item);
```

And include `#mintable` in the quote block after `#receive_auth`.

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
    // Verify types resolve and are constructible via the trait
    let _leaf = <TransferAuth as Mintable>::Unsigned { amount: 100 };

    let _nested = <Payment as Mintable>::Unsigned {
        memo: 42,
        auth: UnsignedSigner {
            consumer: ContractAddress::default(),
            data: <TransferAuth as Mintable>::Unsigned { amount: 100 },
        },
    };

    // Verify build_auth_token_node works
    let _node = <Payment as Mintable>::build_auth_token_node(_nested);
}
```

Note: the companion struct can be accessed via `<Payment as Mintable>::Unsigned { ... }` — users never name the generated struct directly.

- [ ] **Step 4: Verify**

Run: `cargo test --test compile_pass`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add macros/src/contract_type.rs tests/compile-pass/signer_mintable.rs
git commit -m "feat: #[contracttype] generates Mintable impls via trait dispatch"
```

---

### Task 4: Update `mint_auth_token` to use Mintable trait

**Files:**
- Modify: `src/signer.rs`

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

- [ ] **Step 2: Add `consume` stub**

```rust
impl<T> Signer<T>
where
    T: for<'a> ReadXDR<'a> + crate::ReceiveAuthHandles,
{
    pub fn consume(self) -> T {
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

- [ ] **Step 1: Add a mint example**

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
