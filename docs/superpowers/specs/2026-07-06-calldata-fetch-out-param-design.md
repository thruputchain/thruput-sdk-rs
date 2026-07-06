# Calldata fetch as out-param primitive: eliminate the funcref table growth from #264

**Date:** 2026-07-06
**Status:** Approved

## Problem

PR #264 deduped the per-entrypoint calldata machinery by erasing each generated
handler closure to a plain `fn` pointer (`as fn(&Calldata, &AuthTokenIdBuf)`),
so every entrypoint shares one instantiation of
`with_calldata_and_auth_tokens`. That shared instantiation receives the handler
as a runtime value and must `call_indirect` it — so each entrypoint handler's
address escapes into the module's funcref table, costing roughly one table slot
per entrypoint.

The chain enforces `MAX_TABLE_ELEMENTS = 64` on the table's initial size at
deploy time (`hogd/rust/common/src/lib.rs:11`, via `verify_contract_meter`);
the CLI instrumentation failure is the same check running earlier. Three
integration contracts now exceed it:

| Contract       | entrypoints | table pre-#263 | at #263 | at #264 (main) |
|----------------|-------------|----------------|---------|----------------|
| tuple_return   | 30          | none           | 46      | 72 ✗           |
| cxs_roundtrip  | 21          | 56             | 62      | 78 ✗           |
| txs_roundtrip  | ~21         | —              | —       | over ✗         |

`cxs_roundtrip` burns 56 slots on its own serde/XDR vtables, so contracts
combining many entrypoints with internal dyn dispatch have no headroom for a
per-entrypoint SDK cost.

## Design

Invert the call direction. The size win from #264 came from sharing one copy
of the fetch machinery, not from fn pointers — fn pointers were only the
mechanism for reaching per-entrypoint code *from* shared code. Instead, each
generated wrapper owns the buffers as locals and calls the shared machinery
directly:

### SDK: new primitive, `with_` API deleted

In `crates/sdk/src/contract_op.rs`:

```rust
/// Fetch this frame's calldata and auth-token ids into the caller's
/// buffers (cleared first), throwing a `LibraryExn::Syscall` for
/// `kGetCalldata` on failure.
#[doc(hidden)]
#[inline(never)]
pub fn fetch_calldata_and_auth_tokens(
    calldata: &mut Calldata,
    auth_ids: &mut AuthTokenIdBuf,
) {
    fetch_calldata_and_auth_tokens_into(calldata, auth_ids)
        .unwrap_or_else(|syserr| syscall_throw(SyscallNo::kGetCalldata, syserr));
}
```

- Non-generic: exactly one copy exists by construction — no closure types to
  erase, nothing for LLVM to monomorphize per entrypoint.
- `#[inline(never)]` stays: LTO must not paste the ~440 B body back into every
  export.
- `#[doc(hidden)]` stays: raw calldata access remains plumbing, not surface.
- `with_calldata_and_auth_tokens` is **deleted** (pre-launch, no backward
  compat). The private `fetch_calldata_and_auth_tokens_into` fns (alloc and
  no-alloc variants) are unchanged.
- Doc comments in `contract_op.rs` that describe the closure API (on
  `Calldata`, `AuthTokenIdBuf`, and the fetch internals) are updated to
  describe the out-param form; buffer-ownership docs move to the new fn:
  buffers are locals of the *generated wrapper's* frame, filled in place,
  never moved by value; nested fetches still cannot alias.

### Macro: wrapper owns the buffers

In `crates/macros/src/common/exports.rs`, `wrapped_call` becomes:

```rust
let mut __calldata: ::sdk::contract_op::Calldata = ::sdk::internal::new_scratch();
let mut __auth_ids: ::sdk::contract_op::AuthTokenIdBuf =
    ::sdk::contract_op::AuthTokenIdBuf::new();
::sdk::contract_op::fetch_calldata_and_auth_tokens(&mut __calldata, &mut __auth_ids);
let calldata = &__calldata;
let auth_token_ids = &__auth_ids;
// ...existing decode / invoke / encode body, unchanged, borrowing the locals
```

- No closure, no `as fn(...)` cast, no `call_indirect`. Both calls
  (wrapper→fetch, and the handler body itself) are direct. **Zero funcref
  table entries added by the SDK.**
- Free-function exports go through the same `generate_method` path — no
  special casing.
- The `export_wrappers_use_outlined_calldata_shim` macro test is rewritten to
  assert the generated code calls `fetch_calldata_and_auth_tokens` and does
  **not** contain an `as fn` cast of the handler.
- Memory accounting is unchanged in substance: the no-alloc 4 KiB buffer
  lives on the shadow stack for the duration of the entrypoint frame, one
  frame up from where #264 put it (the wrapper instead of the shared fn).
  Generated wrappers fetch exactly once, at frame root.

### Other consumers (out-param form)

- `crates/sdk/tests/calldata.rs` — build the two buffers, fetch, assert on
  them directly.
- `crates/sdk/tests/large_encodes.rs` — same; the closure's return value
  (`calldata.len()`) becomes a plain read after the fetch.
- `integration_test/contracts/calldata_raw` (**submodule** — branch and push
  separately) — replace `with_calldata_and_auth_tokens(|x, _| x.len())` with
  the out-param form.

### Unchanged

- The write scratch (`with_scratch`) and its cell: its checkout happens
  mid-frame at arbitrary depth, where a static is the right home.
- rv32 guest dispatch (`GROUNDHOG_RV32_ENTRYPOINTS` distributed slice): fn
  pointers there are rv32, not wasm; no funcref table exists.
- Entrypoint export names, IDs, and the `#[inline(always)]` internal-fn
  structure.

## Error handling

Identical to #264: fetch failure throws `LibraryExn::Syscall` for
`kGetCalldata`. The throw moves from inside `with_calldata_and_auth_tokens`
into the new primitive — same exception, same reachability (no-alloc guest
with calldata > 4096 B or > 32 auth tokens).

## Verification

1. `cargo test` across the workspace (macro expansion tests, sdk tests,
   native mock integration tests).
2. Rebuild the integration fleet with the exact CMake flags
   (`-F entrypoints`): all 33 contracts must pass instrumentation —
   specifically `cxs_roundtrip`, `txs_roundtrip`, `tuple_return` back under
   the 64-slot cap, with table sizes at or below their pre-#263 baselines
   (56 / — / 0).
3. Fleet-wide optimized-wasm size diff vs. main: expected ~neutral (each
   export trades a 32-byte thunk for buffer locals + one direct call; the
   funcref table and its element segment shrink).
