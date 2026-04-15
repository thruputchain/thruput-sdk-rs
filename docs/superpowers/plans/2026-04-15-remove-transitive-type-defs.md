# Remove Transitive TYPE_DEFS Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate the transitive compile-time `TYPE_DEFS` closure so each `#[contracttype]` emits only its own typedef fragment, enabling recursive types and removing O(n^2) WASM bloat.

**Architecture:** Delete `ContractTypeDefsView`, `TYPE_DEFS`, `ContractTypeInfo::type_defs`, and all method-level typedef emission. Each `#[contracttype]` self-registers via `emit_raw_fragment_tokens_with_cfg`. The spec tool's existing `resolve_reachable_type_defs` handles transitive discovery. Add `Box<T>` blanket impl for recursive type support.

**Tech Stack:** Rust proc macros (`syn`/`quote`/`darling`), `groundhog-contract-spec`, `groundhog-codegen`, `groundhog-macros`, `groundhog-spec-tool`

**Spec:** `docs/superpowers/specs/2026-04-15-remove-transitive-type-defs.md`

---

### Task 1: Strip `type_defs` from `ContractTypeInfo` and all `ContractTypeRefSpec` impls

This is the core change. Remove the `type_defs` field from `ContractTypeInfo`, delete `ContractTypeDefsView`, delete `TYPE_DEFS` from `ContractTypeSpec`, delete the `type_defs()` helper, and delete the `const_txs_typedef_fragment_closure_*` functions. Add `Box<T>` impl.

**Files:**
- Modify: `groundhog-contract-spec/src/lib.rs`
- Modify: `groundhog-contract-spec/src/view.rs`
- Modify: `groundhog-contract-spec/src/groundhog_xdr_types.rs`

- [ ] **Step 1: Delete `ContractTypeDefsView` from `view.rs`**

In `groundhog-contract-spec/src/view.rs`, delete the `ContractTypeDefsView` enum (lines 99-104):

```rust
// DELETE this entire block:
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractTypeDefsView<'a> {
    Empty,
    One(ContractTypeDefView<'a>),
    Many(&'a [ContractTypeDefsView<'a>]),
}
```

- [ ] **Step 2: Simplify `ContractTypeSpec` trait in `lib.rs`**

In `groundhog-contract-spec/src/lib.rs`, remove `TYPE_DEFS` from the trait (lines 26-33). Before:

```rust
pub trait ContractTypeSpec {
    const TYPE_DEF: view::ContractTypeDefView<'static>;
    const TYPE_DEFS: view::ContractTypeDefsView<'static> =
        view::ContractTypeDefsView::One(Self::TYPE_DEF);
}
```

After:

```rust
/// Implemented by `#[contracttype]` types to provide their spec type definition.
pub trait ContractTypeSpec {
    /// The type's definition (struct, enum, or union) as a const view.
    const TYPE_DEF: view::ContractTypeDefView<'static>;
}
```

- [ ] **Step 3: Simplify `ContractTypeInfo` in `lib.rs`**

Remove the `type_defs` field (lines 41-48). Before:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractTypeInfo<'a> {
    pub ty: view::ContractTypeView<'a>,
    pub type_defs: view::ContractTypeDefsView<'a>,
}
```

After:

```rust
/// The spec type reference for a type (e.g. `U32`, `Named("Foo")`, `Option(&U64)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractTypeInfo<'a> {
    pub ty: view::ContractTypeView<'a>,
}
```

- [ ] **Step 4: Update all primitive `ContractTypeRefSpec` impls**

Remove the `type_defs` field from every primitive impl. For example, the `()` impl (lines 57-62) changes from:

```rust
impl ContractTypeRefSpec for () {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Void,
        type_defs: view::ContractTypeDefsView::Empty,
    };
}
```

To:

```rust
impl ContractTypeRefSpec for () {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Void,
    };
}
```

Apply the same change to all primitive impls: `bool`, `u8`, `u32`, `i32`, `u64`, `i64`, `u128`, `i128`, `str`, `[u8]`, `&[u8]`, `&str`, `String`.

- [ ] **Step 5: Update generic `ContractTypeRefSpec` impls**

Remove `type_defs` from `[T; N]`, `Vec<T>`, `VecM<T, N>`, `Option<T>`, and the blanket `T: ContractTypeSpec` impl.

The `[T; N]` impl (lines 146-157) changes from:

```rust
impl<T, const N: usize> ContractTypeRefSpec for [T; N]
where
    T: ContractTypeRefSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Array(view::ContractArrayTypeView {
            element: Some(&type_ref::<T>()),
            len: N as u32,
        }),
        type_defs: type_defs::<T>(),
    };
}
```

To:

```rust
impl<T, const N: usize> ContractTypeRefSpec for [T; N]
where
    T: ContractTypeRefSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Array(view::ContractArrayTypeView {
            element: Some(&type_ref::<T>()),
            len: N as u32,
        }),
    };
}
```

Same pattern for `Vec<T>`, `VecM<T, N>`, `Option<T>`.

The blanket `T: ContractTypeSpec` impl (lines 205-213) changes from:

```rust
impl<T> ContractTypeRefSpec for T
where
    T: ContractTypeSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: T::TYPE_DEF.type_ref(),
        type_defs: T::TYPE_DEFS,
    };
}
```

To:

```rust
impl<T> ContractTypeRefSpec for T
where
    T: ContractTypeSpec,
{
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: T::TYPE_DEF.type_ref(),
    };
}
```

- [ ] **Step 6: Update tuple macro**

The `impl_tuple_type_ref_spec!` macro (lines 215-228) changes from:

```rust
macro_rules! impl_tuple_type_ref_spec {
    ($(($($name:ident),+)),+ $(,)?) => {
        $(
            impl<$($name),+> ContractTypeRefSpec for ($($name,)+)
            where
                $($name: ContractTypeRefSpec),+
            {
                const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
                    ty: view::ContractTypeView::Tuple(&[$(type_ref::<$name>()),+]),
                    type_defs: view::ContractTypeDefsView::Many(&[$(type_defs::<$name>()),+]),
                };
            }
        )+
    };
}
```

To:

```rust
macro_rules! impl_tuple_type_ref_spec {
    ($(($($name:ident),+)),+ $(,)?) => {
        $(
            impl<$($name),+> ContractTypeRefSpec for ($($name,)+)
            where
                $($name: ContractTypeRefSpec),+
            {
                const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
                    ty: view::ContractTypeView::Tuple(&[$(type_ref::<$name>()),+]),
                };
            }
        )+
    };
}
```

- [ ] **Step 7: Delete `type_defs()` helper and closure functions**

Delete the `type_defs()` function (lines 261-265) and all three `const_txs_typedef_fragment_closure_*` functions (lines 267-315).

- [ ] **Step 8: Add `Box<T>` impl**

Add after the `&str` impl (after line 144):

```rust
#[cfg(feature = "alloc")]
impl<T: ContractTypeRefSpec> ContractTypeRefSpec for Box<T> {
    const INFO: ContractTypeInfo<'static> = T::INFO;
}
```

Note: requires adding `use alloc::boxed::Box;` to the `#[cfg(feature = "alloc")]` imports at the top of the file (line 6). Change:

```rust
use alloc::{string::String, vec::Vec};
```

To:

```rust
use alloc::{boxed::Box, string::String, vec::Vec};
```

- [ ] **Step 9: Update `groundhog_xdr_types.rs`**

Remove `type_defs` from all 6 impls in `groundhog-contract-spec/src/groundhog_xdr_types.rs`. Remove the `view` import line too since `ContractTypeDefsView` is gone. For example, the `ContractAddress` impl changes from:

```rust
impl ContractTypeRefSpec for ContractAddress {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Address,
        type_defs: view::ContractTypeDefsView::Empty,
    };
}
```

To:

```rust
impl ContractTypeRefSpec for ContractAddress {
    const INFO: ContractTypeInfo<'static> = ContractTypeInfo {
        ty: view::ContractTypeView::Address,
    };
}
```

Apply to all 6 impls: `ContractAddress`, `BlockHeight`, `SHA256Hash`, `Ed25519PublicKey`, `Ed25519Signature`, `ContractEntrypointID`.

- [ ] **Step 10: Build to verify**

Run: `cargo check -p groundhog-contract-spec`

Expected: Compilation succeeds (tests may fail, that's Task 2).

- [ ] **Step 11: Commit**

```bash
git add groundhog-contract-spec/src/lib.rs groundhog-contract-spec/src/view.rs groundhog-contract-spec/src/groundhog_xdr_types.rs
git commit -m "Remove TYPE_DEFS, ContractTypeDefsView, and type_defs from contract spec

Each #[contracttype] will emit only its own typedef fragment.
The transitive closure machinery is no longer needed since the
spec tool's reachability walk handles transitive discovery.
Also adds Box<T> blanket impl for recursive type support."
```

---

### Task 2: Update `groundhog-contract-spec` tests

**Files:**
- Modify: `groundhog-contract-spec/tests/roundtrip.rs`

- [ ] **Step 1: Delete `typedef_fragment_closure_helpers_match_runtime_xdr` test**

Delete the entire test at lines 414-472. It tests the deleted `const_txs_typedef_fragment_closure_*` functions.

- [ ] **Step 2: Update `contract_type_spec_helpers_are_derived_from_type_def` test**

The test at lines 590-620 needs the `TYPE_DEFS` assertion removed. Replace the test with:

```rust
#[test]
fn contract_type_spec_helpers_are_derived_from_type_def() {
    struct Payload;

    impl ContractTypeSpec for Payload {
        const TYPE_DEF: ContractTypeDefView<'static> =
            ContractTypeDefView::Struct(ContractStructDefView {
                name: "Payload",
                fields: &[ContractFieldView {
                    name: "count",
                    ty: ContractTypeView::U32,
                }],
            });
    }

    assert_eq!(Payload::TYPE_DEF.symbol(), "Payload");
    assert_eq!(groundhog_contract_spec::symbol::<Payload>(), "Payload");
    assert!(matches!(
        groundhog_contract_spec::type_ref::<Payload>(),
        ContractTypeView::Named("Payload")
    ));
    assert!(matches!(Payload::TYPE_DEF, ContractTypeDefView::Struct(_)));
    assert_eq!(
        groundhog_contract_spec::type_ref::<Payload>(),
        Payload::TYPE_DEF.type_ref()
    );
}
```

- [ ] **Step 3: Update `contract_type_ref_helpers_cover_primitives_and_composites` test**

Remove all `type_defs` assertions (lines 682-716). Keep only the `type_ref` assertions (lines 623-681).

- [ ] **Step 4: Update `alias_byte_collections_resolve_via_trait_layer` test**

Remove the `type_defs` assertions at lines 739-746. Keep only the `type_ref` assertions.

- [ ] **Step 5: Update `contract_type_ref_helpers_cover_groundhog_xdr_builtins` test**

Remove the `type_defs` assertions at lines 762-772. Keep only the `type_ref` assertions.

- [ ] **Step 6: Delete `alias_composites_forward_nested_typedef_closure` test**

Delete the entire test at lines 775-820. It exists solely to test transitive typedef closure serialization.

- [ ] **Step 7: Remove unused imports in `roundtrip.rs`**

Remove `ContractTypeDefsView` from the import on line 8. Also remove any now-unused items.

- [ ] **Step 8: Run tests**

Run: `cargo test -p groundhog-contract-spec`

Expected: All tests pass.

- [ ] **Step 9: Commit**

```bash
git add groundhog-contract-spec/tests/roundtrip.rs
git commit -m "Update contract-spec tests for TYPE_DEFS removal"
```

---

### Task 3: Simplify `derive_contract_type_spec.rs` (macro crate)

Remove transitive type_defs generation from the `#[derive(ContractTypeSpec)]` macro. Simplify `emit_raw_typedef_fragment` to emit a single fragment directly.

**Files:**
- Modify: `macros/src/derive_contract_type_spec.rs`

- [ ] **Step 1: Simplify `expand()` function**

Remove the `type_defs` variable and `TYPE_DEFS` const from the generated impl. The `expand()` function (lines 8-45) changes from:

```rust
pub fn expand(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "ContractTypeSpec cannot be derived for generic types",
        ));
    }

    let (type_def, type_defs) = match &input.data {
        syn::Data::Struct(data) => (
            struct_type_def_tokens(ident, &data.fields)?,
            struct_type_defs_tokens(&data.fields)?,
        ),
        syn::Data::Enum(data) => (
            enum_type_def_tokens(ident, &data.variants)?,
            enum_type_defs_tokens(&data.variants)?,
        ),
        syn::Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                ident,
                "ContractTypeSpec cannot be derived for unions",
            ));
        }
    };

    let raw_typedef = emit_raw_typedef_fragment(ident);

    Ok(quote! {
        impl ::sdk::__contract_spec::ContractTypeSpec for #ident {
            const TYPE_DEF: ::sdk::__contract_spec::ContractTypeDefView<'static> = #type_def;
            const TYPE_DEFS: ::sdk::__contract_spec::ContractTypeDefsView<'static> = #type_defs;
        }

        #raw_typedef
    })
}
```

To:

```rust
pub fn expand(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "ContractTypeSpec cannot be derived for generic types",
        ));
    }

    let type_def = match &input.data {
        syn::Data::Struct(data) => {
            struct_type_def_tokens(ident, &data.fields)?
        }
        syn::Data::Enum(data) => {
            enum_type_def_tokens(ident, &data.variants)?
        }
        syn::Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                ident,
                "ContractTypeSpec cannot be derived for unions",
            ));
        }
    };

    let raw_typedef = emit_raw_typedef_fragment(ident);

    Ok(quote! {
        impl ::sdk::__contract_spec::ContractTypeSpec for #ident {
            const TYPE_DEF: ::sdk::__contract_spec::ContractTypeDefView<'static> = #type_def;
        }

        #raw_typedef
    })
}
```

- [ ] **Step 2: Simplify `emit_raw_typedef_fragment` to use `emit_raw_fragment_tokens_with_cfg`**

Replace the function (lines 47-52) from:

```rust
fn emit_raw_typedef_fragment(ident: &syn::Ident) -> TokenStream {
    codegen::spec_reg::emit_raw_typedef_fragment_stream_tokens_with_cfg(
        quote! { target_family = "wasm" },
        quote! { <#ident as ::sdk::__contract_spec::ContractTypeSpec>::TYPE_DEFS },
    )
}
```

To:

```rust
fn emit_raw_typedef_fragment(ident: &syn::Ident) -> TokenStream {
    codegen::spec_reg::emit_raw_fragment_tokens_with_cfg(
        quote! { target_family = "wasm" },
        quote! {
            ::sdk::__contract_spec::SpecFragmentView::TypeDef(
                <#ident as ::sdk::__contract_spec::ContractTypeSpec>::TYPE_DEF
            )
        },
    )
}
```

- [ ] **Step 3: Delete `struct_type_defs_tokens`, `enum_type_defs_tokens`, `enum_variant_type_defs_tokens`, `fields_type_defs_tokens`**

Delete these four functions (lines 69-290):
- `struct_type_defs_tokens` (lines 69-77)
- `enum_type_defs_tokens` (lines 139-153)
- `enum_variant_type_defs_tokens` (lines 254-268)
- `fields_type_defs_tokens` (lines 270-290)

- [ ] **Step 4: Build to verify**

Run: `cargo check -p groundhog-macros`

Expected: Compilation succeeds.

- [ ] **Step 5: Commit**

```bash
git add macros/src/derive_contract_type_spec.rs
git commit -m "Simplify ContractTypeSpec derive: emit single typedef fragment

Remove transitive type_defs generation. Each #[contracttype]
now emits exactly one SpecFragmentView::TypeDef for itself."
```

---

### Task 4: Remove typedef emission from `#[contract]` codegen

Remove all method-level and throws-level typedef fragment emission from the `#[contract]` macro's codegen.

**Files:**
- Modify: `gen/src/spec_reg.rs`
- Modify: `gen/src/contract.rs`
- Modify: `macros/src/contract.rs`

- [ ] **Step 1: Simplify `generate_impl_raw_spec_fragments` in `spec_reg.rs`**

Remove `trait_throw_typedef_fragments`, `typedef_fragments`, and the method typedef loop. The function (lines 11-77) changes from:

```rust
pub fn generate_impl_raw_spec_fragments(
    item: &syn::ItemImpl,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), item);
    let self_ty = &item.self_ty;
    let methods = contract.methods_sorted().into_iter().collect::<Vec<_>>();

    let (iface_name, iface_vers, iface_throws) = if let Some((
        _,
        trait_path,
        _,
    )) = &item.trait_
    {
        (
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_NAME },
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_FULL_VERSION },
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_THROWS },
        )
    } else {
        let iface_name = effective_interface_name(&contract);
        let iface_vers = contract.defaulted_interface_vers();
        let throws = throws_spec_tokens(&contract)?;
        (
            quote! { #iface_name },
            quote! { #iface_vers },
            quote! { &[#(#throws),*] },
        )
    };

    let interface_fragment = generate_interface_fragment_tokens(
        iface_name.clone(),
        iface_vers.clone(),
        iface_throws,
    );
    let trait_throw_typedef_fragments =
        item.trait_.as_ref().map(|(_, trait_path, _)| {
            emit_raw_typedef_fragment_stream_tokens(quote! {
                <#self_ty as #trait_path>::__GROUNDHOG_IFACE_THROW_TYPE_DEFS
            })
        });
    let mut typedef_fragments = contract
        .throws_list()
        .into_iter()
        .map(|thrown| typedef_fragment_tokens_for_type_path(thrown.ty()))
        .collect::<Result<Vec<_>>>()?;
    let method_fragments = methods
        .iter()
        .map(|method| {
            generate_method_fragment_tokens(
                iface_name.clone(),
                iface_vers.clone(),
                method,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    for method in &methods {
        typedef_fragments.extend(typedef_fragment_tokens_for_method(method)?);
    }

    Ok(quote! {
        #interface_fragment
        #(#method_fragments)*
        #trait_throw_typedef_fragments
        #(#typedef_fragments)*
    })
}
```

To:

```rust
pub fn generate_impl_raw_spec_fragments(
    item: &syn::ItemImpl,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), item);
    let self_ty = &item.self_ty;
    let methods = contract.methods_sorted().into_iter().collect::<Vec<_>>();

    let (iface_name, iface_vers, iface_throws) = if let Some((
        _,
        trait_path,
        _,
    )) = &item.trait_
    {
        (
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_NAME },
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_FULL_VERSION },
            quote! { <#self_ty as #trait_path>::__GROUNDHOG_IFACE_THROWS },
        )
    } else {
        let iface_name = effective_interface_name(&contract);
        let iface_vers = contract.defaulted_interface_vers();
        let throws = throws_spec_tokens(&contract)?;
        (
            quote! { #iface_name },
            quote! { #iface_vers },
            quote! { &[#(#throws),*] },
        )
    };

    let interface_fragment = generate_interface_fragment_tokens(
        iface_name.clone(),
        iface_vers.clone(),
        iface_throws,
    );
    let method_fragments = methods
        .iter()
        .map(|method| {
            generate_method_fragment_tokens(
                iface_name.clone(),
                iface_vers.clone(),
                method,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        #interface_fragment
        #(#method_fragments)*
    })
}
```

- [ ] **Step 2: Simplify `generate_free_function_raw_spec_fragments` in `spec_reg.rs`**

Same pattern. The function (lines 79-110) changes from:

```rust
pub fn generate_free_function_raw_spec_fragments(
    item: &syn::ItemFn,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), item);
    let iface_name = effective_interface_name(&contract);
    let iface_vers = contract.defaulted_interface_vers();
    let throws = throws_spec_tokens(&contract)?;

    let interface_fragment = generate_interface_fragment_tokens(
        quote! { #iface_name },
        quote! { #iface_vers },
        quote! { &[#(#throws),*] },
    );
    let method_fragment = generate_method_fragment_tokens(
        quote! { #iface_name },
        quote! { #iface_vers },
        item,
    )?;
    let mut typedef_fragments = contract
        .throws_list()
        .into_iter()
        .map(|thrown| typedef_fragment_tokens_for_type_path(thrown.ty()))
        .collect::<Result<Vec<_>>>()?;
    typedef_fragments.extend(typedef_fragment_tokens_for_method(item)?);

    Ok(quote! {
        #interface_fragment
        #method_fragment
        #(#typedef_fragments)*
    })
}
```

To:

```rust
pub fn generate_free_function_raw_spec_fragments(
    item: &syn::ItemFn,
    config: &Config,
) -> Result<TokenStream> {
    let contract = ConfiguredContract::new(config.clone(), item);
    let iface_name = effective_interface_name(&contract);
    let iface_vers = contract.defaulted_interface_vers();
    let throws = throws_spec_tokens(&contract)?;

    let interface_fragment = generate_interface_fragment_tokens(
        quote! { #iface_name },
        quote! { #iface_vers },
        quote! { &[#(#throws),*] },
    );
    let method_fragment = generate_method_fragment_tokens(
        quote! { #iface_name },
        quote! { #iface_vers },
        item,
    )?;

    Ok(quote! {
        #interface_fragment
        #method_fragment
    })
}
```

- [ ] **Step 3: Delete dead functions from `spec_reg.rs`**

Delete these functions (they are now unused):
- `emit_raw_typedef_fragment_stream_tokens` (lines 183-190)
- `emit_raw_typedef_fragment_stream_tokens_with_cfg` (lines 192-210)
- `typedef_fragment_tokens_for_method` (lines 212-230)
- `typedef_fragment_tokens_for_type_path` (lines 232-242)
- `typedef_fragment_tokens_for_type` (lines 244-250)

Also remove the now-unused import of `contract_type_typedef_tokens` from the `use` statement at the top of the file (line 3).

- [ ] **Step 4: Delete `throws_typedef_tokens` and `contract_type_typedef_tokens` from `contract.rs`**

In `gen/src/contract.rs`:

Delete `throws_typedef_tokens` (lines 394-406):

```rust
// DELETE
pub fn throws_typedef_tokens(
    contract: &impl ContractSrc,
) -> Result<Vec<TokenStream>> {
    contract
        .throws_list()
        .into_iter()
        .map(|thrown| {
            let path = thrown.ty();
            let ty: syn::Type = syn::parse_quote! { #path };
            Ok(lower_contract_type(&ty)?.type_defs)
        })
        .collect()
}
```

Delete `contract_type_typedef_tokens` (lines 501-503):

```rust
// DELETE
pub fn contract_type_typedef_tokens(ty: &syn::Type) -> Result<TokenStream> {
    Ok(lower_contract_type(ty)?.type_defs)
}
```

Remove the `type_defs` field from `LoweredContractType` (lines 455-458) and its assignment in `lower_via_type_ref_trait` (lines 460-469). Since the struct now has only one field, replace it. Before:

```rust
pub struct LoweredContractType {
    pub view: TokenStream,
    pub type_defs: TokenStream,
}

fn lower_via_type_ref_trait(ty: &syn::Type) -> LoweredContractType {
    LoweredContractType {
        view: quote! {
            ::sdk::__contract_spec::type_ref::<#ty>()
        },
        type_defs: quote! {
            ::sdk::__contract_spec::type_defs::<#ty>()
        },
    }
}
```

After:

```rust
fn lower_via_type_ref_trait(ty: &syn::Type) -> TokenStream {
    quote! {
        ::sdk::__contract_spec::type_ref::<#ty>()
    }
}
```

Then update `lower_contract_type` and `contract_type_view_tokens` to return `TokenStream` directly instead of `LoweredContractType`. `lower_contract_type` (lines 471-493) changes from:

```rust
pub fn lower_contract_type(ty: &syn::Type) -> Result<LoweredContractType> {
```

To:

```rust
pub fn contract_type_view_tokens(ty: &syn::Type) -> Result<TokenStream> {
```

And merge the two functions. The current `contract_type_view_tokens` (lines 495-498) just unwraps `.view`:

```rust
pub fn contract_type_view_tokens(
    ty: &syn::Type,
) -> Result<proc_macro2::TokenStream> {
    Ok(lower_contract_type(ty)?.view)
}
```

After merging, delete `LoweredContractType` struct, delete the separate `contract_type_view_tokens` wrapper, and rename `lower_contract_type` to `contract_type_view_tokens`:

```rust
pub fn contract_type_view_tokens(ty: &syn::Type) -> Result<TokenStream> {
    match ty {
        syn::Type::Reference(reference) => {
            contract_type_view_tokens(&reference.elem)
        }
        syn::Type::Path(_) => Ok(lower_via_type_ref_trait(ty)),
        syn::Type::Paren(paren) => contract_type_view_tokens(&paren.elem),
        syn::Type::Slice(_) | syn::Type::Array(_) | syn::Type::Tuple(_) => {
            Ok(lower_via_type_ref_trait(ty))
        }
        syn::Type::ImplTrait(_)
        | syn::Type::TraitObject(_)
        | syn::Type::BareFn(_)
        | syn::Type::Ptr(_) => Err(error(
            ty,
            "unsupported contract spec type; supported shapes are named types, tuples, arrays, options, vecs, and references to supported types",
        )),
        _ => Err(error(
            ty,
            "unsupported contract spec type; supported shapes are named types, tuples, arrays, options, vecs, and references to supported types",
        )),
    }
}
```

- [ ] **Step 5: Remove `__GROUNDHOG_IFACE_THROW_TYPE_DEFS` from trait expansion in `macros/src/contract.rs`**

Delete the `throws_typedef_tokens` variable (lines 41-42) and the hidden const that uses it (lines 76-80). Before:

```rust
            let throws_tokens =
                codegen::contract::throws_spec_tokens(&contract)?;
            let throws_typedef_tokens =
                codegen::contract::throws_typedef_tokens(&contract)?;
            // ...
            stripped.items.push(parse_quote! {
                #[doc(hidden)]
                const __GROUNDHOG_IFACE_THROW_TYPE_DEFS: ::sdk::__contract_spec::ContractTypeDefsView<'static> =
                    ::sdk::__contract_spec::ContractTypeDefsView::Many(&[#(#throws_typedef_tokens),*]);
            });
```

After (just delete the two blocks, keep `throws_tokens`):

```rust
            let throws_tokens =
                codegen::contract::throws_spec_tokens(&contract)?;
```

- [ ] **Step 6: Build the workspace**

Run: `cargo check`

Expected: Compilation succeeds across the workspace.

- [ ] **Step 7: Commit**

```bash
git add gen/src/spec_reg.rs gen/src/contract.rs macros/src/contract.rs
git commit -m "Remove method-level typedef emission from #[contract] codegen

Typedef fragments now come exclusively from #[contracttype].
The #[contract] macro only emits interface and method fragments."
```

---

### Task 5: Update `gen/src/spec_reg.rs` tests

**Files:**
- Modify: `gen/src/spec_reg.rs` (test module at bottom)

- [ ] **Step 1: Update test assertions**

The tests in `spec_reg.rs` (lines 252-462) check generated token streams. Several assert on `type_defs` or `maybe_type_def` in the output. Update them:

- `byte_slices_and_vecs_do_not_emit_u8_typedef_fragments` (line 311): Delete this test entirely -- it tested that method-level typedef emission didn't emit for `u8`, which no longer exists.
- `byte_composite_shapes_lower_without_trait_specialization` (line 329): Remove the `type_defs` assertions (lines 362-364). Keep the `type_ref` assertions.
- `alias_byte_collections_emit_trait_calls` (line 392): Remove `type_defs` assertions (lines 413-414). Keep `type_ref` assertions.
- `builtin_byte_blob_types_use_trait_typedef_resolution` (line 419): Delete this test entirely -- it only asserted on `type_defs` in method fragment output.
- `block_height_uses_trait_typedef_resolution` (line 444): Delete this test entirely -- same reason.

- [ ] **Step 2: Run tests**

Run: `cargo test -p groundhog-codegen`

Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add gen/src/spec_reg.rs
git commit -m "Update spec_reg tests for typedef emission removal"
```

---

### Task 6: Update `gen/src/contract.rs` tests

**Files:**
- Modify: `gen/src/contract.rs` (test module at bottom)

- [ ] **Step 1: Update tests that reference `contract_type_typedef_tokens`**

The tests in `gen/src/contract.rs` that reference `contract_type_typedef_tokens` or check `type_defs` in token output need updating:

- `alias_paths_use_trait_resolution_for_spec_typedefs` (line 520): Delete this test -- it tested the deleted `contract_type_typedef_tokens` function.
- Any other test that imports or calls `contract_type_typedef_tokens`: delete or update.

Check for references to `lower_contract_type` in tests and update to use `contract_type_view_tokens` directly.

- [ ] **Step 2: Run tests**

Run: `cargo test -p groundhog-codegen`

Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add gen/src/contract.rs
git commit -m "Update contract.rs tests for LoweredContractType removal"
```

---

### Task 7: Add `recursive_type` example contract

Create a new example contract that proves recursive types now work.

**Files:**
- Create: `example-contracts/recursive_type/Cargo.toml`
- Create: `example-contracts/recursive_type/src/lib.rs`
- Create: `example-contracts/recursive_type/src/implementation.rs`
- Modify: `example-contracts/Cargo.toml` (add to workspace members)

- [ ] **Step 1: Create `Cargo.toml`**

Create `example-contracts/recursive_type/Cargo.toml`:

```toml
[package]
name = "recursive_type"
description = "A contract demonstrating recursive #[contracttype] structs"
version.workspace = true
authors.workspace = true
license.workspace = true
publish.workspace = true
repository.workspace = true
homepage.workspace = true
keywords.workspace = true
categories.workspace = true
edition.workspace = true

[features]
entrypoints = []
dev = ["sdk/dev"]

[lib]
crate-type = ["cdylib", "rlib"]
doctest = false
test = false

[dependencies]
sdk = { workspace = true, features = ["alloc"] }

[dev-dependencies]
recursive_type = { path = ".", features = ["dev"] }
```

- [ ] **Step 2: Create `src/lib.rs`**

Create `example-contracts/recursive_type/src/lib.rs`:

```rust
#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

mod implementation;

pub use implementation::*;
```

- [ ] **Step 3: Create `src/implementation.rs`**

Create `example-contracts/recursive_type/src/implementation.rs`:

```rust
extern crate alloc;

use alloc::boxed::Box;
use sdk::*;

#[contracttype]
pub struct Tree {
    pub value: u32,
    pub left: Option<Box<Tree>>,
    pub right: Option<Box<Tree>>,
}

#[derive(Default)]
pub struct RecursiveTypeContract;

#[contract(version = "0.1.0")]
impl RecursiveTypeContract {
    pub fn depth(tree: Tree) -> u32 {
        let left_depth = match tree.left {
            Some(left) => Self::depth(*left),
            None => 0,
        };
        let right_depth = match tree.right {
            Some(right) => Self::depth(*right),
            None => 0,
        };
        1 + if left_depth > right_depth {
            left_depth
        } else {
            right_depth
        }
    }
}
```

- [ ] **Step 4: Add to workspace**

In `example-contracts/Cargo.toml`, add `"recursive_type"` to the workspace members list (after `"spec_named_types"`).

- [ ] **Step 5: Build**

Run: `cargo check -p recursive_type`

Expected: Compilation succeeds. This is the key validation that recursive `#[contracttype]` works.

- [ ] **Step 6: Commit**

```bash
git add example-contracts/recursive_type/ example-contracts/Cargo.toml
git commit -m "Add recursive_type example contract

Proves that recursive #[contracttype] structs (Tree with
Option<Box<Tree>> fields) compile correctly after the
TYPE_DEFS removal."
```

---

### Task 8: Full workspace build and test

**Files:** None (verification only)

- [ ] **Step 1: Run full workspace check**

Run: `cargo check`

Expected: Clean compilation across entire workspace.

- [ ] **Step 2: Run full workspace tests**

Run: `cargo test`

Expected: All tests pass.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -- -D warnings`

Expected: No warnings.

- [ ] **Step 4: Run fmt check**

Run: `cargo fmt -- --check`

Expected: No formatting issues. If there are, fix with `cargo fmt`.

- [ ] **Step 5: Commit any fixups**

If fmt or clippy produced changes:

```bash
git add -u
git commit -m "Fix formatting and clippy warnings"
```
