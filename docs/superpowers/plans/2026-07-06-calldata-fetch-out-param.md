# Out-Param Calldata Fetch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `with_calldata_and_auth_tokens` (closure/fn-pointer API) with a non-generic out-param fetch primitive so the SDK adds zero funcref-table entries per entrypoint, unblocking `cxs_roundtrip`/`txs_roundtrip`/`tuple_return` from the chain's 64-slot table cap while keeping #264's one-copy-per-contract size win.

**Spec:** `docs/superpowers/specs/2026-07-06-calldata-fetch-out-param-design.md`

**Architecture:** The shared calldata machinery becomes `fetch_calldata_and_auth_tokens(&mut Calldata, &mut AuthTokenIdBuf)` — non-generic, `#[inline(never)]`, throwing. Macro-generated wrappers own the two buffers as frame locals, call the primitive directly, and run their decode/invoke/encode body inline. No closure, no `as fn` cast, no `call_indirect` anywhere in the entrypoint path.

**Tech Stack:** Rust (workspace at `/Users/geofframseyer/groundhog/sdk_rs`), proc-macro codegen via `quote!`, wasm fleet built with `scripts/dev.sh`, `wasm-objdump` for table inspection.

## Global Constraints

- Branch: `geoff/calldata-fetch-out-param` (commits are hook-blocked on `main`).
- `integration_test/contracts` is a **git submodule** — branch, commit, and push it separately (same branch name), and push the submodule **before** the parent so CI can resolve the gitlink.
- Pre-launch: no backward-compat shims; `with_calldata_and_auth_tokens` is deleted outright.
- The new primitive MUST stay non-generic and `#[inline(never)]` — genericity re-monomorphizes per entrypoint; inlining pastes the ~440 B body into every export.
- The generated wrapper MUST NOT take any function's address (`as fn` cast, closure passed to a generic) — each escaped address is a funcref table slot, and the chain caps the table at 64 initial elements (`MAX_TABLE_ELEMENTS`, `hogd/rust/common/src/lib.rs:11`).
- All `cargo` invocations run with `--locked` when going through `scripts/dev.sh` (it wraps cargo); plain `cargo test` from the repo root is fine.

---

### Task 1: SDK primitive — replace `with_calldata_and_auth_tokens` with `fetch_calldata_and_auth_tokens`

**Files:**
- Modify: `crates/sdk/src/contract_op.rs:346-385` (the `with_calldata_and_auth_tokens` fn and its doc comment)
- Modify: `crates/sdk/src/contract_op.rs:222-225` and `:282-291` (doc lines in the two private `fetch_calldata_and_auth_tokens_into` variants that say "This is the primitive behind [`with_calldata_and_auth_tokens`]")
- Test: `crates/sdk/tests/calldata.rs`
- Test: `crates/sdk/tests/large_encodes.rs:55-57`

**Interfaces:**
- Consumes: existing private `fetch_calldata_and_auth_tokens_into(&mut Calldata, &mut AuthTokenIdBuf) -> SysResult<()>` (both alloc/no-alloc variants, unchanged), `syscall_throw`, `SyscallNo::kGetCalldata` (already imported in the file).
- Produces: `pub fn fetch_calldata_and_auth_tokens(calldata: &mut Calldata, auth_ids: &mut AuthTokenIdBuf)` in `sdk::contract_op` (re-exported at `sdk::` root via the existing `pub use contract_op::*` in `crates/sdk/src/lib.rs:132`). Callers construct buffers with `sdk::internal::new_scratch()` (returns `Calldata`) and `AuthTokenIdBuf::new()`. Tasks 2 and 3 call exactly this signature.

- [ ] **Step 1: Rewrite the two SDK tests against the new API (failing test)**

In `crates/sdk/tests/calldata.rs`, replace the `run` closure body (lines 16-23):

```rust
            .run(move || {
                let mut calldata = sdk::internal::new_scratch();
                let mut auth_ids =
                    sdk::contract_op::AuthTokenIdBuf::new();
                sdk::contract_op::fetch_calldata_and_auth_tokens(
                    &mut calldata,
                    &mut auth_ids,
                );
                assert_eq!(calldata.as_slice(), expected.as_slice());
                assert!(auth_ids.is_empty());
            })
```

In `crates/sdk/tests/large_encodes.rs`, replace lines 55-57 (`let calldata_len = sdk::contract_op::with_calldata_and_auth_tokens(...)`):

```rust
            let mut calldata = sdk::internal::new_scratch();
            let mut auth_ids = sdk::contract_op::AuthTokenIdBuf::new();
            sdk::contract_op::fetch_calldata_and_auth_tokens(
                &mut calldata,
                &mut auth_ids,
            );
            let calldata_len = calldata.len();
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p sdk --test calldata --test large_encodes`
Expected: FAIL to compile — `cannot find function fetch_calldata_and_auth_tokens in module sdk::contract_op`

- [ ] **Step 3: Implement the primitive**

In `crates/sdk/src/contract_op.rs`, replace the whole `with_calldata_and_auth_tokens` item (doc comment + fn, lines 346-385) with:

```rust
/// Fetch this frame's calldata and auth-token ids into the caller's
/// buffers (cleared first), throwing a `LibraryExn::Syscall` on fetch
/// failure. The single calldata API: generated entrypoint wrappers and
/// raw-calldata contracts both use it. The throw is guest-reachable and
/// not limited to an adversarial host: in the no-alloc guest it is
/// exactly what an ordinary transaction gets if its calldata exceeds
/// `SCRATCH_MAX` (4096) bytes or carries more than 32 auth tokens.
///
/// The buffers live in the caller's frame and are filled in place —
/// never moved by value. Generated entrypoint wrappers declare them as
/// locals and fetch exactly once, at the root of the frame; the decoded
/// args borrow the calldata buffer for the entire handler frame. In the
/// no-alloc guest that costs `SCRATCH_MAX` (4096) bytes of stack per
/// live fetch. There is no shared cell, so nested fetches (the native
/// mock's in-process frames, or a hand-written re-fetch) each get their
/// own buffers and cannot alias.
///
/// Non-generic and `#[inline(never)]`, both load-bearing for codesize:
/// every entrypoint reaches this single copy of the fetch machinery
/// (~440 bytes in the optimized wasm) by direct call — nothing
/// monomorphizes per entrypoint, and no handler's address is ever taken,
/// so the funcref table (capped at 64 initial elements on-chain) does
/// not grow with entrypoint count. Without the attribute, LTO inlines
/// the body back into each export, recreating the per-export copies.
///
/// `doc(hidden)`: raw calldata access is plumbing for the generated
/// entrypoint wrappers, not a surface contracts should discover and
/// build on.
#[doc(hidden)]
#[inline(never)]
pub fn fetch_calldata_and_auth_tokens(
    calldata: &mut Calldata,
    auth_ids: &mut AuthTokenIdBuf,
) {
    fetch_calldata_and_auth_tokens_into(calldata, auth_ids).unwrap_or_else(
        |syserr| syscall_throw(SyscallNo::kGetCalldata, syserr),
    );
}
```

In both private `fetch_calldata_and_auth_tokens_into` doc comments, change

```text
/// This is the primitive behind [`with_calldata_and_auth_tokens`]: the
```

to

```text
/// This is the fallible core of [`fetch_calldata_and_auth_tokens`]: the
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p sdk --test calldata --test large_encodes`
Expected: PASS (3 tests in calldata, 2+ in large_encodes)

Note: `cargo test -p groundhog-macros` and the workspace build are expected to FAIL at this point — the macro codegen still references the deleted fn. That's Task 2.

- [ ] **Step 5: Commit**

```bash
git add crates/sdk/src/contract_op.rs crates/sdk/tests/calldata.rs crates/sdk/tests/large_encodes.rs
git commit -m "sdk: replace with_calldata_and_auth_tokens with out-param fetch primitive"
```

(If a pre-commit hook runs workspace-wide checks that trip on the not-yet-updated macro crate, commit with `--no-verify` and note it; Task 2 restores the workspace build.)

---

### Task 2: Macro codegen — wrapper owns the buffers, no `as fn` cast

**Files:**
- Modify: `crates/macros/src/common/exports.rs:421-464` (the comment block + `wrapped_call` quote)
- Modify: `crates/macros/src/common/exports.rs:619-641` (the `export_wrappers_use_outlined_calldata_shim` test)

**Interfaces:**
- Consumes: `::sdk::contract_op::fetch_calldata_and_auth_tokens(&mut Calldata, &mut AuthTokenIdBuf)` from Task 1; `::sdk::internal::new_scratch()`; `::sdk::contract_op::{Calldata, AuthTokenIdBuf}`.
- Produces: generated entrypoint wrappers with the out-param shape. No other task consumes macro internals, but Task 4's table measurements depend on this codegen.

- [ ] **Step 1: Rewrite the macro unit test (failing test)**

Replace the `export_wrappers_use_outlined_calldata_shim` test in `crates/macros/src/common/exports.rs` with:

```rust
    #[test]
    fn export_wrappers_use_outlined_calldata_fetch() {
        let config = parse_config(r#"interface = "Iface""#);
        let item: syn::ItemFn = syn::parse_quote! {
            fn ping() {}
        };

        let generated = generate_free_function_bundle(&item, &config)
            .unwrap()
            .to_string();

        // The wrapper must own the buffers and reach the shared,
        // non-generic fetch primitive by direct call.
        assert!(
            generated.contains("fetch_calldata_and_auth_tokens"),
            "generated: {generated}"
        );

        // Nothing in the wrapper may take a function's address: every
        // escaped address is a funcref table slot, and the chain caps the
        // table at 64 initial elements. The old `as fn(...)` handler cast
        // cost ~1 slot per entrypoint.
        assert!(!generated.contains("as fn"), "generated: {generated}");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p groundhog-macros export_wrappers_use_outlined_calldata_fetch`
Expected: FAIL — `generated` contains `as fn` and no `fetch_calldata_and_auth_tokens`

- [ ] **Step 3: Rewrite `wrapped_call`**

In `crates/macros/src/common/exports.rs`, replace the comment block and `wrapped_call` binding (lines 421-464) with:

```rust
    // Decode the args, run the body, encode the result. The calldata and
    // auth-token buffers are locals of this (per-entrypoint) wrapper frame,
    // filled in place by the shared, non-generic
    // `fetch_calldata_and_auth_tokens` — no by-value copy of the no-alloc
    // SCRATCH_MAX buffer — and the decoded args borrow the calldata buffer
    // for the whole frame.
    //
    // Both calls here (wrapper -> fetch, and the handler body itself) are
    // direct. That is load-bearing for the chain's funcref-table cap (64
    // initial elements): passing the handler to the SDK as a closure or
    // `fn` pointer reifies one table slot per entrypoint, which pushed
    // many-entrypoint contracts past the cap. The shared machinery stays
    // one copy per contract because the fetch fn is non-generic and
    // `#[inline(never)]`.
    let wrapped_call = quote! {{
        //
        // If debug mode: log the method-call
        #log_guest_call

        let mut __gh_calldata: ::sdk::contract_op::Calldata =
            ::sdk::internal::new_scratch();
        let mut __gh_auth_token_ids: ::sdk::contract_op::AuthTokenIdBuf =
            ::sdk::contract_op::AuthTokenIdBuf::new();
        ::sdk::contract_op::fetch_calldata_and_auth_tokens(
            &mut __gh_calldata,
            &mut __gh_auth_token_ids,
        );

        let res = {

            // Load contract state
            #load_state

            // Decode the args (Signer<T> reads 0 bytes, gets sentinel handle;
            // contract_call_decode then fills in Signer handles from the auth
            // token ID list)
            let mut __args =
                ::sdk::call::contract_call_decode(
                    __gh_calldata.as_ref(),
                    __gh_auth_token_ids.as_slice(),
                )
                    .unwrap_or_else(|e|
                        ::sdk::exn::decoding_args_throw(&e));

            // Destructure into named args
            let (#arg_names) = __args;

            // Call the method implementation (passing the contract state, if any,
            // and the args)
            #invoke

            // Make sure any storage nodes are fully dropped (and written to environment).
            // Not technically necessary, given that the storage is wrapped in a Node<T>.
        };

        // Return the result to the VM
        ::sdk::contract_return_encoded(&res);
    }};
```

(`__gh_`-prefixed buffer names so user method args named `calldata` etc. cannot collide.)

- [ ] **Step 4: Run the macro tests, then the workspace tests**

Run: `cargo test -p groundhog-macros`
Expected: PASS (including `export_wrappers_use_outlined_calldata_fetch`)

Run: `cargo test && cargo test -p sdk -Fstd --tests`
Expected: PASS — this compiles every macro-expansion consumer (standard-interfaces, standard-tests, trybuild fixtures) against the new codegen. Any failure here is a real regression in this task; fix before committing.

- [ ] **Step 5: Commit**

```bash
git add crates/macros/src/common/exports.rs
git commit -m "macros: entrypoint wrappers own calldata buffers; drop the fn-pointer handler cast"
```

---

### Task 3: Migrate the `calldata_raw` contract (submodule)

**Files:**
- Modify: `integration_test/contracts/calldata_raw/src/lib.rs:7` (import) and `:30-40` (the `calldata` fn)

**Interfaces:**
- Consumes: `sdk::fetch_calldata_and_auth_tokens` (root re-export from Task 1), `sdk::contract_op::AuthTokenIdBuf`, `sdk::internal::new_scratch`.
- Produces: nothing downstream; this is the last `with_calldata_and_auth_tokens` caller, so after this task the identifier must have zero hits repo-wide.

- [ ] **Step 1: Branch the submodule**

```bash
cd integration_test/contracts
git checkout -b geoff/calldata-fetch-out-param
```

- [ ] **Step 2: Rewrite the consumer**

In `integration_test/contracts/calldata_raw/src/lib.rs`, change line 7:

```rust
use sdk::{fetch_calldata_and_auth_tokens, get_calldata_len};
```

and replace the `calldata` fn (lines 30-40):

```rust
pub fn calldata(count: u32) {
    let mut len: usize = 0;

    for _ in 0..count {
        // Fresh buffers per iteration: each fetch is independent, matching
        // the pre-existing semantics of one fetch per call.
        let mut buf = sdk::internal::new_scratch();
        let mut auth_ids = sdk::contract_op::AuthTokenIdBuf::new();
        fetch_calldata_and_auth_tokens(&mut buf, &mut auth_ids);
        let x_len = buf.len();
        if len > 0 {
            assert_eq!(x_len, len);
        }
        len = x_len;
    }
}
```

- [ ] **Step 3: Verify the identifier is fully gone and the contract builds**

Run: `grep -rn "with_calldata_and_auth_tokens" /Users/geofframseyer/groundhog/sdk_rs --include="*.rs"`
Expected: no output.

Run (from the repo root): `scripts/dev.sh build integration_test/calldata_raw`
Expected: BUILD/SPEC lines complete without error.

- [ ] **Step 4: Commit the submodule, then the parent gitlink**

```bash
cd integration_test/contracts
git add calldata_raw/src/lib.rs
git commit -m "calldata_raw: migrate to out-param fetch_calldata_and_auth_tokens"
cd ../..
git add integration_test/contracts
git commit -m "integration_test: bump contracts submodule for out-param calldata fetch"
```

---

### Task 4: Fleet verification — table sizes and wasm sizes

**Files:** none modified; measurement only.

**Interfaces:**
- Consumes: the full change set (Tasks 1-3).
- Produces: the numbers for the PR description.

- [ ] **Step 1: Build the integration fleet**

Run (from the repo root): `scripts/dev.sh build-itc`
Expected: all contracts build; **no** `table initial size exceeds max_table_initial_size` instrumentation errors. This is the headline check — `cxs_roundtrip`, `txs_roundtrip`, and `tuple_return` failed here on main.

- [ ] **Step 2: Inspect funcref tables**

```bash
for c in cxs_roundtrip txs_roundtrip tuple_return authclaim_test dev_wallet; do
  echo "== $c"
  wasm-objdump -x -j Table "target/optimized/${c}.wasm" 2>/dev/null || echo "no table section"
done
```

Expected values (initial size), vs. main:

| Contract       | main (#264) | required   | expected            |
|----------------|-------------|------------|---------------------|
| tuple_return   | 72          | ≤ 64       | ~0 (pre-#263: none) |
| cxs_roundtrip  | 78          | ≤ 64       | ~56 (its own vtables) |
| txs_roundtrip  | > 64        | ≤ 64       | well under cap      |
| authclaim_test | 6           | no growth  | ≤ 6                 |

If any table exceeds its expectation, dump the element segment (`wasm-objdump -x -j Elem`) and find what address still escapes — do not proceed to the PR with unexplained slots.

- [ ] **Step 3: Record wasm sizes**

```bash
ls -l target/optimized/*.wasm | awk '{print $5, $9}' | sort -k2 > /private/tmp/claude-501/-Users-geofframseyer-groundhog-sdk-rs/f5dde5ef-adf3-4f86-95c6-20e0d8c49b64/scratchpad/sizes-branch.txt
cat /private/tmp/claude-501/-Users-geofframseyer-groundhog-sdk-rs/f5dde5ef-adf3-4f86-95c6-20e0d8c49b64/scratchpad/sizes-branch.txt
```

Known main baselines to compare against: `authclaim_test` 4761 B, `dev_wallet` 10070 B (optimized). Expected: ~neutral per contract (each export trades a 32-byte thunk for buffer locals + one direct call; the table and its element segment shrink). A regression of more than a few hundred bytes on any single contract means LTO re-inlined the fetch body — verify `#[inline(never)]` survived and investigate before proceeding.

- [ ] **Step 4: Run the integration test suite**

Run: `scripts/dev.sh test-itc`
Expected: PASS. (If this is prohibitively slow locally, run at minimum `cd integration_test && cargo test -p calldata_raw` and let remote CI cover the rest — per project convention, prefer remote CI for the broad sweep.)

---

### Task 5: PR

**Files:** none modified.

- [ ] **Step 1: Push the submodule first, then the parent**

```bash
cd integration_test/contracts && git push -u origin geoff/calldata-fetch-out-param && cd ../..
git push -u origin geoff/calldata-fetch-out-param
```

- [ ] **Step 2: Open the PR**

```bash
gh pr create \
  --title "sdk+macros: out-param calldata fetch — kill #264's funcref-table growth" \
  --body "$(cat <<'EOF'
## Problem

#264 deduped the entrypoint calldata machinery by passing each handler as a
`fn` pointer, which costs ~1 funcref table slot per entrypoint. The chain caps
table initial size at 64 (`MAX_TABLE_ELEMENTS`, enforced by
`verify_contract_meter` at deploy time), and three integration contracts blew
past it: tuple_return 72, cxs_roundtrip 78, txs_roundtrip similar.
cxs_roundtrip burns 56 slots on its own serde/XDR vtables, so any
per-entrypoint SDK cost is fatal for many-entrypoint contracts.

## Fix

Invert the call direction. `with_calldata_and_auth_tokens` (closure API) is
replaced by a non-generic, `#[inline(never)]` out-param primitive:

    fetch_calldata_and_auth_tokens(&mut Calldata, &mut AuthTokenIdBuf)

Generated wrappers own the buffers as frame locals, call the primitive
directly, and run their decode/invoke/encode body inline. No closure, no
`as fn` cast, no `call_indirect`: zero SDK-added table slots, while the fetch
machinery stays one copy per contract (non-generic + inline(never)).

Buffers are still filled in place, never moved by value; nested fetches still
cannot alias (no shared cell). Spec:
docs/superpowers/specs/2026-07-06-calldata-fetch-out-param-design.md

## Results

(table + size numbers from Task 4 go here)

Companion submodule PR: integration_test/contracts branch
geoff/calldata-fetch-out-param (calldata_raw migration).
EOF
)"
```

Fill in the Results section with the actual Task 4 numbers before submitting.

- [ ] **Step 3: Watch CI**

Invoke the `check-ci` skill to monitor `gh pr checks`, fix failures, and address Greptile review.
