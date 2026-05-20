# `#[contract_type(export)]` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `#[contract_type(export)]` to force-include types in the contract spec even when they're not reachable from method signatures.

**Architecture:** A new `SpecFragmentView::ExportedTypeDef` variant (discriminant 3) distinguishes force-exported types from regular typedefs. The `#[contract_type(export)]` attribute passes `export` through to the derive via a helper attribute. The spec tool treats `ExportedTypeDef` fragments as both typedef definitions and reachability roots.

**Tech Stack:** Rust proc macros (syn/quote/darling), groundhog-contract-spec, groundhog-spec (external XDR crate), groundhog-spec-tool

**Spec:** `docs/superpowers/specs/2026-04-15-contract_type-export.md`

---

### Task 1: Add `ExportedTypeDef` variant to `SpecFragmentView`

**Files:**
- Modify: `groundhog-contract-spec/src/view.rs`

- [ ] **Step 1: Add the variant**

In `groundhog-contract-spec/src/view.rs`, add `ExportedTypeDef` to `SpecFragmentView`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstWriteXDR, WriteXDR)]
#[repr(i32)]
pub enum SpecFragmentView<'a> {
    Interface(SpecInterfaceFragmentView<'a>) = 0,
    Method(SpecMethodFragmentView<'a>) = 1,
    TypeDef(ContractTypeDefView<'a>) = 2,
    ExportedTypeDef(ContractTypeDefView<'a>) = 3,
}
```

- [ ] **Step 2: Update `From<SpecFragmentView>` for `SpecFragment`**

In the same file, find the `impl From<SpecFragmentView<'_>> for SpecFragment` block and add the new arm:

```rust
SpecFragmentView::ExportedTypeDef(def) => SpecFragment::ExportedTypeDef {
    type_def: def.into(),
},
```

Note: `SpecFragment::ExportedTypeDef` must exist in the external `groundhog-spec` crate. If it doesn't yet exist, this won't compile — see Task 1a below.

- [ ] **Step 3: Build to verify**

Run: `cargo check -p groundhog-contract-spec`

If this fails because `SpecFragment::ExportedTypeDef` doesn't exist in `groundhog-spec`, you need to add it to the external crate first (discriminant 3, same payload as `SpecTypeDef`). Bump `groundhog-spec` version accordingly and update the workspace dependency pin.

- [ ] **Step 4: Commit**

```bash
git add groundhog-contract-spec/src/view.rs
git commit -m "Add ExportedTypeDef variant to SpecFragmentView"
```

---

### Task 2: Add `export` to `#[contract_type]` macro and pass through to derive

**Files:**
- Modify: `macros/src/contract_type.rs`
- Modify: `macros/src/derive_contract_type_spec.rs`

- [ ] **Step 1: Add `export` to `ContractTypeArgs`**

In `macros/src/contract_type.rs`, add `export` to the args struct:

```rust
#[derive(Debug, Default, FromMeta)]
pub struct ContractTypeArgs {
    pub name: Option<String>,
    pub export: Option<bool>,
}
```

- [ ] **Step 2: Pass `export` through via helper attribute**

In `macros/src/contract_type.rs`, when `args.export == Some(true)`, inject a `#[contract_type_spec(export)]` helper attribute before the derive. Update both the enum and struct arms of `expand`. For the struct arm:

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
    let export_attr = if args.export == Some(true) {
        quote! { #[contract_type_spec(export)] }
    } else {
        quote! {}
    };
    quote! {
        #[derive(::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
        #[xdrs(crate = "::sdk::__xdrs")]
        #export_attr
        #item

        #name
    }
}
```

Apply the same pattern to the enum arm (add `#[repr(i32)]` after `#[xdrs(...)]` as before, plus `#export_attr`).

- [ ] **Step 3: Read and strip helper attribute in derive**

In `macros/src/derive_contract_type_spec.rs`, at the top of `expand()`, check for the `contract_type_spec` attribute with `export` in it:

```rust
pub fn expand(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let ident = &input.ident;
    let export = input.attrs.iter().any(|attr| {
        attr.path().is_ident("contract_type_spec")
            && attr
                .parse_args::<syn::Ident>()
                .map(|id| id == "export")
                .unwrap_or(false)
    });
    // ... rest of function
```

- [ ] **Step 4: Emit `ExportedTypeDef` when export is set**

In `emit_raw_typedef_fragment`, accept an `export: bool` parameter. Change the fragment expression:

```rust
fn emit_raw_typedef_fragment(ident: &syn::Ident, export: bool) -> TokenStream {
    let fragment_ident = format_ident!("__groundhog_spec_fragment_{ident}");
    let raw_len_ident = format_ident!("__groundhog_spec_raw_len_{ident}");
    let raw_fragment_ident =
        format_ident!("__groundhog_spec_raw_typedef_fragment_{ident}");

    let fragment_variant = if export {
        quote! { ::sdk::__contract_spec::SpecFragmentView::ExportedTypeDef(
            <#ident as ::sdk::__contract_spec::ContractTypeSpec>::TYPE_DEF
        ) }
    } else {
        quote! { ::sdk::__contract_spec::SpecFragmentView::TypeDef(
            <#ident as ::sdk::__contract_spec::ContractTypeSpec>::TYPE_DEF
        ) }
    };

    quote! {
        #[doc(hidden)]
        const #fragment_ident: ::sdk::__contract_spec::SpecFragmentView<'static> =
            #fragment_variant;

        #[doc(hidden)]
        const #raw_len_ident: usize = #fragment_ident.const_txs_size();

        #[cfg(target_family = "wasm")]
        #[doc(hidden)]
        #[cfg_attr(target_family = "wasm", unsafe(link_section = "groundhogspecrawv0"))]
        static #raw_fragment_ident: [u8; #raw_len_ident] = {
            let mut w =
                ::sdk::__xdrs::txs::ConstWriter::<#raw_len_ident>::new();
            #fragment_ident.const_write_txs(&mut w);
            w.into_bytes()
        };
    }
}
```

Update the call site in `expand()` to pass `export`:

```rust
let raw_typedef = emit_raw_typedef_fragment(ident, export);
```

- [ ] **Step 5: Build to verify**

Run: `cargo check -p groundhog-macros`

Expected: Compiles. The `contract_type_spec` helper attribute may produce a warning about an unknown attribute — if so, register it in the derive proc macro definition in `macros/src/lib.rs` using `#[proc_macro_derive(ContractTypeSpec, attributes(contract_type_spec))]`.

- [ ] **Step 6: Commit**

```bash
git add macros/src/contract_type.rs macros/src/derive_contract_type_spec.rs macros/src/lib.rs
git commit -m "Add export flag to #[contract_type] and pass through to derive"
```

---

### Task 3: Handle `ExportedTypeDef` in the spec tool

**Files:**
- Modify: `groundhog-spec-tool/src/lib.rs`
- Modify: `groundhog-spec-tool/tests/canonicalize.rs`

- [ ] **Step 1: Write a failing test**

In `groundhog-spec-tool/tests/canonicalize.rs`, add:

```rust
#[test]
fn canonicalize_spec_includes_exported_type_defs_as_roots() {
    let spec = canonicalize_spec(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        // Regular typedef — NOT reachable from any method, should be tree-shaken
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: s("Unreachable"),
                    fields: vec![].try_into().unwrap(),
                },
            },
        },
        // Exported typedef — NOT reachable from any method, but should be included
        SpecFragment::ExportedTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: s("ChannelState"),
                    fields: vec![ContractField {
                        name: s("nonce"),
                        ty: ContractType::U64,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
    ])
    .unwrap();

    let type_names: Vec<_> = spec
        .type_defs
        .iter()
        .map(|td| match td {
            ContractTypeDef::TypeDefStruct { struct_def } => {
                spec_str(&struct_def.name)
            }
            ContractTypeDef::TypeDefEnum { enum_def } => {
                spec_str(&enum_def.name)
            }
            ContractTypeDef::TypeDefUnion { union_def } => {
                spec_str(&union_def.name)
            }
            _ => panic!("unexpected variant"),
        })
        .collect();

    // ChannelState is included because it's exported
    assert!(type_names.contains(&"ChannelState"));
    // Unreachable is NOT included — regular typedef, not reachable
    assert!(!type_names.contains(&"Unreachable"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p groundhog-spec-tool -- canonicalize_spec_includes_exported_type_defs_as_roots`

Expected: FAIL — `SpecFragment::ExportedTypeDef` doesn't exist yet in the owned enum, or if it does, the spec tool ignores it (falls through to `_ => {}`).

- [ ] **Step 3: Handle `ExportedTypeDef` in `canonicalize_spec`**

In `groundhog-spec-tool/src/lib.rs`, in the `canonicalize_spec` function, add a new `HashSet` to track exported type names, and handle the new variant alongside `SpecTypeDef`:

Add near the top of the function (after `let mut type_defs = ...`):

```rust
let mut exported_roots = std::collections::HashSet::<ContractSpecString>::new();
```

In the match on fragments (after the `SpecFragment::SpecTypeDef` arm), add:

```rust
SpecFragment::ExportedTypeDef { type_def } => {
    let symbol = type_def_symbol(type_def).clone();
    exported_roots.insert(symbol.clone());
    let defs = type_defs.entry(symbol).or_default();
    if !defs.iter().any(|existing| existing == type_def) {
        defs.push(type_def.clone());
    }
}
```

Then update the `resolve_reachable_type_defs` call to pass the exported roots:

```rust
let type_defs = resolve_reachable_type_defs(&interfaces, &type_defs, &exported_roots)?;
```

- [ ] **Step 4: Update `resolve_reachable_type_defs` to accept exported roots**

Change the function signature:

```rust
fn resolve_reachable_type_defs(
    interfaces: &[ContractSpecInterface],
    defs: &BTreeMap<ContractSpecString, Vec<ContractTypeDef>>,
    exported_roots: &std::collections::HashSet<ContractSpecString>,
) -> Result<Vec<ContractTypeDef>, SpecToolError> {
```

After the existing loop that seeds roots from interface methods/throws (after line 330 in the current code), add:

```rust
for symbol in exported_roots {
    collect_named_types(
        &ContractType::Named { named: symbol.clone() },
        defs,
        &mut resolved,
        &mut pending,
    )?;
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p groundhog-spec-tool -- canonicalize_spec_includes_exported_type_defs_as_roots`

Expected: PASS

- [ ] **Step 6: Write a test for exported types with transitive deps**

```rust
#[test]
fn canonicalize_spec_exported_type_pulls_in_transitive_deps() {
    let spec = canonicalize_spec(&[
        SpecFragment::SpecInterface {
            interface_fragment: SpecInterfaceFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                throws: vec![].try_into().unwrap(),
            },
        },
        SpecFragment::SpecMethod {
            method_fragment: SpecMethodFragment {
                interface_name: s("Named"),
                interface_version: s("0.1.0"),
                method: ContractSpecMethod {
                    name: s("ping"),
                    args: vec![].try_into().unwrap(),
                    return_type: ContractType::Void,
                },
            },
        },
        SpecFragment::ExportedTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: s("Outer"),
                    fields: vec![ContractField {
                        name: s("inner"),
                        ty: ContractType::Named {
                            named: s("Inner"),
                        },
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
        SpecFragment::SpecTypeDef {
            type_def: ContractTypeDef::TypeDefStruct {
                struct_def: ContractStructDef {
                    name: s("Inner"),
                    fields: vec![ContractField {
                        name: s("value"),
                        ty: ContractType::U32,
                    }]
                    .try_into()
                    .unwrap(),
                },
            },
        },
    ])
    .unwrap();

    let type_names: Vec<_> = spec
        .type_defs
        .iter()
        .map(|td| match td {
            ContractTypeDef::TypeDefStruct { struct_def } => {
                spec_str(&struct_def.name)
            }
            _ => panic!("unexpected variant"),
        })
        .collect();

    assert!(type_names.contains(&"Outer"));
    assert!(type_names.contains(&"Inner"));
}
```

- [ ] **Step 7: Run all spec tool tests**

Run: `cargo test -p groundhog-spec-tool`

Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add groundhog-spec-tool/src/lib.rs groundhog-spec-tool/tests/canonicalize.rs
git commit -m "Handle ExportedTypeDef in spec tool as reachability root"
```

---

### Task 4: Add `export` to payment_channel's `ChannelState`

**Files:**
- Modify: `integration_test/contracts/payment_channel/src/interface.rs`

- [ ] **Step 1: Add `export` to `ChannelState`**

```rust
#[contract_type(export)]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "spec", derive(XDR))]
pub struct ChannelState {
    pub channel_id: u64,
    pub balance_a: i128,
    pub balance_b: i128,
    pub nonce: u64,
}
```

- [ ] **Step 2: Build to verify**

Run: `cargo check` (from the workspace root, with appropriate features)

Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add integration_test/contracts/payment_channel/src/interface.rs
git commit -m "Add export to payment_channel ChannelState"
```

---

### Task 5: Full workspace verification

- [ ] **Step 1: Run all tests**

Run: `cargo test -p groundhog-contract-spec -p groundhog-codegen -p groundhog-spec-tool`

Expected: All tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -p groundhog-contract-spec -p groundhog-codegen -p groundhog-spec-tool -p groundhog-macros -- -D warnings`

Expected: No warnings.

- [ ] **Step 3: Run fmt**

Run: `cargo fmt -- --check`

Expected: No formatting issues. Fix with `cargo fmt` if needed.

- [ ] **Step 4: Build WASM for payment_channel (if available)**

Run: `bash scripts/dev.sh build integration_test/contracts/payment_channel`

Expected: Builds successfully and spec includes `ChannelState`.

- [ ] **Step 5: Commit any fixups**

```bash
git add -u
git commit -m "Fix formatting and clippy warnings"
```
