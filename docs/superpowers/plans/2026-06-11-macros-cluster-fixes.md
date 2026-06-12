# Macros Cluster Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the enum-MintableType bug (auth tokens in enum payloads never minted), remove the `storage = false` footgun, reject ambiguous method visibility, dedup the auth-handle walkers, and harden `#[storage]` — per `docs/superpowers/specs/2026-06-11-macros-cluster-fixes-design.md`.

**Architecture:** All macro changes live in `crates/macros`; one new marker trait (`ContractState`) lands in `crates/sdk/src/storage/node.rs`. Contracts across `example-contracts/` and `integration_test/` migrate off `storage = false`. Tests are sdk-side integration tests plus trybuild compile-pass/compile-fail fixtures under `crates/sdk/tests/`.

**Tech Stack:** Rust (nightly-2026-01-30), syn/quote/darling proc macros, trybuild, xdrs.

**Conventions:**
- Work happens on branch `geoff/macros-cluster-fixes` (already created; spec committed).
- sdk package name is `sdk`; macros package is `groundhog-macros`. Run sdk tests as `cargo test -p sdk ...` from the repo root.
- Generated macro code always uses `::sdk::` paths. `crates/sdk/src/lib.rs:25` has `extern crate self as sdk;` so these resolve inside the sdk crate too.
- Trybuild harnesses: `crates/sdk/tests/compile_fail.rs` globs `tests/compile-fail/*.rs`, `compile_pass.rs` globs `tests/compile-pass/*.rs`, `compile_fail_storage.rs` globs `tests/compile-fail-storage/*.rs`. Regenerate `.stderr` files with `TRYBUILD=overwrite cargo test -p sdk --test compile_fail` and ALWAYS review the diff before committing.

---

### Task 1: `ContractState` marker trait in the sdk

**Files:**
- Modify: `crates/sdk/src/storage/node.rs`

The trait gives a self-explaining diagnostic when a `#[contract]` state type isn't a storage struct. It must exist before the macro change (Task 2) references it.

- [ ] **Step 1: Add the trait, helper fn, and a unit test**

Append to `crates/sdk/src/storage/node.rs` (after the existing impls; `Keypath` is already in scope in that module — check the file's imports and add `use super::keypath::Keypath;` only if it is not):

```rust
/// Marker for types usable as a contract's root state by `#[contract]`.
///
/// Automatically implemented for every type that implements
/// `From<Keypath>` — i.e. anything annotated with `#[storage]`, including
/// unit structs like `struct Token;`. Contract methods taking
/// `&self`/`&mut self` require the state type to implement this trait;
/// contracts with only associated functions never construct state and do
/// not need it.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used as contract state",
    note = "methods taking `&self`/`&mut self` require the state type to be a storage struct: add `#[storage]` to the definition of `{Self}`",
    note = "contracts with only associated functions (no `self` methods) do not need `#[storage]`"
)]
pub trait ContractState: From<Keypath> {}

impl<T: From<Keypath>> ContractState for T {}

/// Construct the root state node for a contract entrypoint.
///
/// Called by `#[contract]`-generated code; not intended for direct use.
#[doc(hidden)]
pub fn contract_state_root<T: ContractState>() -> Node<T> {
    Node::from(Keypath::root())
}
```

And a test module at the bottom of the same file (create it if the file has none; extend it if one exists):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct UnitState;
    impl From<Keypath> for UnitState {
        fn from(_: Keypath) -> Self {
            Self
        }
    }

    #[test]
    fn unit_state_satisfies_contract_state() {
        let _node: Node<UnitState> = contract_state_root::<UnitState>();
    }
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p sdk --lib node`
Expected: PASS (the new test runs; `pub use node::*;` in `storage/mod.rs` already re-exports the new items).

- [ ] **Step 3: Commit**

```bash
git add crates/sdk/src/storage/node.rs
git commit -m "Add ContractState marker trait with on_unimplemented diagnostic"
```

---

### Task 2: Remove the `storage` flag from `#[contract]`

**Files:**
- Modify: `crates/macros/src/common/config.rs` (remove field at line 182, method at 236-238, tests at ~311-326)
- Modify: `crates/macros/src/common/exports.rs:289-324` (the `load_state` block)

- [ ] **Step 1: Delete the config surface**

In `crates/macros/src/common/config.rs`:
- Delete `pub storage: Option<bool>,` from `struct Config`.
- Delete the `pub fn storage(&self) -> bool { ... }` method.
- Delete the three tests `storage_defaults_to_true`, `storage_explicit_true`, `storage_explicit_false` and the `// -- storage --` comment line.

- [ ] **Step 2: Make the storage path unconditional in exports.rs**

In `crates/macros/src/common/exports.rs`, replace the entire `if config.storage() { ... } else { ... }` inside the `let load_state = if is_method {` branch (currently lines 304-320) with:

```rust
        quote! {
            use core::ops::{Deref, DerefMut};
            let #mutability state_node: ::sdk::storage::Node<#self_ty> =
                ::sdk::storage::contract_state_root::<#self_ty>();
            let #mutability state :&#mutability #self_ty = #deref;
        }
```

(The `use ::sdk::storage::{Keypath, Node};` import line and the `// TODO: if !config.storage(), assert presence of Default impl` comment go away with the replaced block. Keep the `deref`/`mutability` computation above it unchanged.)

- [ ] **Step 3: Check the macros crate builds and its unit tests pass**

Run: `cargo test -p groundhog-macros`
Expected: PASS. If rustc warns that `config` is now unused in the enclosing function, check whether `config` is used elsewhere in that function; only if it is fully dead, rename the parameter in that function's signature to `_config` (do NOT remove the parameter — call sites stay stable).

Note: `cargo test -p sdk` will FAIL at this point (fixtures still say `storage = false`). That is expected; Task 3 fixes them. Do not commit yet.

---

### Task 3: Update sdk trybuild fixtures + new ContractState compile-fail test

**Files:**
- Modify: `crates/sdk/tests/compile-pass/bare_impl_default_entrypoints.rs`
- Modify: `crates/sdk/tests/compile-pass/signer_method_param.rs`
- Modify: `crates/sdk/tests/compile-pass/signer_in_contract_type.rs`
- Modify: `crates/sdk/tests/compile-fail/contract_invalid_version.rs` (+ `.stderr`)
- Modify: `crates/sdk/tests/compile-fail/contract_invalid_interface.rs` (+ `.stderr`)
- Modify: `crates/sdk/tests/compile-fail/contract_invalid_exports.rs` (+ `.stderr`)
- Modify: `crates/sdk/tests/compile-fail/contract_arg_pattern.rs` (+ `.stderr`)
- Modify: `crates/sdk/tests/compile-fail/legacy_exports_export_num_path.rs` (+ `.stderr`)
- Modify: `crates/sdk/tests/compile-fail/entrypoints_field_private.rs` (+ `.stderr`)
- Create: `crates/sdk/tests/compile-fail/contract_state_not_storage.rs` (+ `.stderr`)

- [ ] **Step 1: Apply the migration rule to every fixture above**

The rule (used again in Task 4 — it is repeated there in full):
1. In the `#[contract(...)]` attribute, delete `storage = false` (and the adjacent comma). If the parens become empty, write bare `#[contract]`.
2. If the impl block has any method with a `&self`/`&mut self` receiver: put `#[sdk::storage]` on the state struct. Delete `#[derive(Default)]` if `Default` has no other use in the file.
3. If the impl block has only associated functions: just delete `storage = false` and any `#[derive(Default)]` that existed solely for it.

Concretely for `bare_impl_default_entrypoints.rs` (has a `&self` method):

```rust
#![allow(unexpected_cfgs)]

use sdk::*;

#[sdk::storage]
pub struct BareContract;

#[contract]
impl BareContract {
    pub fn ping(&self) -> u32 {
        7
    }
}

fn main() {
    let _client = BareContractClient::new(&ContractAddress::default());
    let _entrypoint: ContractEntrypointID =
        BareContractEntrypoints::ping.into();
    let _entrypoint_u64 = BareContractEntrypoints::ping.into_u64();
}
```

For the two signer compile-pass fixtures and the six compile-fail fixtures, open each and apply the same rule (inspect the receivers — do not assume).

- [ ] **Step 2: Add the new compile-fail test for the ContractState diagnostic**

Create `crates/sdk/tests/compile-fail/contract_state_not_storage.rs`:

```rust
#![allow(unexpected_cfgs)]

use sdk::*;

pub struct Wallet {
    balance: u64,
}

#[contract]
impl Wallet {
    pub fn deposit(&mut self, amount: u64) {
        self.balance += amount;
    }
}

fn main() {}
```

- [ ] **Step 3: Regenerate stderr files and verify**

Run: `TRYBUILD=overwrite cargo test -p sdk --test compile_fail --test compile_pass`
Then: `git diff crates/sdk/tests/compile-fail/` — review every `.stderr` change. The six migrated fixtures should only shift line numbers / drop the storage arg from the rendered attribute. `contract_state_not_storage.stderr` must contain the strings `cannot be used as contract state` and `add `#[storage]``. If it shows a generic `From<Keypath>` trait error instead, the `#[diagnostic::on_unimplemented]` attribute isn't taking effect — stop and fix Task 1.

Run: `cargo test -p sdk`
Expected: PASS (all sdk tests, including both trybuild suites).

- [ ] **Step 4: Commit**

```bash
git add crates/macros crates/sdk/tests
git commit -m "Remove #[contract] storage flag; state always loads via ContractState

storage = false silently discarded &mut self mutations on a throwaway
Default value. The storage path is free for unit structs (Node is a
keypath-handle wrapper), so stateless contracts now use #[storage] on a
unit struct and the trap is structurally impossible: a fieldful
non-storage state type fails to compile with a diagnostic that names
the fix."
```

---

### Task 4: Migrate workspace contracts off `storage = false`

**Files (apply the rule from Task 3 Step 1 to each):**

example-contracts:
- Modify: `example-contracts/erc20_no_storage/src/implementation.rs`
- Modify: `example-contracts/invoke_flags_test/src/implementation.rs`
- Modify: `example-contracts/erc1271_wallet/src/lib.rs`
- Modify: `example-contracts/erc1271_caller/src/lib.rs`
- Modify: `example-contracts/signer_caller/src/lib.rs`
- Modify: `example-contracts/signer_callee/src/lib.rs`
- Modify: `example-contracts/signer_leaf/src/lib.rs`
- Modify: `example-contracts/signer_middle/src/lib.rs`
- Modify: `example-contracts/signer_minter/src/lib.rs`
- Modify: `example-contracts/signer_tuple_test/src/lib.rs`
- Modify: `example-contracts/bare_impl_caller/src/lib.rs` (no flag; just delete the dead `#[derive(Default)]` — associated fns only)

integration_test (same rule; the ones marked *disabled* are not workspace members but must be migrated textually anyway so they don't bitrot further):
- Modify: `integration_test/contracts/authclaim_test/src/implementation.rs`
- Modify: `integration_test/contracts/calldata_wrapper/src/implementation.rs`
- Modify: `integration_test/contracts/cxs_roundtrip/src/implementation.rs` *(disabled)*
- Modify: `integration_test/contracts/delegate_call_test/src/implementation.rs`
- Modify: `integration_test/contracts/deploy_and_transfer/src/implementation.rs`
- Modify: `integration_test/contracts/dev_wallet/src/implementation.rs`
- Modify: `integration_test/contracts/initial_token_claim/src/implementation.rs`
- Modify: `integration_test/contracts/itc_erc20/src/implementation.rs`
- Modify: `integration_test/contracts/manual_revert_test/src/implementation.rs` *(disabled)*
- Modify: `integration_test/contracts/nnint_output_fault_test/src/implementation.rs` *(disabled)*
- Modify: `integration_test/contracts/no_syscall_test/src/implementation.rs`
- Modify: `integration_test/contracts/output_alias_test/src/implementation.rs` *(disabled)*
- Modify: `integration_test/contracts/read_only_test/src/implementation.rs`
- Modify: `integration_test/contracts/return_fault_test/src/implementation.rs`
- Modify: `integration_test/contracts/sha256_wrapper/src/implementation.rs`
- Modify: `integration_test/contracts/simple_wallet_authclaim/src/implementation.rs`
- Modify: `integration_test/contracts/syscall_misc_tests/src/implementation.rs`
- Modify: `integration_test/contracts/syscall_parse_hardening_test/src/implementation.rs` *(disabled)*
- Modify: `integration_test/contracts/tuple_return/src/implementation.rs`
- Modify: `integration_test/contracts/txs_roundtrip/src/implementation.rs` *(disabled)*
- Modify: `integration_test/contracts/view_gateway_test/src/implementation.rs`

- [ ] **Step 1: Migrate every file in the list**

The migration rule, restated in full:
1. In the `#[contract(...)]` attribute, delete `storage = false` (and the adjacent comma). If the parens become empty, write bare `#[contract]`. Keep all other args (e.g. `version = "0.1.0"` in itc_erc20, `exports = "guest_exports"` in the hardening tests).
2. If the impl block has any method with a `&self`/`&mut self` receiver: put `#[sdk::storage]` (or `#[storage]` if the file has `use sdk::*;`) on the state struct. Delete `#[derive(Default)]` if `Default` has no other use in the file. Example (itc_erc20):

```rust
#[sdk::storage]
pub struct Token;

#[contract(version = "0.1.0")]
impl ERC20 for Token {
```

3. If the impl block has only associated functions: delete the flag (and any `#[derive(Default)]` that existed solely for it) and leave the bare struct alone.

Escape hatch: if any contract turns out to have a **fieldful** state struct under `storage = false` whose `Default` field values were actually read, stop and flag it in the task report instead of guessing — the spec says it must be converted to storage handles or associated functions, which needs a human look.

- [ ] **Step 2: Verify example-contracts**

Run (from repo root): `./scripts/dev.sh test-examples`
Expected: PASS for every package. (Known pre-existing issue: this script must run from the repo root or it silently iterates zero packages — make sure the output actually lists packages.)

- [ ] **Step 3: Verify integration_test compiles**

Run: `cargo check --workspace --manifest-path integration_test/Cargo.toml`
Expected: clean check. (Disabled contracts are not members and won't be checked — that's fine, they were migrated textually.)

- [ ] **Step 4: Commit**

```bash
git add example-contracts integration_test
git commit -m "Migrate contracts off storage = false to #[storage] unit structs"
```

---

### Task 5: Enum mintable recursion — failing tests first

**Files:**
- Create: `crates/sdk/tests/enum_mintable.rs`

- [ ] **Step 1: Write the tests**

```rust
//! Auth-token minting through `#[contract_type]` enums: a `Signed<T>`
//! inside an enum variant must mint a child AuthTokenNode, exactly as
//! it does inside a struct field (see signer_lifecycle.rs).

#![allow(unexpected_cfgs)]

use sdk::mintable::{AuthTokenPreimage, MintableType, build_auth_token_node};
use sdk::receive_auth_handles::ReceiveAuthHandles;
use sdk::signer::Signed;
use sdk::*;

#[contract_type]
pub struct PermitData {
    pub amount: u64,
}

#[contract_type]
pub enum Action {
    Noop = 0,
    Delegated(Signed<PermitData>) = 1,
    Pair {
        id: u32,
        permit: Signed<PermitData>,
    } = 2,
}

type ActionPreimage = <Action as MintableType>::Preimage;
type PermitPreimage = <PermitData as MintableType>::Preimage;

fn addr(byte: u8) -> ContractAddress {
    ContractAddress::from([byte; 32])
}

#[test]
fn unit_variant_mints_no_children() {
    let node = build_auth_token_node::<Action>(AuthTokenPreimage {
        consumer: addr(0xBB),
        data: ActionPreimage::Noop,
    });
    assert!(node.child_nodes.is_empty());
}

#[test]
fn tuple_variant_mints_child_token() {
    let node = build_auth_token_node::<Action>(AuthTokenPreimage {
        consumer: addr(0xBB),
        data: ActionPreimage::Delegated(AuthTokenPreimage {
            consumer: addr(0xCC),
            data: PermitPreimage { amount: 7 },
        }),
    });
    assert_eq!(node.consumer_address, addr(0xBB));
    assert_eq!(node.child_nodes.len(), 1);
    assert_eq!(node.child_nodes[0].consumer_address, addr(0xCC));
    assert!(node.child_nodes[0].child_nodes.is_empty());
}

#[test]
fn named_variant_mints_child_token() {
    let node = build_auth_token_node::<Action>(AuthTokenPreimage {
        consumer: addr(0xBB),
        data: ActionPreimage::Pair {
            id: 9,
            permit: AuthTokenPreimage {
                consumer: addr(0xCD),
                data: PermitPreimage { amount: 8 },
            },
        },
    });
    assert_eq!(node.child_nodes.len(), 1);
    assert_eq!(node.child_nodes[0].consumer_address, addr(0xCD));
}

#[test]
fn payload_roundtrip_with_receive_handles() {
    let node = build_auth_token_node::<Action>(AuthTokenPreimage {
        consumer: addr(0xBB),
        data: ActionPreimage::Delegated(AuthTokenPreimage {
            consumer: addr(0xCC),
            data: PermitPreimage { amount: 7 },
        }),
    });

    // Decode the minted payload (Signed encodes zero bytes; the data
    // travels in the child node) and bind a handle, mirroring
    // signer_lifecycle.rs.
    let payload_bytes: &[u8] = node.payload.as_ref();
    let mut decoded: Action =
        xdrs::cxs::decode(payload_bytes).expect("payload decode failed");

    let mut iter = [10u32].into_iter();
    decoded.receive_auth_handles(&mut iter).unwrap();

    match decoded {
        Action::Delegated(signed) => assert_eq!(signed.handle(), 10),
        other => panic!("wrong variant decoded: discriminant mismatch"),
    }
}
```

(If `xdrs` is not already a dev-dependency path that resolves in sdk tests, copy the exact decode invocation from `crates/sdk/tests/signer_lifecycle.rs:120-121`, which uses `xdrs::cxs::decode`.)

- [ ] **Step 2: Run and verify the tests fail**

Run: `cargo test -p sdk --test enum_mintable`
Expected: COMPILE FAILURE — `ActionPreimage` has no variant `Noop`/`Delegated`/`Pair` because today `<Action as MintableType>::Preimage` is `Action` itself and `Action::Delegated` requires a `Signed` value, not an `AuthTokenPreimage`. This compile failure IS the bug's signature. Do not commit yet.

---

### Task 6: Enum mintable recursion — implementation

**Files:**
- Modify: `crates/macros/src/contract_type.rs` (enum branch of `expand`, lines 14-63; add new generator near `generate_preimage_and_mintable`)

- [ ] **Step 1: Add the enum preimage/mintable generator**

Add after `generate_preimage_and_mintable` (the struct version, ends ~line 465):

```rust
/// Enum counterpart of [`generate_preimage_and_mintable`]: a mirrored
/// `Preimage` enum (same variants, same explicit discriminants, every
/// field type `T` replaced by `<T as MintableType>::Preimage`) plus
/// `MintableType`/`Mintable` impls whose `mint_into_parent` recurses
/// into each variant's fields, so `Signed` fields in enum payloads mint
/// child auth-token nodes exactly like struct fields do.
fn generate_preimage_and_mintable_enum(
    item: &syn::ItemEnum,
) -> (TokenStream, TokenStream) {
    let ident = &item.ident;
    let vis = &item.vis;
    let mod_name = format_ident!("__preimage_{}", ident);

    let mut preimage_variants: Vec<TokenStream> = Vec::new();
    let mut mint_arms: Vec<TokenStream> = Vec::new();

    for variant in &item.variants {
        let vident = &variant.ident;
        let (_, disc) = variant
            .discriminant
            .as_ref()
            .expect("explicit discriminants checked by check_enum_discriminants");

        match &variant.fields {
            syn::Fields::Named(fields) => {
                let pre_fields: Vec<TokenStream> = fields
                    .named
                    .iter()
                    .map(|f| {
                        let name = &f.ident;
                        let ty = &f.ty;
                        quote! {
                            #name: <#ty as ::sdk::mintable::MintableType>::Preimage
                        }
                    })
                    .collect();
                preimage_variants.push(quote! {
                    #vident { #(#pre_fields),* } = #disc
                });

                let names: Vec<_> =
                    fields.named.iter().map(|f| &f.ident).collect();
                let mints: Vec<TokenStream> = fields
                    .named
                    .iter()
                    .map(|f| {
                        let name = &f.ident;
                        let ty = &f.ty;
                        quote! {
                            #name: <#ty as ::sdk::Mintable>::mint_into_parent(
                                #name, children,
                            )
                        }
                    })
                    .collect();
                mint_arms.push(quote! {
                    #mod_name::Preimage::#vident { #(#names),* } =>
                        Self::#vident { #(#mints),* }
                });
            }
            syn::Fields::Unnamed(fields) => {
                let pre_fields: Vec<TokenStream> = fields
                    .unnamed
                    .iter()
                    .map(|f| {
                        let ty = &f.ty;
                        quote! {
                            <#ty as ::sdk::mintable::MintableType>::Preimage
                        }
                    })
                    .collect();
                preimage_variants.push(quote! {
                    #vident(#(#pre_fields),*) = #disc
                });

                let bindings: Vec<syn::Ident> = (0..fields.unnamed.len())
                    .map(|i| format_ident!("f{}", i))
                    .collect();
                let mints: Vec<TokenStream> = fields
                    .unnamed
                    .iter()
                    .zip(&bindings)
                    .map(|(f, b)| {
                        let ty = &f.ty;
                        quote! {
                            <#ty as ::sdk::Mintable>::mint_into_parent(
                                #b, children,
                            )
                        }
                    })
                    .collect();
                mint_arms.push(quote! {
                    #mod_name::Preimage::#vident(#(#bindings),*) =>
                        Self::#vident(#(#mints),*)
                });
            }
            syn::Fields::Unit => {
                preimage_variants.push(quote! { #vident = #disc });
                mint_arms.push(quote! {
                    #mod_name::Preimage::#vident => Self::#vident
                });
            }
        }
    }

    let preimage_mod = quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        #vis mod #mod_name {
            use super::*;

            #[derive(::sdk::ReadXDR, ::sdk::WriteXDR)]
            #[xdrs(crate = "::sdk::__xdrs")]
            #[repr(i32)]
            pub enum Preimage {
                #(#preimage_variants,)*
            }
        }
    };

    let mintable_impl = quote! {
        impl ::sdk::mintable::MintableType for #ident {
            type Preimage = #mod_name::Preimage;
        }

        impl ::sdk::Mintable for #ident {
            fn mint_into_parent(
                preimage: Self::Preimage,
                children: &mut ::sdk::mintable::ChildNodeBuf,
            ) -> Self {
                match preimage {
                    #(#mint_arms,)*
                }
            }
        }
    };

    (preimage_mod, mintable_impl)
}
```

- [ ] **Step 2: Wire it into the enum branch of `expand`**

In the `Item::Enum(item)` arm (contract_type.rs:14-63): delete the whole `let mintable_enum = quote! { ... };` identity block, and replace with:

```rust
            let (preimage_mod, mintable_enum) =
                generate_preimage_and_mintable_enum(&item);
```

(Place this AFTER the `check_enum_discriminants` early return so the `.expect` on discriminants is safe.) Then add `#preimage_mod` to the output quote, mirroring the struct branch's ordering:

```rust
            quote! {
                #[derive(::sdk::ReadXDR, ::sdk::WriteXDR, ::sdk::ContractTypeSpec)]
                #[xdrs(crate = "::sdk::__xdrs")]
                #[repr(i32)]
                #export_attr
                #item

                #preimage_mod

                #name

                #collect_auth

                #receive_auth

                #mintable_enum
            }
```

- [ ] **Step 3: Run the new tests**

Run: `cargo test -p sdk --test enum_mintable`
Expected: PASS (all 4 tests).

- [ ] **Step 4: Run the broader suites to catch regressions**

Run: `cargo test -p sdk && cargo test -p groundhog-macros`
Expected: PASS. Watch specifically for: existing `#[contract_type]` enums in tests/examples that constructed preimages under the old identity scheme — if any fail to compile, update them to the new mirrored-preimage construction (that breakage is the intended behavior change).

Run: `./scripts/dev.sh test-examples`
Expected: PASS (signer_* examples exercise mintable paths).

- [ ] **Step 5: Commit**

```bash
git add crates/macros/src/contract_type.rs crates/sdk/tests/enum_mintable.rs
git commit -m "Mint auth-token children recursively through contract_type enums

Enums previously got an identity MintableType (Preimage = Self) while
structs recursed field-by-field, so a Signed<T> in an enum variant
never minted a child node even though collect/receive walkers recurse
into enums. Enums now generate a mirrored Preimage enum (same explicit
discriminants) and a variant-matching mint_into_parent, symmetric with
structs."
```

---

### Task 7: Dedup the four auth-handle walkers

**Files:**
- Modify: `crates/macros/src/contract_type.rs:174-297, 467-518`

- [ ] **Step 1: Replace the four walkers with a parameterized pair**

Delete `expand_collect_auth_handles_struct`, `expand_receive_auth_handles_struct`, `expand_collect_auth_handles_enum`, `expand_receive_auth_handles_enum` and add:

```rust
/// Which auth-handle walker trait to generate. Collect and Receive
/// differ only in trait path, method signature, and per-field call —
/// parameterized here so the struct/enum field-walking logic exists
/// once.
#[derive(Clone, Copy)]
enum AuthWalk {
    Collect,
    Receive,
}

impl AuthWalk {
    fn call_on(self, target: TokenStream) -> TokenStream {
        match self {
            AuthWalk::Collect => quote! { #target.collect_auth_handles(out)?; },
            AuthWalk::Receive => {
                quote! { #target.receive_auth_handles(handles)?; }
            }
        }
    }

    fn impl_block(self, ident: &syn::Ident, body: TokenStream) -> TokenStream {
        match self {
            AuthWalk::Collect => quote! {
                impl ::sdk::CollectAuthHandles for #ident {
                    fn collect_auth_handles(
                        &self,
                        out: &mut ::sdk::collect_auth_handles::AuthHandleBuf,
                    ) -> Result<(), ::sdk::FaultString> {
                        #body
                        Ok(())
                    }
                }
            },
            AuthWalk::Receive => quote! {
                impl ::sdk::ReceiveAuthHandles for #ident {
                    fn receive_auth_handles(
                        &mut self,
                        handles: &mut impl ::core::iter::Iterator<Item = u32>,
                    ) -> Result<(), ::sdk::AuthTokenError> {
                        #body
                        Ok(())
                    }
                }
            },
        }
    }
}

fn expand_auth_handles_struct(
    item: &syn::ItemStruct,
    walk: AuthWalk,
) -> TokenStream {
    let field_calls: Vec<TokenStream> = match &item.fields {
        syn::Fields::Named(fields) => fields
            .named
            .iter()
            .map(|f| {
                let name = &f.ident;
                walk.call_on(quote! { self.#name })
            })
            .collect(),
        syn::Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let idx = syn::Index::from(i);
                walk.call_on(quote! { self.#idx })
            })
            .collect(),
        syn::Fields::Unit => vec![],
    };

    walk.impl_block(&item.ident, quote! { #(#field_calls)* })
}

fn expand_auth_handles_enum(
    item: &syn::ItemEnum,
    walk: AuthWalk,
) -> TokenStream {
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
                        .map(|n| walk.call_on(quote! { #n }))
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
                        .map(|b| walk.call_on(quote! { #b }))
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

    let ident = &item.ident;
    walk.impl_block(
        ident,
        quote! {
            match self {
                #(#arms,)*
            }
        },
    )
}
```

Update the four call sites in `expand`:
- enum branch: `let collect_auth = expand_auth_handles_enum(&item, AuthWalk::Collect);` and `let receive_auth = expand_auth_handles_enum(&item, AuthWalk::Receive);`
- struct branch: `let collect_auth = expand_auth_handles_struct(&item, AuthWalk::Collect);` and `let receive_auth = expand_auth_handles_struct(&item, AuthWalk::Receive);`

- [ ] **Step 2: Verify no behavior change**

Run: `cargo test -p groundhog-macros && cargo test -p sdk`
Expected: PASS (signer_lifecycle, signer_consume, enum_mintable all exercise both walkers on both shapes).

- [ ] **Step 3: Commit**

```bash
git add crates/macros/src/contract_type.rs
git commit -m "Dedup the four copy-pasted auth-handle walkers into one generator"
```

---

### Task 8: Reject restricted visibility on bare-impl contract methods

**Files:**
- Modify: `crates/macros/src/common/contract.rs` (new validator near `is_visible`, line ~399)
- Modify: `crates/macros/src/contract.rs` (call it in the `syn::Item::Impl` branch, ~line 140)
- Create: `crates/sdk/tests/compile-fail/bare_impl_pub_crate_method.rs` (+ `.stderr`)
- Create: `crates/sdk/tests/compile-pass/bare_impl_private_helper.rs`

- [ ] **Step 1: Write the compile-fail and compile-pass fixtures**

`crates/sdk/tests/compile-fail/bare_impl_pub_crate_method.rs`:

```rust
#![allow(unexpected_cfgs)]

use sdk::*;

#[sdk::storage]
pub struct Admin;

#[contract]
impl Admin {
    pub fn ping(&self) -> u32 {
        1
    }

    pub(crate) fn admin_reset(&self) {}
}

fn main() {}
```

`crates/sdk/tests/compile-pass/bare_impl_private_helper.rs`:

```rust
#![allow(unexpected_cfgs)]

use sdk::*;

#[sdk::storage]
pub struct Helper;

#[contract]
impl Helper {
    pub fn ping(&self) -> u32 {
        self.seven()
    }

    fn seven(&self) -> u32 {
        7
    }
}

fn main() {
    let _client = HelperClient::new(&ContractAddress::default());
}
```

- [ ] **Step 2: Run to verify current behavior (fail wrong / pass)**

Run: `cargo test -p sdk --test compile_fail --test compile_pass`
Expected: `bare_impl_pub_crate_method` FAILS the trybuild expectation (it currently compiles cleanly — the method is silently skipped — so trybuild reports "expected compile error, got success"). `bare_impl_private_helper` already passes.

- [ ] **Step 3: Implement the validator**

In `crates/macros/src/common/contract.rs`, next to `is_visible` (~line 399):

```rust
/// Bare-impl contract methods must be either `pub` (exported as an
/// entrypoint) or fully private (an internal helper, skipped). Restricted
/// visibility is ambiguous — the author plausibly expected export — so
/// reject it. Trait impls export every method regardless of visibility.
pub fn validate_bare_impl_method_visibility(
    item: &syn::ItemImpl,
) -> Result<()> {
    if item.trait_.is_some() {
        return Ok(());
    }
    for impl_item in &item.items {
        if let syn::ImplItem::Fn(f) = impl_item {
            if matches!(f.vis, syn::Visibility::Restricted(_)) {
                return Err(syn::Error::new_spanned(
                    &f.vis,
                    "ambiguous visibility for contract method: use `pub` \
                     to export it as an entrypoint, or make it private to \
                     keep it internal",
                ));
            }
        }
    }
    Ok(())
}
```

In `crates/macros/src/contract.rs`, in the `syn::Item::Impl(item)` branch, immediately after `let contract = ConfiguredContract::new(config.clone(), item);` add:

```rust
            contract::validate_bare_impl_method_visibility(item)?;
```

(`Result` here is `syn::parse::Result`, already imported in both files — verify `contract.rs` common module's `Result` import covers it; it uses `syn::Result` aliases already for the other validators.)

- [ ] **Step 4: Generate stderr and verify**

Run: `TRYBUILD=overwrite cargo test -p sdk --test compile_fail`
Review `bare_impl_pub_crate_method.stderr` — must contain `ambiguous visibility for contract method` with the span on `pub(crate)`.
Run: `cargo test -p sdk --test compile_fail --test compile_pass`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/macros crates/sdk/tests
git commit -m "Error on pub(crate)/pub(super) methods in bare-impl #[contract]

Restricted-visibility methods were silently omitted from exports,
client, and spec; now they are a spanned compile error. Private
methods remain the supported way to write helpers inside the impl."
```

---

### Task 9: `#[storage]` hygiene — reject args, qualify the derive

**Files:**
- Modify: `crates/macros/src/lib.rs:143-159` (the `storage` attribute macro)
- Modify: `crates/macros/src/storage.rs` (two `use sdk::storage::*;` occurrences in generated blocks)
- Create: `crates/sdk/tests/compile-fail-storage/storage_attr_args.rs` (+ `.stderr`)

- [ ] **Step 1: Write the compile-fail fixture**

`crates/sdk/tests/compile-fail-storage/storage_attr_args.rs`:

```rust
#![allow(unexpected_cfgs)]

#[sdk::storage(anything = "here")]
pub struct S {
    v: sdk::storage::NNInt,
}

fn main() {}
```

- [ ] **Step 2: Run to verify it fails wrong**

Run: `cargo test -p sdk --test compile_fail_storage`
Expected: trybuild reports "expected compile error, got success" for `storage_attr_args` (args are currently swallowed).

- [ ] **Step 3: Implement**

Replace the `storage` attribute macro in `crates/macros/src/lib.rs`:

```rust
#[cfg(feature = "storage")]
#[proc_macro_attribute]
pub fn storage(
    attr_args: TokenStream,
    item_tokens: TokenStream,
) -> TokenStream {
    let attr_args: proc_macro2::TokenStream = attr_args.into();
    if !attr_args.is_empty() {
        return syn::Error::new_spanned(
            &attr_args,
            "#[storage] takes no arguments",
        )
        .into_compile_error()
        .into();
    }

    let item = syn::parse_macro_input!(item_tokens as syn::Item);

    quote::quote! {
        #[derive(::sdk::Storage)]
        #item
    }
    .into()
}
```

In `crates/macros/src/storage.rs`, change both generated `use sdk::storage::*;` lines (one in the unit-struct branch ~line 66, one in the named-struct branch ~line 109) to `use ::sdk::storage::*;`.

- [ ] **Step 4: Generate stderr and verify**

Run: `TRYBUILD=overwrite cargo test -p sdk --test compile_fail_storage`
Review the new `.stderr` (must contain `#[storage] takes no arguments`).
Run: `cargo test -p sdk`
Expected: PASS (counter/examples compile via the qualified derive path; `extern crate self as sdk` covers in-crate uses).

- [ ] **Step 5: Commit**

```bash
git add crates/macros crates/sdk/tests
git commit -m "#[storage]: reject stray arguments, emit path-qualified derive"
```

---

### Task 10: Rustdoc for #[contract], #[contract_type], #[storage]

**Files:**
- Modify: `crates/macros/src/lib.rs` (doc comments on `pub fn contract` ~line 119, `pub fn contract_type`, `pub fn storage`)

- [ ] **Step 1: Write the docs**

On `pub fn contract` (replace any existing doc comment; keep it above the `#[proc_macro_attribute]`):

```rust
/// Turn a trait, impl block, or free function into contract entrypoints,
/// generating exports, a typed client, and spec registration.
///
/// # State
///
/// Methods taking `&self`/`&mut self` load the contract's state from
/// storage: the self type must be a storage struct (annotate it with
/// [`#[storage]`](macro@storage); a unit struct like `struct Token;`
/// works for contracts that keep no state in the struct). Contracts with
/// only associated functions need no storage annotation. The former
/// `storage = false` flag is gone — stateless contracts use a
/// `#[storage]` unit struct instead.
///
/// # Method visibility (bare impl blocks)
///
/// In a bare (non-trait) impl block, `pub` methods become entrypoints
/// and private methods are skipped (use them for helpers). Restricted
/// visibility (`pub(crate)`, `pub(super)`, `pub(in ...)`) is a compile
/// error because the intent is ambiguous. Trait impls export every
/// trait method regardless of visibility.
///
/// # Arguments
///
/// `version = "x.y.z"`, `interface = "Name"`, `exports = "mod_name"`,
/// `client = ClientIdent`, `throws = Error` / `throws(E1, E2)`,
/// `emits(Event1, Event2)`, `test_setup = "fn_name"`.
```

On `pub fn contract_type` (locate it in lib.rs; append to or create its doc comment):

```rust
/// Make a struct or enum usable as a contract argument/return/auth
/// payload: derives XDR codecs, spec registration, auth-handle walkers,
/// and the `Mintable` preimage machinery.
///
/// Both structs and enums recurse when minting auth tokens: a
/// [`Signed<T>`](signer::Signed) field — including one inside an enum
/// variant — becomes a child auth-token node, and its preimage type in
/// the generated `Preimage` mirror is `AuthTokenPreimage<T::Preimage>`.
///
/// Enum variants must carry explicit discriminants.
```

(Adjust intra-doc links if they don't resolve from the macros crate — plain code formatting without links is fine; `cargo doc -p groundhog-macros` must not warn.)

On `pub fn storage`:

```rust
/// Mark a struct as contract storage: generates `From<Keypath>` so each
/// field is bound to a storage slot derived from the struct's keypath.
///
/// Works on named-field structs (fields must themselves be storage
/// types like `NNInt`, `Map`, `Value`, or nested storage structs) and
/// on unit structs (`struct Token;`) for contracts that keep all state
/// behind raw storage keys. Takes no arguments.
```

- [ ] **Step 2: Verify docs build clean**

Run: `cargo doc -p groundhog-macros --no-deps 2>&1 | grep -i warn`
Expected: no warnings about the new docs (fix any broken intra-doc links by demoting them to plain code spans).

- [ ] **Step 3: Commit**

```bash
git add crates/macros/src/lib.rs
git commit -m "Document #[contract] state/visibility rules, #[contract_type] enum minting, #[storage]"
```

---

### Task 11: Full verification sweep

- [ ] **Step 1: Root workspace**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 2: Examples and integration**

Run (from repo root): `./scripts/dev.sh test-examples`
Expected: PASS, with packages actually listed in the output.
Run: `cargo check --workspace --manifest-path integration_test/Cargo.toml`
Expected: clean.

- [ ] **Step 3: Lint/format**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets 2>&1 | tail -5`
Expected: fmt clean; no new clippy warnings introduced by this work (pre-existing warnings out of scope).

- [ ] **Step 4: Grep for stragglers**

Run: `grep -rn "storage = false" --include="*.rs" . | grep -v target | grep -v docs/`
Expected: zero hits.

- [ ] **Step 5: Final commit if anything moved, then hand off**

Use the superpowers:finishing-a-development-branch skill (PR against main from `geoff/macros-cluster-fixes`).
