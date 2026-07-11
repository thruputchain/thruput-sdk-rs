# SDK Footgun Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix four audit footguns — unhygienic generated locals, `Vector::len()` error-swallowing, missing cross-crate storage-domain collision detection, and `contract_return*` skipping `ValueMut` flushes — and land them as one PR passing CI.

**Architecture:** Two macro-hygiene renames in `crates/macros`; a one-line storage fix in `crates/sdk`; the existing native linkme domain registry re-used at the bin-wrapper aggregation point via a new one-line test macro; and the terminal-return fns moved out of the public API plus a *native-only* live-guard counter (zero guest bytes by construction). Spec: `docs/superpowers/specs/2026-07-11-sdk-footgun-fixes-design.md`.

**Tech Stack:** Rust workspace (`crates/*`), proc macros (`syn`/`quote`), sdk mock `TestEnvironment` tests in `example-contracts/*`, bash CI scripts under `scripts/`.

## Global Constraints

- **Zero on-chain cost:** every live-guard-counter piece sits under `cfg(all(not(any(doc, target_family = "wasm", target_arch = "riscv32")), feature = "std"))` — guest (wasm/rv32) artifacts must contain none of it. No runtime domain checks on guests.
- Generated-code locals use the existing `__gh_` prefix convention (`__gh_calldata` precedent).
- No deprecation shims: old `sdk::contract_return*` root paths are removed outright.
- No spec/XDR/protocol/CLI changes of any kind.
- Working directory: `/Users/geofframseyer/groundhog/agent1/sdk_rs`. Main workspace commands run from the repo root; example-contracts is its own workspace (`cd example-contracts` for those).
- Run `cargo fmt` before every commit (repo has `rustfmt.toml`).

---

### Task 0: Branch

**Files:** none

- [ ] **Step 1: Create the working branch**

```bash
cd /Users/geofframseyer/groundhog/agent1/sdk_rs
git checkout -b sdk-footgun-fixes
```

- [ ] **Step 2: Sanity-check baseline**

Run: `cargo test -p sdk --features dev 2>&1 | tail -5`
Expected: existing tests pass (green baseline before changes).

---

### Task 1: Hygienic locals in generated client/wrapper code

**Files:**
- Create: `example-contracts/codegen_hygiene/Cargo.toml`
- Create: `example-contracts/codegen_hygiene/src/lib.rs`
- Create: `example-contracts/codegen_hygiene/tests/test.rs`
- Modify: `example-contracts/Cargo.toml` (members list)
- Modify: `crates/macros/src/common/client.rs:253-254,277-278` (+ the `log_call` quote at ~line 209)
- Modify: `crates/macros/src/common/exports.rs:376-381` (`load_state`) and `:388-399` (`invoke`)

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: generated client methods bind `__gh_entrypoint`/`__gh_args`; generated guest wrappers bind `__gh_state_node`/`__gh_state`. No downstream task depends on these names.

- [ ] **Step 1: Write the failing fixture crate**

`example-contracts/codegen_hygiene/Cargo.toml`:

```toml
[package]
name = "codegen_hygiene"
description = "Codegen test: user params named after generated locals must not be shadowed"
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

[dependencies]
sdk = { workspace = true }

[dev-dependencies]
codegen_hygiene = { path = ".", features = ["dev"] }
```

`example-contracts/codegen_hygiene/src/lib.rs`:

```rust
//! Regression fixture: user parameters named after macro-generated locals
//! (`entrypoint`, `args`, `state`, `state_node`) must be passed through
//! verbatim, not shadowed by the generated bindings (audit finding #5:
//! a client-side param named `entrypoint` silently sent the generated
//! entrypoint ID instead of the argument).
#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_contract!();

use sdk::storage::Value;
use sdk::*;

#[derive(Debug)]
#[sdk::storage]
pub struct Hygiene {
    marker: Value<u64>,
}

#[contract(client = HygieneClient)]
impl Hygiene {
    /// Client-side hazard: generated method binds the entrypoint ID before
    /// capturing user args.
    pub fn echo_entrypoint(&self, entrypoint: u64) -> u64 {
        entrypoint
    }

    /// Client-side: generated method binds the args tuple.
    pub fn echo_args(&self, args: u64) -> u64 {
        args
    }

    /// Guest-side hazard: generated wrapper binds the loaded state.
    pub fn echo_state(&self, state: u64) -> u64 {
        state
    }

    /// Guest-side: generated wrapper binds the state node.
    pub fn echo_state_node(&self, state_node: u64) -> u64 {
        state_node
    }

    /// All at once, mixed with a real storage read so `self` is used.
    pub fn echo_sum(&self, state: u64, entrypoint: u64, args: u64) -> u64 {
        state + entrypoint + args + self.marker.try_get().none_if_missing().unwrap_or(0)
    }
}
```

`example-contracts/codegen_hygiene/tests/test.rs`:

```rust
use codegen_hygiene::HygieneClient;
use sdk::*;

static NONCE: SHA256Hash = sdk::sha256_static!("CODEGEN_HYGIENE_NONCE");

#[test]
fn params_named_after_generated_locals_pass_through() {
    let env = sdk::TestEnvironment::no_cost();
    let contract = codegen_hygiene::register_mock_contract(&env);
    let hash = contract.as_hash().clone();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&NONCE, &hash);
            let client = HygieneClient::new(&addr);

            assert_eq!(client.echo_entrypoint(42), 42);
            assert_eq!(client.echo_args(43), 43);
            assert_eq!(client.echo_state(44), 44);
            assert_eq!(client.echo_state_node(45), 45);
            assert_eq!(client.echo_sum(1, 2, 3), 6);

            // try_ variants generate their own bindings — exercise one.
            assert_eq!(client.try_echo_entrypoint(46).unwrap(), 46);
        })
    })
    .assert_success();
}
```

Register the crate — in `example-contracts/Cargo.toml`, add to the `members`
list (alphabetically near the other `codegen_*` entries):

```toml
    "codegen_hygiene",
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd example-contracts && cargo test -p codegen_hygiene 2>&1 | tail -20`
Expected: FAIL — `echo_state`/`echo_state_node` produce compile errors in the generated wrapper (the user arg shadows the generated `state` binding before `#invoke`, so `state.echo_state(state)` calls a method on `u64`). If it somehow compiles, `echo_entrypoint` mis-sends the generated ID and the assertion fails. Either failure mode confirms the bug.

- [ ] **Step 3: Rename the client-side generated locals**

In `crates/macros/src/common/client.rs`, in the plain generated method (currently lines 253-254):

```rust
            let __gh_entrypoint = #entrypoint_lit;
            let __gh_args = #args_tuple;
```

and its call site (currently line 260):

```rust
                ::sdk::call::try_call_raw_with::<_, #return_type, ::core::convert::Infallible>(
                    &self.address, __gh_entrypoint, self.config.gas_limit, &__gh_args, self.config.flags,
                    |exn| exn.re_throw(),
                )
```

Same in the `try_` method (currently lines 277-278 and 284-286):

```rust
            let __gh_entrypoint = #entrypoint_lit;
            let __gh_args = #args_tuple;
```

```rust
                match ::sdk::call::try_call_raw_with::<_, #return_type, #error_enum_ident>(
                    &self.address, __gh_entrypoint, self.config.gas_limit, &__gh_args, self.config.flags,
                    |exn| { #selector_dispatch },
                ) {
```

And the logging quote (currently ~line 209, only active with the `logging` feature) references the old `args` binding — update it:

```rust
        quote! {
            ::sdk::debug_log!("{} with args {:?}", #print_string, __gh_args);
        }
```

- [ ] **Step 4: Rename the guest-side generated locals**

In `crates/macros/src/common/exports.rs`, `load_state` (currently lines 370-381):

```rust
        let deref = if is_mut {
            quote! { __gh_state_node.deref_mut() }
        } else {
            quote! { __gh_state_node.deref() }
        };

        quote! {
            use core::ops::{Deref, DerefMut};
            let #mutability __gh_state_node: ::sdk::storage::Node<#self_ty> =
                ::sdk::storage::contract_state_root::<#self_ty>();
            let #mutability __gh_state :&#mutability #self_ty = #deref;
        }
```

and `invoke` (currently lines 388-399):

```rust
        if let Some(contract) = contract {
            if let Some(trait_path) = contract.trait_path() {
                let self_ty = contract.self_ty();
                quote! { <#self_ty as #trait_path>::#ident(__gh_state, #arg_names) }
            } else {
                quote! { __gh_state.#ident(#arg_names) }
            }
        } else {
            unreachable!("is_method implies contract source")
        }
```

Then grep for stragglers — every remaining generated-code reference to the old names must be updated (there should be none):

Run: `grep -n 'quote' -A3 crates/macros/src/common/client.rs crates/macros/src/common/exports.rs | grep -w 'entrypoint\|args\|state\|state_node' | grep -v __gh`
Expected: no hits inside generated-code quotes (matches in macro-internal Rust variables like `let entrypoint = contract_entrypoint_id(...)` are fine — those are expansion-time values, not generated bindings).

- [ ] **Step 5: Run the fixture test to verify it passes**

Run: `cd example-contracts && cargo test -p codegen_hygiene 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 6: Run the macro/sdk/example suites to catch regressions**

Run: `cargo test -p groundhog-contract-macros && cargo test -p sdk --features dev 2>&1 | tail -3 && cd example-contracts && cargo test 2>&1 | tail -3`
Expected: PASS (if the macros crate has a different package name, find it with `grep '^name' crates/macros/Cargo.toml`).

- [ ] **Step 7: Commit**

```bash
cd /Users/geofframseyer/groundhog/agent1/sdk_rs
cargo fmt
git add crates/macros example-contracts/codegen_hygiene example-contracts/Cargo.toml
git commit -m "macros: use hygienic __gh_ names for generated client/wrapper locals

A user parameter named 'entrypoint' was silently shadowed by the
generated client binding, sending the method's own generated ID
instead of the argument (audit #5)."
```

---

### Task 2: `Vector::len()` must not swallow type-confusion errors

**Files:**
- Modify: `crates/sdk/src/storage/vector.rs:55-63`
- Modify: `example-contracts/storage_errors/src/lib.rs`
- Test: `example-contracts/storage_errors/tests/test.rs`

**Interfaces:**
- Consumes: existing `NoneIfMissing::none_if_missing` (`crates/sdk/src/exn/library.rs:151`).
- Produces: `Vector::len()` aborts `LibraryExn::StorageSerialization` on undecodable len slots. `StorageErrorContract` gains `corrupt_len_slot(&mut self)` (used only by its own tests).

- [ ] **Step 1: Add the corruption method to the fixture and write the failing test**

In `example-contracts/storage_errors/src/lib.rs`, add inside the `#[contract] impl StorageErrorContract` block:

```rust
    /// Deliberately write an undecodable payload (zero bytes) at the
    /// vector's len slot (fixed child 0 of the vector's keypath) to
    /// simulate slot aliasing / type confusion.
    pub fn corrupt_len_slot(&mut self) {
        let mut bad: Value<()> =
            Value::from(self.numbers.keypath().child(&0u32));
        bad.set(&());
    }
```

In `example-contracts/storage_errors/tests/test.rs`, add (reusing the file's existing setup helpers — match the surrounding tests' setup/deploy pattern exactly):

```rust
#[test]
fn corrupted_len_slot_aborts_instead_of_reading_zero() {
    let (env, contract_hash) = setup();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&NONCE, &contract_hash);
            let client = StorageErrorClient::new(&addr);
            client.initialize();
            assert_eq!(client.vector_len(), 3);

            client.corrupt_len_slot();

            // A len slot that exists but does not decode as u32 must abort
            // typed — NOT read as "empty vector" (which would let the next
            // push clobber whatever lives at element 0).
            match client.try_vector_len() {
                Err(StorageErrorClientError::Library(
                    LibraryExn::StorageSerialization,
                )) => {}
                other => panic!(
                    "expected StorageSerialization abort, got {other:?}"
                ),
            }
        })
    })
    .assert_success();
}

#[test]
fn missing_len_slot_still_reads_as_empty() {
    let (env, contract_hash) = setup();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let addr = deploy_contract(&NONCE, &contract_hash);
            let client = StorageErrorClient::new(&addr);
            // No initialize: len slot never written -> empty vector.
            assert_eq!(client.vector_len(), 0);
        })
    })
    .assert_success();
}
```

(If the existing test file names its nonce/setup differently, adopt its names; add `LibraryExn`/`StorageErrorClientError` imports as needed. If `setup()` doesn't exist, follow the file's existing per-test env/deploy boilerplate.)

- [ ] **Step 2: Run to verify the corruption test fails**

Run: `cd example-contracts && cargo test -p storage_errors corrupted_len_slot 2>&1 | tail -10`
Expected: FAIL — `try_vector_len` returns `Ok(0)` (the bug: errors swallowed to 0). `missing_len_slot_still_reads_as_empty` should already PASS.

- [ ] **Step 3: Apply the one-line fix + doc note**

In `crates/sdk/src/storage/vector.rs`, replace the `len` method:

```rust
    /// Current logical extent (number of in-bounds indices).
    ///
    /// A missing len slot reads as 0 (a never-written vector is empty).
    /// Any other failure — a value at the slot that is not a `u32`, a
    /// wrong storage type, a host fault — aborts typed like every other
    /// storage accessor, instead of masquerading as an empty vector.
    ///
    /// Vector slots are sparse-capable: callers may delete element storage
    /// without shrinking len, so an in-bounds index does not guarantee that
    /// the slot currently has backing data.
    pub fn len(&self) -> u32 {
        self.len.try_get().none_if_missing().unwrap_or(0)
    }
```

Add the import if not present: `use crate::exn::NoneIfMissing;` (place with the other `use` items at the top of the file, or use the fully-qualified call `crate::exn::NoneIfMissing::none_if_missing(...)` — prefer the import).

- [ ] **Step 4: Run to verify both tests pass**

Run: `cd example-contracts && cargo test -p storage_errors 2>&1 | tail -5`
Expected: PASS (all storage_errors tests, including pre-existing ones — `len()` is called by `push_with`/`bounds_check`/`iter`, so watch for collateral failures; there should be none since missing-key still maps to 0).

- [ ] **Step 5: Run the sdk suite**

Run: `cargo test -p sdk --features dev 2>&1 | tail -3`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cd /Users/geofframseyer/groundhog/agent1/sdk_rs
cargo fmt
git add crates/sdk/src/storage/vector.rs example-contracts/storage_errors
git commit -m "sdk: Vector::len() aborts on undecodable len slots instead of reading 0

try_get().unwrap_or(0) swallowed type-confusion/decode errors, reading
a corrupted len slot as 'empty vector' so the next push clobbered real
state (audit #6). Missing-key still reads as 0 via none_if_missing()."
```

---

### Task 3: Move `contract_return*` out of the public API

**Files:**
- Create: `crates/sdk/src/internal/terminal_return.rs`
- Modify: `crates/sdk/src/internal/mod.rs`
- Modify: `crates/sdk/src/contract_op.rs` (remove the three fns; keep `contract_abort`, `contract_revert`, helpers)
- Modify: `crates/macros/src/common/exports.rs:476`
- Modify: `integration_test/contracts/dev_wallet/src/implementation.rs:76`
- Modify: `crates/sdk/tests/aborts.rs:38`, `crates/sdk/tests/large_encodes.rs:23`, `crates/sdk/tests/return_encode_error.rs:60`
- Modify: `crates/rv32-runtime/src/dispatch.rs:10` (doc comment path)

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: `sdk::internal::contract_return(buf: &[u8], auth_token_ids: &[u32]) -> !`, `sdk::internal::contract_return_buf(buf: &[u8]) -> !`, `sdk::internal::contract_return_encoded<RET: WriteXDR + ?Sized + CollectAuthHandles>(ret: &RET) -> !`. Task 4 adds the live-guard check inside `contract_return`.

- [ ] **Step 1: Move the three fns**

Create `crates/sdk/src/internal/terminal_return.rs` containing `contract_return`, `contract_return_buf`, and `contract_return_encoded` cut verbatim from `crates/sdk/src/contract_op.rs:17-31,128-152`, with (a) imports they need (`use crate::sys;`, `use crate::{CollectAuthHandles, FaultString, HandlingError, library_throw};`, `use crate::xdr::*;` — trim to what compiles), (b) `#[doc(hidden)]` on each, and (c) this module/doc header:

```rust
//! Terminal-return plumbing for generated entrypoint wrappers.
//!
//! These fns diverge via the Return syscall on real guests: **`Drop` does
//! not run past them.** A live deferred-write guard (e.g.
//! [`ValueMut`](crate::storage::ValueMut)) at the call would have its
//! flush silently discarded on-chain — while the native mock implements
//! Return as an unwind, so destructors DO run in tests, hiding the loss.
//! That asymmetry is why these are not public API: generated wrappers call
//! them only after the user method (and all its locals) has returned.
//! If you call them directly, drop all storage guards first.
```

In `crates/sdk/src/internal/mod.rs` add:

```rust
mod terminal_return;
pub use terminal_return::*;
```

In `crates/sdk/src/contract_op.rs`: delete the three moved fns. `contract_abort`, `contract_revert`, `contract_abort_encoded`, `client_contract_abort_encoded`, `_contract_abort_encoded`, `exception_encoding_error`, and the calldata machinery stay. Note `contract_return_encoded`'s helper calls (`crate::call::collect_auth_handle_bytes`, `crate::call::with_scratch`, `crate::internal::encode_scratch`, `crate::exn::encoding_success_throw`) resolve from the new module unchanged since they're crate-absolute.

- [ ] **Step 2: Update all callers**

- `crates/macros/src/common/exports.rs:476`: `::sdk::contract_return_encoded(&res);` → `::sdk::internal::contract_return_encoded(&res);`
- `integration_test/contracts/dev_wallet/src/implementation.rs:76`: `sdk::contract_return(` → `sdk::internal::contract_return(`
- `crates/sdk/tests/aborts.rs:38`: `sdk::contract_return_buf(` → `sdk::internal::contract_return_buf(`
- `crates/sdk/tests/large_encodes.rs:23` and `crates/sdk/tests/return_encode_error.rs:60`: `sdk::contract_return_encoded(` → `sdk::internal::contract_return_encoded(`
- `crates/rv32-runtime/src/dispatch.rs:10` doc comment: mention `sdk::internal::contract_return_encoded`.

Then sweep for anything missed:

Run: `grep -rn "contract_return" --include='*.rs' crates example-contracts integration_test standard-interfaces standard-tests logging-test | grep -v "internal::contract_return" | grep -v terminal_return`
Expected: only definition-internal references (`contract_return(buf` calls inside `terminal_return.rs`) and comments.

- [ ] **Step 3: Verify everything builds and tests pass**

Run: `cargo build --workspace && cargo test -p sdk --features dev 2>&1 | tail -3 && (cd integration_test && cargo build 2>&1 | tail -3)`
Expected: PASS. (If integration_test builds differently, use `scripts/dev.sh` — `grep -n "itc_build_all" scripts/dev.sh` shows the canonical command.)

- [ ] **Step 4: Commit**

```bash
cd /Users/geofframseyer/groundhog/agent1/sdk_rs
cargo fmt
git add -A crates/sdk crates/macros crates/rv32-runtime integration_test
git commit -m "sdk: move contract_return* out of the public API into sdk::internal

Terminal returns skip Drop on real guests, so a live ValueMut's flush
is silently discarded on the success path — invisible in native tests
because the mock unwinds (audit #7). These are wrapper plumbing, not
API; doc(hidden) under sdk::internal with a loud warning."
```

---

### Task 4: Native-only live-`ValueMut` check at terminal return

**Files:**
- Create: `crates/sdk/src/storage/live_guards.rs`
- Modify: `crates/sdk/src/storage/mod.rs` (module decl)
- Modify: `crates/sdk/src/storage/value.rs:121-127` (`load_mut`), `:162-166` (`Drop`)
- Modify: `crates/sdk/src/internal/terminal_return.rs` (`contract_return`)
- Modify: `crates/sdk/src/call/calls.rs:294-295` (`invoke_frame`, around `raw_Invoke`)
- Modify: `example-contracts/storage_errors/src/lib.rs`
- Test: `example-contracts/storage_errors/tests/test.rs`

**Interfaces:**
- Consumes: Task 3's `sdk::internal::contract_return*`.
- Produces: crate-private `live_guards::{guard_created(), guard_dropped(), assert_no_live_guards(), FrameGuardReset}` — native-test-only; nothing outside `sdk` sees it.

The cfg used on the module and on EVERY call site (referred to below as THE-CFG):

```rust
#[cfg(all(
    not(any(doc, target_family = "wasm", target_arch = "riscv32")),
    feature = "std"
))]
```

- [ ] **Step 1: Add fixture methods and write the failing tests**

In `example-contracts/storage_errors/src/lib.rs`:

Add a field to the storage struct (appended LAST — positional slots must not shift):

```rust
#[sdk::storage]
pub struct StorageErrorContract {
    numbers: Vector<Value<u64>>,
    names: Map<u64, Value<u64>>,
    plain: Value<u64>,
}
```

In `initialize`, append:

```rust
        self.plain.set(&0);
```

Add methods to the impl:

```rust
    /// Early-exit while a deferred-write guard is live: on a real guest the
    /// Return syscall skips Drop and the write to `plain` is silently lost.
    /// The native live-guard check must fail this loudly instead.
    pub fn early_return_with_live_guard(&mut self) {
        let mut guard = self.plain.load_mut();
        *guard += 1;
        sdk::internal::contract_return_encoded(&());
    }

    /// Holding a guard across a nested call is legitimate (the callee frame
    /// has its own state); must NOT false-positive the callee's return.
    pub fn guard_across_call(&mut self, other: ContractAddress) -> u64 {
        let mut guard = self.plain.load_mut();
        *guard += 1;
        let peer_len = StorageErrorClient::new(&other).vector_len();
        *guard += peer_len as u64;
        *guard
        // guard drops here -> flush; wrapper then returns normally
    }

    pub fn get_plain(&self) -> u64 {
        self.plain.try_get().none_if_missing().unwrap_or(0)
    }
```

(Add `use sdk::ContractAddress;` etc. as needed — the file's `use sdk::*;` likely already covers it.)

In `example-contracts/storage_errors/tests/test.rs`:

```rust
#[test]
fn early_return_with_live_guard_fails_loudly() {
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (env, contract_hash) = setup();
        env.transact(|tx| {
            tx.call(&ContractAddress::named("test")).run(move || {
                let addr = deploy_contract(&NONCE, &contract_hash);
                let client = StorageErrorClient::new(&addr);
                client.initialize();
                // Must fail loudly. If the check surfaces as a call error
                // instead of a panic, re-panic with it so the outer
                // catch_unwind sees the message either way.
                let res = client.try_early_return_with_live_guard();
                panic!("surfaced as call result: {res:?}");
            })
        })
        .assert_success();
    }));

    let payload = caught.expect_err(
        "early return with a live ValueMut must fail, not silently succeed",
    );
    let msg = payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default();
    assert!(
        msg.contains("live ValueMut"),
        "failure must name the live guard, got: {msg}"
    );
}

#[test]
fn guard_across_nested_call_is_not_a_false_positive() {
    let (env, contract_hash) = setup();

    env.transact(|tx| {
        tx.call(&ContractAddress::named("test")).run(move || {
            let a = deploy_contract(&NONCE_A, &contract_hash);
            let b = deploy_contract(&NONCE_B, &contract_hash);
            let ca = StorageErrorClient::new(&a);
            let cb = StorageErrorClient::new(&b);
            ca.initialize();
            cb.initialize(); // b's vector has len 3

            // a holds a ValueMut across the call into b; b's normal return
            // must not trip the live-guard check (b's frame has no guards).
            assert_eq!(ca.guard_across_call(b.clone()), 4); // 0 + 1 + 3

            // a's guard flushed on drop.
            assert_eq!(ca.get_plain(), 4);
        })
    })
    .assert_success();
}
```

Add `NONCE_A`/`NONCE_B` statics via `sdk::sha256_static!` following the file's existing nonce pattern. If `guard_across_call`'s signature needs `&ContractAddress` per the client codegen conventions, match what the generated client expects (check a passing example like `dev_wallet`'s `ERC20Client.transfer(&addr, ...)` — references are the norm; adjust `b.clone()` accordingly).

- [ ] **Step 2: Run to verify current (broken) behavior**

Run: `cd example-contracts && cargo test -p storage_errors early_return 2>&1 | tail -10`
Expected: FAIL — the call silently succeeds (mock unwind runs the Drop flush), so the inner panic message is `surfaced as call result: Ok(())` which does not contain "live ValueMut".

`guard_across_nested_call` should PASS at this point (no check exists yet) — run it too and note it: `cargo test -p storage_errors guard_across 2>&1 | tail -5`.

- [ ] **Step 3: Implement the counter module**

Create `crates/sdk/src/storage/live_guards.rs`:

```rust
//! Native-test-only tracking of live deferred-write guards (`ValueMut`).
//!
//! On a real guest a terminal Return syscall skips `Drop`, silently
//! discarding a live guard's flush; the native mock unwinds, so `Drop`
//! runs and tests can't see the loss. This counter inverts that: the
//! terminal-return funnel panics on a live guard, so any native test
//! exercising the path fails loudly. The entire module (and every call
//! site) is compiled only for the native test environment — guest
//! artifacts contain none of it.

use core::cell::Cell;

std::thread_local! {
    static LIVE_VALUE_MUTS: Cell<u32> = const { Cell::new(0) };
}

pub(crate) fn guard_created() {
    LIVE_VALUE_MUTS.with(|c| c.set(c.get() + 1));
}

pub(crate) fn guard_dropped() {
    LIVE_VALUE_MUTS.with(|c| c.set(c.get().saturating_sub(1)));
}

/// Called by `contract_return` before the Return syscall.
pub(crate) fn assert_no_live_guards() {
    let n = LIVE_VALUE_MUTS.with(|c| c.get());
    if n != 0 {
        panic!(
            "contract_return with {n} live ValueMut guard(s): on a real \
             guest the terminal Return syscall skips Drop, silently \
             discarding their storage writes. Drop the guard(s) before \
             returning."
        );
    }
}

/// Zeroes the counter for the duration of a nested in-process mock frame
/// (the callee has its own memory on a real guest, so the caller's live
/// guards must not leak into its check); restores the saved count on drop,
/// including on unwind.
pub(crate) struct FrameGuardReset(u32);

impl FrameGuardReset {
    pub(crate) fn new() -> Self {
        Self(LIVE_VALUE_MUTS.with(|c| c.replace(0)))
    }
}

impl Drop for FrameGuardReset {
    fn drop(&mut self) {
        let saved = self.0;
        LIVE_VALUE_MUTS.with(|c| c.set(saved));
    }
}
```

In `crates/sdk/src/storage/mod.rs`, declare it under THE-CFG:

```rust
#[cfg(all(
    not(any(doc, target_family = "wasm", target_arch = "riscv32")),
    feature = "std"
))]
pub(crate) mod live_guards;
```

- [ ] **Step 4: Hook the three sites (each hook under THE-CFG)**

`crates/sdk/src/storage/value.rs` — `load_mut`:

```rust
    pub fn load_mut(&mut self) -> ValueMut<'_, T> {
        let value = self.get();
        #[cfg(all(
            not(any(doc, target_family = "wasm", target_arch = "riscv32")),
            feature = "std"
        ))]
        crate::storage::live_guards::guard_created();
        ValueMut {
            value: self,
            data: value,
        }
    }
```

`Drop for ValueMut`:

```rust
impl<T: __xdrs::WriteXDR> Drop for ValueMut<'_, T> {
    fn drop(&mut self) {
        #[cfg(all(
            not(any(doc, target_family = "wasm", target_arch = "riscv32")),
            feature = "std"
        ))]
        crate::storage::live_guards::guard_dropped();
        self.value.set(&self.data);
    }
}
```

`crates/sdk/src/internal/terminal_return.rs` — top of `contract_return` (the single funnel; `contract_return_buf`/`contract_return_encoded` route through it):

```rust
pub fn contract_return(buf: &[u8], auth_token_ids: &[u32]) -> ! {
    #[cfg(all(
        not(any(doc, target_family = "wasm", target_arch = "riscv32")),
        feature = "std"
    ))]
    crate::storage::live_guards::assert_no_live_guards();

    let revert = false;
    // ... rest unchanged
```

`crates/sdk/src/call/calls.rs` — in `invoke_frame`, wrap the syscall (currently line 294-295):

```rust
    #[cfg(all(
        not(any(doc, target_family = "wasm", target_arch = "riscv32")),
        feature = "std"
    ))]
    let _frame_guard_reset =
        crate::storage::live_guards::FrameGuardReset::new();
    let result =
        sys::raw_Invoke(address, args, auth_bytes, &inputs, &mut outputs);
    #[cfg(all(
        not(any(doc, target_family = "wasm", target_arch = "riscv32")),
        feature = "std"
    ))]
    drop(_frame_guard_reset);
```

- [ ] **Step 5: Run the two tests**

Run: `cd example-contracts && cargo test -p storage_errors early_return guard_across 2>&1 | tail -10`
Expected: BOTH PASS. `early_return_with_live_guard_fails_loudly` now sees the "live ValueMut" panic (it fires before the mock's Return-unwind). `guard_across_nested_call` still passes thanks to `FrameGuardReset` — if you implemented the check before the reset, this test fails with the false positive, which is the reset's regression proof; make sure the final state has both.

- [ ] **Step 6: Prove zero guest presence**

Run: `cargo build -p sdk --target wasm32-unknown-unknown --no-default-features 2>/dev/null || true` — if the direct build isn't configured for this invocation, instead verify by cfg inspection:
`cargo expand`-free check: `grep -rn "live_guards" crates/sdk/src | grep -v "cfg(all(" | grep -v live_guards.rs`
Expected: every use site is immediately preceded by THE-CFG attribute (manual eyeball of the grep hits — there are exactly 4 files: storage/mod.rs, storage/value.rs (x2), internal/terminal_return.rs, call/calls.rs). Then run the existing guest-artifact discipline check:
Run: `bash scripts/check-storage-registry-stripped.sh 2>&1 | tail -2` (builds a real wasm guest; SKIP is acceptable if the toolchain isn't installed, FAIL is not). Additionally: `strings target/wasm32-unknown-unknown/release/complex_storage.wasm 2>/dev/null | grep -i "live ValueMut" ; echo "exit=$?"` — Expected: no match (exit=1) when the artifact exists.

- [ ] **Step 7: Full sdk + examples test run**

Run: `cargo test -p sdk --features dev 2>&1 | tail -3 && cd example-contracts && cargo test 2>&1 | tail -3`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
cd /Users/geofframseyer/groundhog/agent1/sdk_rs
cargo fmt
git add crates/sdk example-contracts/storage_errors
git commit -m "sdk: native-only live-ValueMut check at terminal return

The mock implements Return as unwind, so a ValueMut flush lost on a
real guest is invisible to native tests (audit #7). A thread-local
guard counter (native test env only -- zero guest bytes) makes
contract_return panic on a live guard; invoke_frame zeroes/restores it
around nested mock frames so guards legitimately held across calls
don't false-positive."
```

---

### Task 5: Cross-crate storage-domain collision detection

**Files:**
- Modify: `crates/sdk/src/storage/domain_registry.rs` (add the macro)
- Create: `example-contracts/counter-rv32/tests/storage_domains.rs`
- Modify: `example-contracts/counter-rv32/Cargo.toml` (dev-deps)
- Modify: `example-contracts/counter-rv32/src/main.rs` (doc comment: the test is part of the pattern)
- Create: `example-contracts/cross_crate_collision/{Cargo.toml, alpha/*, beta/*, wrapper/*}`
- Modify: `scripts/check-domain-collision-detected.sh`

**Interfaces:**
- Consumes: existing `sdk::storage::assert_unique_storage_domains()` and the linkme `STORAGE_DOMAINS` registry (native-only, not feature-gated).
- Produces: `sdk::storage_domain_uniqueness_test!()` — a `#[macro_export]` macro expanding to one native `#[test]`.

- [ ] **Step 1: Add the macro**

In `crates/sdk/src/storage/domain_registry.rs`, append:

```rust
/// Declare a native test asserting that no two contract state roots
/// **linked into this binary** share a storage domain.
///
/// The per-crate test generated by `#[contract]` only sees its own crate's
/// roots. A guest binary that links several contract crates (the rv32
/// bin-wrapper pattern) aggregates all of them — put this in the wrapper
/// crate's `tests/` alongside a `use <contract_crate> as _;` for each
/// linked contract crate so their registrations are linked in:
///
/// ```ignore
/// use counter as _;
/// sdk::storage_domain_uniqueness_test!();
/// ```
#[macro_export]
macro_rules! storage_domain_uniqueness_test {
    () => {
        #[cfg(all(
            test,
            not(any(target_family = "wasm", target_arch = "riscv32"))
        ))]
        #[test]
        fn storage_domains_unique_across_linked_crates() {
            $crate::storage::assert_unique_storage_domains();
        }
    };
}
```

Note: `domain_registry.rs` itself is compiled host-only, but `#[macro_export]` macros must be visible on all targets — if the module is cfg'd out on guest targets, move the `macro_rules!` to a location compiled unconditionally (e.g. `crates/sdk/src/local.rs`'s pattern, or directly in `lib.rs`); check with `grep -n "domain_registry" crates/sdk/src/storage/mod.rs` and place accordingly. The macro body is safe on guests because it expands to a `cfg`'d-out item.

- [ ] **Step 2: Wire it into counter-rv32 (the canonical wrapper)**

`example-contracts/counter-rv32/tests/storage_domains.rs`:

```rust
//! Cross-crate storage-domain uniqueness — part of the bin-wrapper
//! pattern: every contract crate linked into the guest bin must be
//! `use`d here so its root registrations link into this test binary.

use counter as _;

sdk::storage_domain_uniqueness_test!();
```

`example-contracts/counter-rv32/Cargo.toml`, append:

```toml
[dev-dependencies]
counter = { path = "../counter", features = ["dev"] }
```

In `example-contracts/counter-rv32/src/main.rs`, extend the "its job is to:" doc list with a third point:

```
//!   3. Host the cross-crate storage-domain uniqueness test
//!      (`tests/storage_domains.rs`): per-crate lib tests can't see
//!      sibling contract crates' storage roots, so the wrapper — the one
//!      place everything links together — asserts no two roots share a
//!      domain.
```

- [ ] **Step 3: Run the counter-rv32 test (should pass — no collision)**

Run: `cd example-contracts && cargo test -p counter-rv32 2>&1 | tail -5`
Expected: PASS, 1 test (`storage_domains_unique_across_linked_crates`).

- [ ] **Step 4: Create the colliding fixture (workspace-excluded, mirrors multi_root_collision)**

`example-contracts/cross_crate_collision/Cargo.toml`:

```toml
# Stand-alone workspace: deliberately-colliding fixture, only ever built by
# scripts/check-domain-collision-detected.sh (which asserts its test FAILS).
[workspace]
members = ["alpha", "beta", "wrapper"]
resolver = "2"
```

`example-contracts/cross_crate_collision/alpha/Cargo.toml` (and `beta/`, s/alpha/beta/):

```toml
[package]
name = "alpha"
description = "Cross-crate collision fixture: root named Registry, default domain"
version = "0.4.0"
authors = ["Groundhog Team"]
license = "Apache-2.0"
publish = false
edition = "2024"

[features]
entrypoints = []
dev = ["sdk/dev"]

[lib]
crate-type = ["rlib"]
doctest = false
test = false

[dependencies]
sdk = { path = "../../../crates/sdk", registry = "groundhog", version = "=0.17.1" }
```

(Copy the exact sdk `version = "=X.Y.Z"` pin from `example-contracts/multi_root_collision/Cargo.toml` at implementation time — it moves with releases. `test = false` here is deliberate: the *per-crate* generated collision test isn't what's under test, the wrapper's cross-crate one is.)

`example-contracts/cross_crate_collision/alpha/src/lib.rs` (and `beta/`, s/alpha::a/beta::b/ on the method name):

```rust
#![cfg_attr(not(feature = "dev"), no_std)]

use sdk::storage::Value;
use sdk::*;

// Same ident in both crates, no #[storage_domain] override: the default
// domain is the bare ident, so alpha::Registry and beta::Registry collide.
#[sdk::storage]
pub struct Registry {
    v: Value<u64>,
}

#[contract]
impl Registry {
    pub fn a(&self) -> u64 {
        self.v.try_get().none_if_missing().unwrap_or(0)
    }
}
```

(In `beta`, name the method `b`.)

`example-contracts/cross_crate_collision/wrapper/Cargo.toml`:

```toml
[package]
name = "wrapper"
description = "Bin wrapper linking alpha+beta; its uniqueness test must FAIL"
version = "0.4.0"
authors = ["Groundhog Team"]
license = "Apache-2.0"
publish = false
edition = "2024"

[features]
entrypoints = ["alpha/entrypoints", "beta/entrypoints"]

[[bin]]
name = "wrapper"
path = "src/main.rs"
required-features = ["entrypoints"]

[dependencies]
alpha = { path = "../alpha" }
beta = { path = "../beta" }
sdk = { path = "../../../crates/sdk", registry = "groundhog", version = "=0.17.1" }

[dev-dependencies]
alpha = { path = "../alpha", features = ["dev"] }
beta = { path = "../beta", features = ["dev"] }
sdk = { path = "../../../crates/sdk", registry = "groundhog", version = "=0.17.1", features = ["dev"] }
```

`example-contracts/cross_crate_collision/wrapper/src/main.rs` (mirrors counter-rv32):

```rust
//! Colliding-fixture bin wrapper (never actually built for rv32 in CI;
//! exists so the fixture is shaped exactly like the real pattern).
#![no_std]
#![no_main]

extern crate alpha;
extern crate beta;
```

`example-contracts/cross_crate_collision/wrapper/tests/storage_domains.rs`:

```rust
use alpha as _;
use beta as _;

sdk::storage_domain_uniqueness_test!();
```

Also add `"cross_crate_collision"` to the `exclude` list in `example-contracts/Cargo.toml` (next to `"multi_root_collision"`).

- [ ] **Step 5: Verify the fixture's test fails with the collision message**

Run: `cd example-contracts/cross_crate_collision/wrapper && cargo test 2>&1 | tail -15`
Expected: FAIL, with panic message containing `storage domain collision` naming `alpha::Registry` and `beta::Registry` (the existing `assert_unique_storage_domains` message).

- [ ] **Step 6: Extend the CI script**

In `scripts/check-domain-collision-detected.sh`, generalize to check both fixtures — replace the body after `SCRIPT_DIR=` with:

```bash
check_collision_detected() {
    local crate_dir="$1"
    local test_cmd="$2"
    local out status
    out=$(cd "$crate_dir" && eval "$test_cmd" 2>&1)
    status=$?

    if [ "$status" -eq 0 ]; then
        echo "FAIL: expected a storage domain collision in $crate_dir, but tests passed" >&2
        echo "$out" >&2
        exit 1
    fi
    if ! echo "$out" | grep -q "storage domain collision"; then
        echo "FAIL: $crate_dir tests failed but not with the collision message" >&2
        echo "$out" >&2
        exit 1
    fi
    echo "OK: storage domain collision detected in $crate_dir"
}

# Within-crate: two roots sharing a domain in one crate (generated lib test).
check_collision_detected \
    "${SCRIPT_DIR}/../example-contracts/multi_root_collision" \
    "cargo test --features dev"

# Cross-crate: two contract crates linked into one bin-wrapper; caught by
# the wrapper's sdk::storage_domain_uniqueness_test!() aggregation test.
check_collision_detected \
    "${SCRIPT_DIR}/../example-contracts/cross_crate_collision/wrapper" \
    "cargo test"
```

Keep the header comment; update it to mention both fixtures.

- [ ] **Step 7: Run the script**

Run: `bash scripts/check-domain-collision-detected.sh`
Expected: two `OK: storage domain collision detected in ...` lines, exit 0.

- [ ] **Step 8: Commit**

```bash
cd /Users/geofframseyer/groundhog/agent1/sdk_rs
cargo fmt
git add crates/sdk/src/storage example-contracts/counter-rv32 example-contracts/cross_crate_collision example-contracts/Cargo.toml scripts/check-domain-collision-detected.sh
git commit -m "sdk: cross-crate storage-domain collision detection for bin wrappers

Per-crate lib tests only see their own crate's storage roots; two
contract crates linked into one rv32 bin wrapper were checked by
nothing (audit #4). storage_domain_uniqueness_test!() reuses the
native linkme registry at the aggregation point; counter-rv32 adopts
it as part of the documented pattern, and a workspace-excluded
cross_crate_collision fixture + CI script assert detection works."
```

---

### Task 6: Full verification, PR, CI

**Files:** none (verification + PR only)

- [ ] **Step 1: Run the repo's own check pipeline**

Run: `bash -c 'source scripts/dev.sh 2>/dev/null; check_all' 2>&1 | tail -20` — if `dev.sh` isn't sourceable that way, check its usage header (`head -40 scripts/dev.sh`) and run the documented all-checks entrypoint. Time-boxed alternative if the full pipeline is impractical locally: `cargo test --workspace --features dev`, `cd example-contracts && cargo test`, `bash scripts/check-domain-collision-detected.sh`, `cd integration_test && cargo test`, plus `cargo doc --features=dev` (docs are `--deny warnings` in CI — the doc-comment edits in Tasks 3/5 must not break it).
Expected: all green.

- [ ] **Step 2: Verify the working tree is clean and the spec/plan docs are committed**

```bash
git add docs/superpowers/specs/2026-07-11-sdk-footgun-fixes-design.md docs/superpowers/plans/2026-07-11-sdk-footgun-fixes.md
git commit -m "docs: spec + plan for the four sdk footgun fixes"
git status --short
```

Expected: clean tree (pre-existing untracked docs like `docs/sdk-api-audit-2026-07-11.md` may remain — leave them).

- [ ] **Step 3: Push and open the PR**

```bash
git push -u origin sdk-footgun-fixes
gh pr create --title "Fix four SDK footguns: generated-local hygiene, Vector::len, cross-crate domain collisions, contract_return*" --body "$(cat <<'EOF'
From the 2026-07-11 public-API audit. Spec: docs/superpowers/specs/2026-07-11-sdk-footgun-fixes-design.md

- **macros**: generated client/wrapper locals renamed to hygienic `__gh_*` — a user param named `entrypoint` was silently shadowed and the client sent the generated ID instead of the argument (audit #5). New `codegen_hygiene` fixture.
- **Vector::len()**: `.none_if_missing()` before `unwrap_or(0)` — corrupted/aliased len slots abort typed instead of reading as "empty vector" and letting the next push clobber real state (audit #6).
- **storage domains**: new `sdk::storage_domain_uniqueness_test!()` puts the existing linkme registry check at the bin-wrapper aggregation point, catching cross-crate collisions per-crate lib tests can't see (audit #4). counter-rv32 adopts it as part of the pattern; workspace-excluded `cross_crate_collision` fixture + extended CI script assert detection.
- **contract_return***: moved to `sdk::internal` (doc-hidden plumbing; dev_wallet + sdk tests updated), and a **native-test-only** live-`ValueMut` counter makes `contract_return` fail loudly when a deferred flush would be silently discarded on a real guest — previously invisible to tests because the mock unwinds (audit #7). Zero guest bytes: every piece is cfg'd off wasm/rv32.
EOF
)"
```

- [ ] **Step 4: Babysit CI to green**

Invoke the repo's CI skill: `Skill(check-ci)` — monitor checks, fix failures, address Greptile review comments. Iterate until the PR is green.

---

## Self-Review Notes

- Spec coverage: §1→Task 1, §2→Task 2, §3→Task 5, §4→Tasks 3+4, PR goal→Task 6. Non-goals respected (no XDR/CLI changes anywhere).
- Known judgment points for the executor (validated intents, exact surfaces may differ):
  - Task 1 Step 6: macros package name — verify via its Cargo.toml.
  - Task 2/4 fixture tests: match `storage_errors/tests/test.rs`'s existing setup/nonce helpers and by-ref client arg conventions rather than inventing new ones.
  - Task 4 early-return test: the panic may surface to the test as a direct unwind or wrapped in the transact result — the catch_unwind + re-panic shape asserts on the message either way; keep the "live ValueMut" substring assertion.
  - Task 5 Step 1: if `domain_registry` is cfg'd out of guest compilation at the module level, home the `macro_rules!` somewhere unconditionally compiled.
  - Task 5 fixture: copy the current sdk version pin from `multi_root_collision/Cargo.toml`.
