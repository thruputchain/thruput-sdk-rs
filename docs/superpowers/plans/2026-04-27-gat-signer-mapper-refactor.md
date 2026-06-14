# GAT SignerMapper Refactor Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace generated companion unsigned structs (`__groundhog_mintable_*`) with a GAT-based `SignerMapper` generic on `#[contract_type]` structs, so `MiddleAuth<ForMinting>` replaces `MiddleAuthUnsigned`.

**Architecture:** The `#[contract_type]` macro adds `<M: SignerMapper = Signed>` to structs and wraps each field type in `<M as SignerMapper>::Map<FieldType>`. For `Signed` mode, `Map<T> = T` (identity). For `ForMinting` mode, `Map<T> = T::Unsigned` — which maps `Signer<T>` to `UnsignedSigner<T::Unsigned>` and leaves primitives unchanged. No companion types, no generated module names.

**Tech Stack:** Rust GATs, `xdrs` derive macros, `groundhog-macros` proc macros

**Proven:** The approach compiles — see `gat_derive_test` in `src/mintable.rs` which verifies xdrs derives work with `<M as SignerMapper>::Map<T>` field types.

---

### Task 1: Replace manual XDR impls on UnsignedSigner with derives

**Files:**
- Modify: `src/mintable.rs`

- [ ] **Step 1: Replace manual impls with derives**

In `src/mintable.rs`, replace:

```rust
pub struct UnsignedSigner<T> {
    pub consumer: ContractAddress,
    pub data: T,
}

impl<T: WriteXDR> WriteXDR for UnsignedSigner<T> {
    fn write_xdr<F, W: Writer<F>>(&self, w: &mut W) -> WriteResult<()> {
        self.consumer.write_xdr(w)?;
        self.data.write_xdr(w)
    }
}

impl<'r, T: ReadXDR<'r>> ReadXDR<'r> for UnsignedSigner<T> {
    fn read_xdr<R: Reader<'r>>(r: &mut R) -> ReadResult<Self> {
        Ok(Self {
            consumer: ContractAddress::read_xdr(r)?,
            data: T::read_xdr(r)?,
        })
    }
}
```

With:

```rust
#[derive(ReadXDR, WriteXDR)]
#[xdrs(crate = "crate::__xdrs")]
pub struct UnsignedSigner<T> {
    pub consumer: ContractAddress,
    pub data: T,
}
```

If the xdrs derive doesn't handle the generic `T` correctly (e.g., missing bounds), keep the manual impls. Test by compiling.

- [ ] **Step 2: Remove the temporary `gat_derive_test` module**

Delete the `mod gat_derive_test` block at the bottom of `src/mintable.rs`.

- [ ] **Step 3: Verify**

Run: `cargo test --lib`

Expected: 47 tests pass (minus the removed gat_derive_test, so 46).

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add src/mintable.rs
git commit -m "refactor: derive ReadXDR/WriteXDR on UnsignedSigner, remove GAT test"
```

---

### Task 2: Rewrite `#[contract_type]` struct expansion to use SignerMapper generic

**Files:**
- Modify: `macros/src/contract_type.rs`

This is the core change. The macro currently emits `#item` (the original struct) unchanged. Now it needs to transform the struct by adding a generic parameter and wrapping field types.

- [ ] **Step 1: Replace `expand_mintable_struct` with `transform_struct_for_signer_mapper`**

The new function transforms the `ItemStruct` itself (adding the generic, wrapping fields) and also generates the `Mintable` impl. Replace the entire `expand_mintable_struct` function (and `to_snake_case` helper which is no longer needed) with:

```rust
fn transform_struct_for_signer_mapper(
    item: &syn::ItemStruct,
) -> (TokenStream, TokenStream) {
    let ident = &item.ident;

    let fields = match &item.fields {
        syn::Fields::Named(fields) => &fields.named,
        _ => {
            let err = syn::Error::new_spanned(
                item,
                "SignerMapper transform requires named fields",
            )
            .into_compile_error();
            return (err.clone(), err);
        }
    };

    // Build the transformed struct: each field type wrapped in Map<FieldType>
    let vis = &item.vis;
    let attrs: Vec<_> = item.attrs.iter().collect();
    let transformed_fields: Vec<TokenStream> = fields
        .iter()
        .map(|f| {
            let field_vis = &f.vis;
            let name = &f.ident;
            let ty = &f.ty;
            let field_attrs: Vec<_> = f.attrs.iter().collect();
            quote! {
                #(#field_attrs)*
                #field_vis #name: <__M as ::sdk::mintable::SignerMapper>::Map<#ty>
            }
        })
        .collect();

    let struct_def = quote! {
        #(#attrs)*
        #vis struct #ident<__M: ::sdk::mintable::SignerMapper = ::sdk::mintable::Signed> {
            #(#transformed_fields,)*
        }
    };

    // Build the Mintable impl
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

    let mintable_impl = quote! {
        #[cfg(feature = "alloc")]
        impl ::sdk::Mintable for #ident {
            type Unsigned = #ident<::sdk::mintable::ForMinting>;

            fn mint_into_parent(
                unsigned: Self::Unsigned,
                children: &mut ::sdk::Vec<::sdk::AuthTokenNode>,
            ) -> Self {
                Self {
                    #(#mint_fields,)*
                }
            }
        }
    };

    (struct_def, mintable_impl)
}
```

Key details:
- The generic parameter is named `__M` (double underscore prefix to avoid name collisions with user field names)
- Default is `::sdk::mintable::Signed` so `StructName` without params works as before
- Each field type `T` becomes `<__M as ::sdk::mintable::SignerMapper>::Map<T>`
- The `Mintable` impl uses `type Unsigned = StructName<ForMinting>`
- The struct attributes from the original item are preserved (user attrs like `#[derive(Debug)]`)

- [ ] **Step 2: Update the `expand` function's `Item::Struct` arm**

The current code emits `#item` (original struct) with derive attributes prepended. Now the struct itself is transformed, so the derives go on the transformed struct. Change the `Item::Struct` arm:

```rust
        Item::Struct(item) => {
            if !item.generics.params.is_empty() {
                return syn::Error::new_spanned(
                    &item.generics,
                    "#[contract_type] is not supported on generic types",
                )
                .into_compile_error();
            }
            let ident = &item.ident;
            let name = expand_contract_type_name(ident, &args);
            let collect_auth = expand_collect_auth_handles_struct(&item);
            let receive_auth = expand_receive_auth_handles_struct(&item);
            let (struct_def, mintable) =
                transform_struct_for_signer_mapper(&item);
            let export_attr = if args.export == Some(true) {
                quote! { #[contract_type_spec(export)] }
            } else {
                quote! {}
            };
            quote! {
                #[derive(::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
                #[xdrs(crate = "::sdk::__xdrs")]
                #export_attr
                #struct_def

                #name

                #collect_auth

                #receive_auth

                #mintable
            }
        }
```

Note: `#struct_def` replaces `#item`. The derives are prepended to the transformed struct.

- [ ] **Step 3: Delete `to_snake_case` and old `expand_mintable_struct`**

Remove both functions entirely — they're replaced by `transform_struct_for_signer_mapper`.

- [ ] **Step 4: Verify SDK compiles**

Run: `cargo check --features std`

Expected: compiles (existing example contracts may have errors — those are fixed in Task 3).

Run: `cargo test --lib`

Expected: all lib tests pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add macros/src/contract_type.rs
git commit -m "refactor: #[contract_type] adds SignerMapper generic, removes companion structs"
```

---

### Task 3: Update tests and compile-pass files

**Files:**
- Modify: `tests/compile-pass/signer_mintable.rs`
- Modify: `tests/compile-pass/signer_in_contract_type.rs`
- Modify: `tests/signer_lifecycle.rs`

- [ ] **Step 1: Update `tests/compile-pass/signer_mintable.rs`**

Replace the entire file:

```rust
#![allow(unexpected_cfgs)]

use sdk::*;
use sdk::signer::Signer;
use sdk::mintable::{Mintable, UnsignedSigner, ForMinting};

#[contract_type]
pub struct TransferAuth {
    pub amount: u64,
}

#[contract_type]
pub struct Payment {
    pub memo: u32,
    pub auth: Signer<TransferAuth>,
}

fn main() {
    // ForMinting mode: same struct name, different generic parameter
    let unsigned: Payment<ForMinting> = Payment {
        memo: 42,
        auth: UnsignedSigner {
            consumer: ContractAddress::default(),
            data: TransferAuth { amount: 100 },
        },
    };

    let _node = <Payment as Mintable>::build_auth_token_node(unsigned);
}
```

Note: `TransferAuth` has no Signer fields, so `TransferAuth<ForMinting>` has the same field types as `TransferAuth<Signed>`. The user constructs it with `TransferAuth { amount: 100 }` — Rust infers the `ForMinting` mode from context.

- [ ] **Step 2: Update `tests/signer_lifecycle.rs`**

Replace all references to `__groundhog_mintable_*::*Unsigned` with `StructName<ForMinting>`. The struct construction syntax is the same struct name with `ForMinting` type parameter.

Replace all `__groundhog_mintable_middle_auth::MiddleAuthUnsigned { ... }` with `MiddleAuth::<ForMinting> { ... }`.

Replace all `__groundhog_mintable_leaf_auth::LeafAuthUnsigned { ... }` with `LeafAuth { ... }` (type inferred from context since LeafAuth has no Signers — ForMinting and Signed resolve to the same field types).

Add `use sdk::mintable::ForMinting;` to imports.

- [ ] **Step 3: Verify tests**

Run: `cargo test --lib`
Run: `cargo test --test signer_lifecycle`
Run: `cargo test --test compile_pass`

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add tests/compile-pass/signer_mintable.rs tests/signer_lifecycle.rs
git commit -m "test: update tests for SignerMapper GAT approach"
```

---

### Task 4: Update example contracts

**Files:**
- Modify: `example-contracts/signer_minter/src/lib.rs`
- Modify: `example-contracts/signer_middle/src/lib.rs` (may need `ForMinting` if MiddleAuth is constructed in unsigned form)

- [ ] **Step 1: Update `signer_minter/src/lib.rs`**

Replace the file. Key changes:
- Remove imports of `__groundhog_mintable_*` modules
- Use `MiddleAuth::<ForMinting>` instead of `MiddleAuthUnsigned`
- Use `LeafAuth { ... }` directly (no Signer fields, so ForMinting = Signed for field types)
- Add `use sdk::mintable::ForMinting;`

```rust
#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::mintable::{ForMinting, UnsignedSigner};
use sdk::signer::Signer;
use sdk::*;
use signer_leaf::LeafAuth;
use signer_middle::{MiddleAuth, SignerMiddleClient};

#[derive(Default)]
pub struct SignerMinter;

#[contract(storage = false)]
impl SignerMinter {
    pub fn mint_and_call(
        middle_contract: &ContractAddress,
        leaf_contract: &ContractAddress,
    ) -> u64 {
        let expiry = sdk::block::get_block_height();

        let unsigned = UnsignedSigner {
            consumer: middle_contract.clone(),
            data: MiddleAuth::<ForMinting> {
                label: 1000,
                child: UnsignedSigner {
                    consumer: leaf_contract.clone(),
                    data: LeafAuth { data: 10 },
                },
                extra_children: vec![
                    UnsignedSigner {
                        consumer: leaf_contract.clone(),
                        data: LeafAuth { data: 20 },
                    },
                    UnsignedSigner {
                        consumer: leaf_contract.clone(),
                        data: LeafAuth { data: 30 },
                    },
                ],
            },
        };

        let signer: Signer<MiddleAuth> =
            sdk::signer::mint_auth_token::<MiddleAuth>(expiry, unsigned);

        SignerMiddleClient::new(middle_contract)
            .consume_and_forward(signer, leaf_contract)
    }
}
```

- [ ] **Step 2: Verify examples compile**

Run (from example-contracts dir): `cargo check -p signer_leaf -p signer_middle -p signer_minter --target wasm32-unknown-unknown`

Expected: compiles.

- [ ] **Step 3: Run the lifecycle test**

Run (from example-contracts dir): `cargo test -p signer_minter --test lifecycle`

Expected: PASS (result = 1060).

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add example-contracts/signer_minter/src/lib.rs
git commit -m "refactor: signer_minter uses MiddleAuth<ForMinting> instead of companion types"
```
