# SDK footgun fixes — design (2026-07-11)

Four fixes from the 2026-07-11 public-API audit (`docs/sdk-api-audit-2026-07-11.md`),
scoped per discussion. Hard constraint throughout: **zero on-chain (guest) cost** —
no added bytes or instructions in wasm/rv32 release artifacts.

## 1. Hygienic locals in generated code

**Problem (audit #5):** generated client methods bind `let entrypoint = <id>` before
capturing user args, silently shadowing a user parameter named `entrypoint` (any
type — the args tuple is untyped, so inference follows the shadow). A proxy-style
`fn route(&self, entrypoint: ContractEntrypointID)` compiles and sends the
method's own generated ID instead of the argument. Guest-side, the generated
`state`/`state_node` locals collide with a user param named `state` (fails to
compile today, but same hygiene defect).

**Fix:** rename generated locals to the existing `__gh_` convention (matching
`__gh_calldata`, `__args`):
- `crates/macros/src/common/client.rs` (both the plain and `try_` generated
  methods): `entrypoint` → `__gh_entrypoint`, `args` → `__gh_args`.
- `crates/macros/src/common/exports.rs` (`load_state` + `invoke` quotes):
  `state_node` → `__gh_state_node`, `state` → `__gh_state`.

**Tests:**
- Client interface with a method taking a param named `entrypoint:
  ContractEntrypointID` (proxy pattern); mock-based test asserts the *argument's*
  ID is sent on the wire, not the method's generated ID.
- Contract method with params named `state` and `args` compiles and dispatches
  correctly guest-side (native mock test suffices; the wrapper code is
  target-independent).

## 2. `Vector::len()` must not swallow type-confusion errors

**Problem (audit #6):** `crates/sdk/src/storage/vector.rs:62` is
`self.len.try_get().unwrap_or(0)` — wrong-storage-type and decode failures read
as "empty vector", and the next push clobbers whatever actually lives there.
Every other typed accessor aborts on type confusion.

**Fix (one line):** `self.len.try_get().none_if_missing().unwrap_or(0)`.
Missing key still reads as 0 (a never-written vector is empty); any other error
re-throws typed, via the existing `NoneIfMissing` tool. Doc note on `len()`
stating the distinction.

**Tests:** write a value of the wrong storage type (and separately, undecodable
bytes) at the len slot; assert `len()` aborts typed instead of returning 0.
Missing-key still returns 0.

## 3. Cross-crate storage-domain collision detection

**Problem (audit #4):** the native linkme registry + generated uniqueness test
detect two roots sharing a domain only *within one crate's lib tests*. In the
recommended rv32 bin-wrapper pattern, two contract lib crates linked into one
guest bin are visible to no check: lib tests link only their own crate's
registrations, and the wrapper bin (`no_std`/`no_main`, rv32-only) has no tests.

**Decision:** no spec/XDR/CLI changes, no runtime check. Reuse the existing
linkme registry at the aggregation point: a native test *in the bin-wrapper
crate* links all its contract-crate deps, so `distributed_slice` collects every
registration and the existing `assert_unique_storage_domains()` sees them all,
with its existing readable message.

**Fix:**
- New public macro `sdk::storage_domain_uniqueness_test!()` expanding to a
  correctly-cfg'd (`not(wasm)`, `not(rv32)`) `#[test]` fn calling
  `sdk::storage::assert_unique_storage_domains()`. One line per wrapper crate;
  the registration statics are not gated on the `entrypoints` feature, so a
  plain `cargo test` in the wrapper crate exercises it.
- `counter-rv32` gains `tests/storage_domains.rs` using the macro (the test
  target links the contract crates via normal deps; `required-features` on the
  `[[bin]]` does not gate test targets). The bin-wrapper pattern docs
  (counter-rv32 main.rs doc comment + sdk macro docs) document it as part of
  the pattern.
- New workspace-excluded example `example-contracts/cross_crate_collision/`:
  two contract lib crates whose roots share a default domain (same struct
  ident), plus a wrapper crate with the uniqueness test. Mirrors
  `multi_root_collision` exactly.
- `scripts/check-domain-collision-detected.sh` extended (or a sibling script,
  wired at the same `dev.sh` call site) to run the cross-crate example's test
  and assert it FAILS with the "storage domain collision" message — same
  CI-asserts-failure pattern as the within-crate check.

**Residual gap (accepted):** the check is part of the documented pattern, not
enforced on authors who omit the test line. CLI-side enforcement can come later
if wanted; out of scope here.

## 4. `contract_return*`: remove from public API + native live-guard check

**Problem (audit #7):** the public `contract_return*` fns diverge via syscall,
skipping `Drop` — a live `ValueMut`'s flush is silently discarded on the
*success* path. The native mock implements Return as unwind, so destructors DO
run in native tests: the bug is invisible to testing and only manifests on real
guests. Macro-generated code is structurally safe (user method returns by
value; its locals drop before the wrapper encodes) — the exposure is direct
calls to the public fns (early-exit inside a method body, raw forwarding
contracts).

**Fix (two independent parts):**

*Surface reduction:* `contract_return`, `contract_return_buf`, and
`contract_return_encoded` move out of the public API into SDK plumbing —
`#[doc(hidden)]` and re-homed under `sdk::internal` (they must stay `pub`:
generated wrappers in user crates call `contract_return_encoded`, and
cross-crate in-repo callers exist). Callers updated:
- `crates/macros/src/common/exports.rs` generated path →
  `::sdk::internal::contract_return_encoded`.
- `integration_test/contracts/dev_wallet` forwarding →
  `sdk::internal::contract_return` (dev_wallet is ours to change).
- `crates/sdk/tests/{aborts,large_encodes,return_encode_error}.rs` updated
  paths.
- Old paths removed outright (pre-1.0; no deprecation shim).
- Loud doc warning on the hidden fns: diverging skips Drop; no live
  deferred-write guards (`ValueMut`) may be held.
- `contract_abort`/`contract_revert` unchanged (aborts revert the frame, so a
  skipped flush is correct semantics, and they are legitimate public surface).

*Native-only live-guard check:* a `ValueMut` guard counter that **does not
exist in guest builds** — every piece under
`cfg(not(any(target_family = "wasm", target_arch = "riscv32")))`, so zero
on-chain cost by construction (eyeball-verifiable in review):
- thread-local counter (plain `std::thread_local!`; no need for
  `contract_local!` since it's native-only);
- `ValueMut` creation increments, `Drop` decrements;
- `contract_return` (the single funnel under all three fns) panics with a
  clear message if the counter is nonzero — this fires *before* the mock's
  Return-unwind, so native tests now catch exactly what would corrupt state on
  a real guest;
- `invoke_frame` saves/zeroes/restores the counter around `raw_Invoke` (RAII
  guard), so a caller legitimately holding a `ValueMut` across a nested
  in-process mock frame does not false-positive (mirrors real-guest semantics,
  where the callee has its own memory).

**Tests (native-exec):**
- Entrypoint holds a `ValueMut` and calls the internal return fn → frame fails
  with the live-guard message (this is the regression test for the audit bug).
- Caller holds a `ValueMut` across a client call to a callee that returns
  normally → no false positive; caller's flush lands on its own return.
- Guard dropped before early-exit return → succeeds.

## Non-goals

- Wasm multi-crate collision detection beyond what the same macro-in-a-test
  provides (same mechanism works if anyone builds that shape).
- Deprecation shims for the moved `contract_return*` paths (pre-1.0 crate,
  coordinated workspace).
- CLI/spec/protocol changes of any kind.
