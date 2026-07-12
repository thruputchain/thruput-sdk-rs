# SDK public-API audit — footguns, pain points, codesize (2026-07-11)

Multi-agent workflow audit: 8 parallel reviewers (storage, calls, exceptions, macros,
types, ergonomics, panic/fmt codesize, monomorphization codesize), every finding
adversarially verified by an independent agent against the actual code. 84 raw
findings → 80 confirmed (many with corrections), 4 refuted. Duplicates across
reviewers are merged below. Full machine-readable output (with per-finding verifier
reasoning) was in the workflow task output; this file is the curated synthesis.

---

## Tier 1 — silent-corruption footguns

### 1. Positional storage slots silently remap on field reorder
`crates/macros/src/storage.rs:162` — a `#[storage]` field's slot defaults to its
declaration index. Inserting/reordering fields silently remaps on-chain keys; two
same-typed fields (`Value<u64>`) swap live state with zero diagnostics. The
`#[slot(N)]` escape hatch and the warning live only in a **private** fn's doc comment
(`storage.rs:12-18`) — the public `#[storage]` docs (`crates/sdk/doc/macro-storage.md`,
included at `crates/sdk/src/lib.rs:181`) mention neither. Verifier note: not currently
exploitable on-chain (no upgrade syscall; addresses commit to code hash), but design
docs plan upgrades, and dev workflows with persistent state hit it today.
**Fix:** document `#[slot(N)]` + the hazard on the public rustdoc; consider a strict
mode (`#[storage(pinned)]`) or a layout-fingerprint const for upgrade tooling.

### 2. Wire identifiers silently rotate on pre-1.0 version bumps
Entrypoint IDs, exception selectors, and event signatures (topic 0) all default to
hashing `CARGO_PKG_VERSION`'s "effective major", which for 0.x crates is `0.MINOR`
(and for 0.0.z the **full version**, so even patch bumps rotate). A routine
0.1→0.2 bump silently re-keys every entrypoint, selector, and event topic.
- `crates/macros/src/common/contract.rs:70-72` (interface version default)
- `crates/macros/src/contract_exception.rs:96-112`, `derive_exn.rs:54-69` (selectors)
- `crates/macros/src/derive_event.rs:266-274` (event signatures — no pin/override mechanism exists at all)
- **`standard-interfaces/groundhog-erc20/src/lib.rs:30-34` does not pin `version=`** — the flagship interface re-keys on its own crate bumps.
The public macro docs never mention any of this.
**Fix:** pin `version=` on all standard interfaces; add an event-signature override; document the effective-major rule prominently; consider warning when version is defaulted.

### 3. Entrypoint IDs hash only the method *name*
`crates/contract-util/src/lib.rs:9-38` — `NormalizedMethodSignature` keeps only the
ident (acknowledged TODO at lib.rs:12-16). Changing a method's parameter list within
a major version keeps the same wire ID; structurally-compatible changes (e.g.
reordering two `&Address` params) re-route silently with swapped semantics.
Verifier: for 0.x interfaces a *minor* bump does change the ID; the exposure is
same-effective-major signature changes. The spec section already carries full
signatures — tooling could diff, but nothing does.

### 4. Storage-domain collision check is host-`cfg(test)`-only
`crates/macros/src/common/exports.rs:189-200` — two same-named `#[storage]` structs
(default domain = bare ident) alias the same storage root. Detection is a generated
lib test; verifier confirmed the genuine gaps: `[lib] test = false` crates, pipelines
that only `cargo build`/CLI-build, and **cross-crate collisions** (two contract lib
crates linked into one guest bin — the recommended rv32 pattern — are visible to no
per-crate test). The guest artifact itself is never checked.
**Fix:** check at guest link time (the linkme registry idea) or in the groundhog CLI build path.

### 5. Unhygienic `entrypoint` local in generated client methods
`crates/macros/src/common/client.rs:251-264` — generated methods bind
`let entrypoint = <generated id>` *before* capturing user args, shadowing any user
parameter named `entrypoint` **of any type** (no type annotation on the args tuple, so
inference follows the shadow). A proxy-style `fn route(&self, entrypoint: ContractEntrypointID)`
compiles and silently sends the method's own generated ID instead of the argument.
Only signal: a misleading `unused variable` warning. **Fix:** use hygienic/`__`-prefixed
locals (same for the guest-side `state` local, which at least fails to compile).

### 6. Vector::len() maps *all* storage errors to 0
`crates/sdk/src/storage/vector.rs:61-63` — `try_get().unwrap_or(0)` swallows
wrong-storage-type and decode failures, not just missing-key. A corrupted/aliased len
slot reads as "empty vector"; the next push overwrites element 0 and clobbers the
foreign len value. Every other typed accessor aborts on type confusion.
**Fix (one-liner):** `self.len.try_get().none_if_missing().unwrap_or(0)` —
`none_if_missing` (`exn/library.rs:151-160`) is the exact existing tool.

### 7. Terminal returns skip Drop → ValueMut writes silently lost
`crates/sdk/src/contract_op.rs:17-31,129-152` — public `contract_return*` fns diverge
via syscall; `ValueMut` flushes on drop, so a live guard's write is discarded **on the
success path**. Worse (verifier): the native mock implements Return as unwind, so
destructors DO run in native tests — the bug is invisible to mock-based testing and
only manifests on real guests. No in-repo caller hits it today (macro wrapper scopes
correctly), but the fns are public and documented for low-level use.
**Fix:** document loudly; or debug-assert no live scratch/ValueMut borrows at return.

### 8. `debugging` feature forks LibraryExn's wire encoding under a fixed selector
`crates/sdk/src/exn/fault.rs:22-23,116-125` — `FaultString` is ~504 bytes with
`debugging`, a ZST (zero encoded bytes) without, while `LibraryExn::SELECTOR` stays
pinned and library.rs:14 claims the wire layout is feature-independent. Verifier
narrowed it: one-directional — a debugging-built caller fails to typed-decode a
non-debugging callee's abort (`try_as::<LibraryExn>()` → None, degrades to generic
fallback); reverse direction works (prefix decode tolerates trailing bytes).
Still: mixed-feature deployments silently lose typed error handling.

### 9. Oversized abort payloads (1025–4096 B) revert the whole invocation untyped
`crates/sdk/src/contract_op.rs:100-110` — no length check against the protocol's
1024-byte return-buffer cap; `throw(BigExn)` becomes a deterministic whole-tx fault
instead of a catchable typed abort. Ironically >4096 B in no-alloc builds gets the
well-handled typed `EncodingException`; under alloc the Vec grows so the 1025+ window
is unbounded. **Fix:** length-check in `_contract_abort_encoded` and fall back to the
EncodingException path.

### Other confirmed footguns (lower severity)
- **Uninitialized `Signed<T>` sentinel** only `debug_assert`-guarded; release guests send handle `u32::MAX` into Invoke (`signer.rs:151-171`).
- **Exception payload prefix-decode ignores trailing bytes** — selector collisions mis-decode silently instead of BadDecode (`call/encode.rs:21-29`); the exceptions example's own test documents this caveat.
- **`#[exn]` on generic exception types**: one selector for all instantiations → cross-type mis-decode (`derive_exn.rs:71-79`).
- **`throws(...)` is client-side only and unenforced** — undeclared exceptions degrade to `Unknown` at runtime; no build-time cross-check of `throw()` sites vs the declared list (three reviewers independently flagged this).
- **`exports =`/`test_setup =` on a `#[contract]` trait are silently ignored** (`contract.rs:39-210`).
- **Forgetting the `entrypoints` feature** silently builds a contract with no entrypoints (`exports.rs:517,532`).
- **Generated `{Client}Error` Display is gated on a user-crate feature literally named `dev`** — undocumented cross-crate convention; missing it is a bare E0277, defining `dev` for other reasons changes codegen (`client.rs:358-375`).
- **Generated names can collide with user types**: `{Event}Payload`, `{Client}Error`, `{Iface}Entrypoints`, `__preimage_{Ident}` (`derive_event.rs:312`).
- **`log_event` silently drops a `None` topic in release** (`event.rs:85-96`); `IntoTopic` hashes or passes-through depending on type — equal bytes, different topics (`event.rs:11-66`).
- **`sdk::StorageKey` silently shadows the protocol union** with its ContractData subtype (`types.rs:17,32`).
- **Recursive `#[contract_type]`s decode attacker calldata with unbounded recursion**; the `recursive_type` example showcases the pattern without a warning.
- **Client-decoded error variants are callee-controlled** — decoding a typed error is not authentication of it (`client.rs:472-491`).
- **Auth-handle decode failures mislabeled** as `CallingError::DecodingSuccess` (`raw_call_error.rs:92-94`).
- **`encode_xdr_into` transmutes `&mut [u8]` → `&mut [MaybeUninit<u8>]`** in a safe pub fn, relying on an unstated xdrs invariant (`serialize.rs:14-26`).
- **NNInt** accepts negative `set()` despite the name, defers underflow to commit (`nnint.rs:29-38`).
- **`ValueMut` flushes unconditionally on drop** (no dirty flag) — read-only `load_mut()` still emits a storage write → spurious lock conflicts/gas (`value.rs:162-166`). (Reentrancy half refuted: reentrance guard prevents the lost-update case.)

---

## Tier 2 — codesize (ranked by expected bytes per contract)

### C1. `try_call_raw_with` monomorphizes the whole call path per (ARGS, RET, X)
`crates/sdk/src/call/calls.rs:29-78`. The type-independent middle (scratch closure,
invoke_frame, resize_auth_handles, fill_return_buf, RawExceptionRef parse/map) is
duplicated per instantiation; measured ~**600–1000 B per used call form**, multiple KB
in call-heavy contracts (swap_driver, wallet forwarders). A non-generic byte-level
core effectively exists as `contract_call` (calls.rs:180-237) but isn't used.
**Fix:** outline a non-generic `#[inline(never)]` core; keep encode (ARGS), decode
(RET), and on_exn dispatch as thin generic shims. Biggest single win.
(Related refuted claim, for the record: the plain/try_ pair does NOT duplicate
encode/decode bodies — `contract_call_decode<T>` is X-independent; only thin glue
differs, and the Infallible/X split is deliberate. Don't unify the two wrappers.)

### C2. Published-crate wasm builds silently ship ~2x panic-fmt bloat
Repo-local builds are protected by `.cargo/config.toml` rustflags (build errors
loudly), but **external authors** depending on the published sdk get 16,040 B vs
5,984 B (blessed) for the hasher example — with `core::fmt`, `panic_fmt`, and SDK
panic strings embedded. ~2x is attributable to panic formatting specifically.
**Fix:** ship the required rustflags/build-std guidance in the sdk crate's docs;
consider a link-time marker or documented size check for out-of-pipeline builds.

### C3. Generated `re_throw` Library arm re-inflates the encoder PR #279 removed
`crates/macros/src/common/client.rs:390` — `Self::Library(e) => ::sdk::throw(e)` goes
through the generic `_contract_abort_encoded::<ContractExn<LibraryExn>>` (~1.5 KB rv32
.text) that `library_abort` was hand-rolled to eliminate. **`cthrow`
(client_library_throw) is already imported three lines above** — this is a one-line
macro fix. Latent (only paid by contracts that re-throw the generated enum, e.g.
`try_x().or_re_throw()`), but it silently undoes the cb1287e optimization when hit.
Also consider re-throwing wire bytes verbatim for declared variants instead of
decode-then-re-encode.

### C4. Scratch checkout prologue + `fresh_fallback` monomorphized per call site
`calls.rs:396-425`, `cell.rs:93-115`. The checkout sequence re-inlines at every
storage/event/call/return site (~30-50 B each), and — worse, per verifier — the
`#[cold] fresh_fallback` is closure-generic: **erc20's binary contains 7 copies, each
with a 4368-byte stack frame** and a duplicate of the site's encode body; wasm-opt
ignores `#[inline(never)]` and re-inlines some. **Fix:** non-generic checkout that
returns the guard, keeping only the user closure inline.

### C5. `fill_return_buf` is `#[inline]` despite being non-generic
`calls.rs:427-471` — hint-inlined into every call-path instantiation, ~100-250 B per
copy. Direct contradiction of the codebase's own pattern (`fetch_calldata_and_auth_tokens`
is `#[inline(never)]` with a load-bearing comment). **Fix (one-liner):** `#[inline(never)]`.

### C6. No-alloc `RawException` is ~4 KiB inline
`exn/raw.rs:12-21,141-159` — every generated `try_*` returns a Result carrying two
4 KiB-buffer variants. Documented deliberate trade, but the primary cost is **guest
stack** (4 KiB per live error vs 24 KiB default rv32 stack) plus the
`clippy::result_large_err` lint leaking into user crates (swap_driver and
groundhog-erc20 both carry crate-level allows already). Consider a truncated-payload
representation (must also address FaultString variants in dev builds).

### C7. Smaller confirmed items
- Panic-with-format-args on syscall paths instead of `syscall_throw`: `hasher.rs:52,68,75` (the `new`/Drop panics are documented/unreachable — verifier), `block.rs:6` (bare unwrap), `signer.rs:96-121,217,363`, `rt.rs:24`, generated `panic!("{f}")` at `client.rs:393`. Free in blessed immediate-abort builds; real cost in C2's out-of-pipeline builds, plus opaque traps vs decodable aborts.
- Blanket `Exn for &T` lets `throw(e)` and `throw(&e)` monomorphize two encoders (`exn/mod.rs:46-51`).
- Each `try_*` method inlines the full selector-dispatch for all `throws()` types into its own monomorphization (`client.rs:284-287`).
- `#[contract_exception]` doesn't derive `ConstXDRSize`, so user exceptions can't use the cheap unchecked encode path (`contract_op.rs:100-110`).
- `Keypath::child<I>` and `log_event<E>` monomorphize framing/syscall plumbing per type when only the encode needs the type parameter.
- `NNInt` recomputes sha256 of its keypath on every op instead of memoizing like `Value<T>` (runtime cost, not size) (`nnint.rs:30-135`).

---

## Tier 3 — pain points / DX

- **README quickstart is wrong**: tells authors to enable a `storage` feature that doesn't exist (README.md:22). Hard error on first build.
- **Missing storage conveniences**: no `get_or`/`get_or_default` on `Value` (3-call incantation today), no `exists()`/`is_set()` on Value/NNInt (init guards must decode the whole value); NNInt is i128-only with no `dec`/`strict_dec` (sign-cast gymnastics in every token contract).
- **API inconsistencies**: `Value::set(&T)` vs `NNInt::set(i128)`; client builders mix `&self` and `self` receivers; `with_commit_locking` obscures that it sets the TryInvoke flag; missing-key behavior differs per storage type/method; sys_exn vs sys_result have same-named fns with contradictory semantics (and the throwing module is doc(hidden)).
- **Three spellings for the same thing** across examples: `#[sdk::storage]`/`#[storage]`/`#[derive(Storage)]`; six overlapping compile-time hash macros with four spellings of the nonce pattern in docs/examples; two TestEnvironment constructors with no guidance.
- **Boilerplate**: every contract carries the same 5-piece Cargo/lib.rs incantation (no_std cfg_attr, declare_mock_contract!, entrypoints/dev features, cdylib, self-dev-dep); free-function contracts repeat identical `#[contract(interface=..., version=...)]` per fn; examples pass `client = ...` equal to the default.
- **Reserved names undocumented**: `new`, `with_*`, and the entire `try_` prefix are reserved on client methods — existing standards with `try_`-named methods can't be expressed.
- **Examples reach into `sdk::__xdrs`** for basic serialization (simple_wallet_old:51) — no public path for the same task.
- **Diagnostics**: `#[contract]` on an impl of a non-`#[contract]` trait cascades into cryptic `__GROUNDHOG_IFACE_*` errors; unsupported items dump the raw syn AST Debug; compound Map/Vector element types need `Node<>` with no guiding diagnostic.
- **Node<> wrapping requirement** for compound element types is undiscoverable without reading examples.

---

## Refuted (worth knowing why)

- **Map::at_disjoint_mut's formatted assert**: immediate-abort lowers it to a bare trap; a typed throw would be *larger*.
- **RawCallError::Invalid conflation**: entrypoint/version mismatch never produces Invalid (it faults the child frame); the existing message correctly names the two real causes.
- **Plain/try_ client pairs duplicating encode/decode**: they don't — decode is X-independent; the split is deliberate and beneficial.
- **invoke_frame/resize_auth_handles X-genericity**: true at source level, codesize-neutral after LLVM inlining (verified via nm on release rlibs).

---

## Suggested quick wins (small diffs, verified payoff)

1. `client.rs:390`: `::sdk::throw(e)` → `cthrow(e)` (already imported). — C3
2. `calls.rs:427`: `#[inline]` → `#[inline(never)]` on `fill_return_buf`. — C5
3. `vector.rs:62`: insert `.none_if_missing()`. — footgun 6
4. README.md:22: delete the nonexistent `storage` feature instruction.
5. Document `#[slot(N)]` + reorder hazard in `crates/sdk/doc/macro-storage.md`.
6. Pin `version=` on `standard-interfaces/groundhog-erc20` (and audit the other standard interfaces).
7. Rename generated locals (`entrypoint`, `state`) to hygienic `__groundhog_*` names.
