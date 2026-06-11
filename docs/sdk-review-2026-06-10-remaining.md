# SDK review — remaining findings (2026-06-10)

Unaddressed findings from the multi-agent SDK review (see
`sdk-review-2026-06-10.md` for the full report including verifier notes).

**Addressed in the 2026-06-10 fix pass** (not listed below):
the `[T; N]` `ReceiveAuthHandles` no-op, the `InvokeError` Display
recursion, `static_key!`/`sha256_static!` hashing the quoted token text
(both now hash the literal's value, matching `hash_to_array!` — note this
**changes every storage key / static hash** derived by those macros),
`Signed::consume` + auth-trait rustdoc, the `Backtrace` unsoundness
(now crate-private + unsafe capture), `consume()` leftover-child-handle
check, `build_auth_token_node` now takes the full `AuthTokenPreimage`
(breaking signature change), README quickstart, `try_commit_transaction`
rustdoc, and erc20-test abort checks (re-enabled by default and repaired)
+ the hardcoded mint-event amount.

Additionally, the ERC20 error contract was standardized: the trait in
`standard-interfaces/groundhog-erc20` now documents exactly which
`ERC20Error` each method throws, all four implementations (`erc20`,
`erc20_low_level`, `erc20_no_storage`, `itc_erc20`) throw
`ERC20Error::TooLarge` on balance/allowance overdraft instead of leaking
storage-layer `NonNegativeIntUnderflow`, and erc20-test pins the exact
error in its abort checks. (This partially addresses the
"groundhog-erc20-if trait omits authorization and error contracts"
finding below — the trait-docs half is done; the `StdERC20Event`
i128::MAX conversion panic is not.)

Also fixed: the contract-macro panic paths now emit spanned compile
errors instead of "proc macro panicked" — invalid `version = "..."`
strings (in `#[exn]`, `#[contract_exception]`, and `#[contract]`),
non-identifier `exports`/`interface` strings, and `_`/pattern method
arguments are all rejected with real diagnostics, covered by six new
trybuild compile-fail tests; `contract-util` gained
`semver::try_major_version_from_string` to support this.

**New issue found during the fix pass:** `cargo test -p erc20 -p
erc20_low_level` (one cargo invocation, both packages) fails at HEAD:
`example-contracts/erc20`'s dev-dependency on
`erc20-native-test/metadata` feature-unifies into the other erc20
example tests, which then exercise the metadata interface against
contracts that don't implement it. Each package passes in isolation,
which is how CI runs them — but it's another instance of the
feature-unification fragility called out below.

Two more silent-failure instances in `scripts/dev.sh` observed during
verification: `./scripts/dev.sh test` dispatches to a nonexistent
`test_all` function and exits 0, and `dev.sh test-examples` run from any
directory other than the repo root iterates zero packages (relative
`example-contracts` path) and also exits 0.

## medium severity (22 remaining)

### [sdk-core/footgun] ReturnBuf/AbortBuf hard-capped at 500 bytes while the protocol allows 1024-byte returns

**File:** `crates/sdk/src/call/contract_result.rs:15`

`pub type Buf = heapless::Vec<u8, 500>; pub type ReturnBuf = Buf;` while the SDK's own test comment in calls.rs:260 states 'A child can return up to the protocol's 1024-byte cap. When it returns 600 bytes, both Invoke and GetReturnedBuffer report 600 even though only CAPACITY bytes fit'. So calling any (protocol-valid) contract whose return encodes to 501-1024 bytes always fails at runtime, and with the misleading error `RawCallError::UnparseableReturn` / `CallingError::DecodingSuccess` rather than anything indicating a size limit. The same magic 500 appears independently as `CALLDATA_MAX` (contract_op.rs:17), `EncodeBuf` (call/encode.rs:31), `RawException` payload (exn/raw.rs:12), and `ENCODE_MAX` (internal/serialize.rs:19) — none documented to contract authors.

### [sdk-core/footgun] FaultString wire format silently changes with the `debugging` feature, breaking cross-build exception decoding

**File:** `crates/sdk/src/exn/fault.rs:6-10,21,131`

With `debugging`, `FaultString` serializes as `Option<&str>` (`struct FaultString(Option<heapless::String<500>>)`, write_xdr at line 24-29); without it, `pub struct FaultString;` serializes to zero bytes (line 133-138). `LibraryExn` (exn/library.rs) embeds `FaultString` in most variants and is a cross-contract exception with a fixed selector derived only from version "1.0.0" + "LibraryExn" — the selector does not change when the payload layout changes. Two contracts (or a contract and an off-chain decoder) built with different `debugging` settings will mis-decode each other's library exceptions with no versioning signal. Same pattern in `Panic` (panic.rs) and the `#[cfg(feature = "storage")] Storage(StorageError) = 8` variant of LibraryExn, whose existence is also feature-dependent.

### [sdk-core/footgun] contract_exit silently ignores its exit code and is identical to contract_revert

**File:** `crates/sdk/src/contract_op.rs:41-47`

`pub fn contract_exit(_code: i32) -> ! { sys::raw_Exit() }` and `pub fn contract_revert() -> ! { sys::raw_Exit() }` are byte-identical; the `code` parameter is discarded. The names imply opposite semantics (clean exit vs. revert), but both abort the frame the same way, and a caller passing `contract_exit(0)` expecting a successful return gets a revert. Neither has rustdoc, and a repo-wide grep shows neither is used anywhere (nor are `contract_return_empty`, `contract_result_should_commit`, `contract_result_should_revert`).

### [sdk-core/footgun] Blanket `Hash for T: WriteXDR` panics via unwrap() on values larger than ~500 bytes

**File:** `crates/sdk/src/hash.rs:14-23`

`impl<T: WriteXDR> Hash for T { fn hash(&self) -> Hash256 { let buf = crate::internal::encode_xdr(self).unwrap(); ... } }` — `internal::encode_xdr` writes into a fixed `MUBuf<ENCODE_MAX>` (ENCODE_MAX = max(kMaxEventLength, 500), internal/serialize.rs:19-24), so hashing any value whose XDR encoding exceeds that buffer panics with a bare unwrap. The trait applies to *every* WriteXDR type, the limit is invisible in the API, and neither the trait nor the impl documents the panic.

### [sdk-core/footgun] try_get_calldata_with_auth_tokens trusts host-reported lengths with no capacity check, unlike the return-path

**File:** `crates/sdk/src/contract_op.rs:205-209`

`let written = unsafe { written.assume_init() }; unsafe { calldata.set_len(written as usize) };` — the calldata path applies the host-reported length directly to a `MUBuf<500>`. The sibling return path was hardened precisely because the host 'reports the *full* source length, which can exceed capacity' (see `checked_returned_len`, call/calls.rs:205-236, added with tests). If calldata exceeds 500 bytes, `set_len` exceeds SIZE and the next `as_ref()` panics on a confusing slice-index error instead of producing a clean decode fault; there is also no `written <= SIZE` proof backing the subsequent `assume_init_ref`. Same for the auth-token buffer at line 209.

### [sdk-core/redundancy] static_mu_buf!/static_vec! are #[macro_export]ed, completely unused, and expand to `static mut` that is unsound off-guest

**File:** `crates/sdk/src/internal/static_buf.rs:4-114`

A repo-wide grep finds zero invocations of either macro. Because they are `#[macro_export]`, they are public API at the crate root (`sdk::static_mu_buf!`), yet they expand to `static mut BUF: RefCell<...>` with `#[allow(static_mut_refs)]` and the comment 'SAFETY: We're single-threaded' — if a contract author invokes one in host-side test code (cargo test runs threads in parallel), two threads borrow the same `static mut` RefCell: a data race / UB the RefCell cannot prevent. Dead, exported, and unsound when used as exported — should be deleted or made guest-gated/doc(hidden) internals.

### [sdk-storage/footgun] NNInt is named 'Non-negative integer' but set() stores negatives and inc() defers underflow to commit time

**File:** `crates/sdk/src/storage/nnint.rs:8-57`

The type's entire rustdoc is `/// Non-negative integer`, yet `set(-5)` succeeds and `get()` then returns -5 (proven by tests/nnints.rs `negative_set_succeeds` and `set_negative_then_increment_succeeds`: "We can explicitly set a negative value, and the transaction succeeds"). Meanwhile `inc()` (doc: "Increment by an amount (without querying the current value)") can drive the committed value negative without any call-site error — the transaction only reverts at commit (tests/internal_nnints.rs `negative_increment_reverts`: "The frame succeeds but the commit must fail"). The nuanced rules (negative set is allowed and disables the non-negativity check; inc after a non-negative set throws immediately; otherwise underflow is a deferred commit failure; concurrency constraints of set vs add) are documented only on the free functions in src/sys_exn/nnint.rs, not on the NNInt methods a contract author actually calls. NNInt is the obvious balance/counter primitive, so an author relying on the type name to guarantee non-negativity at call time, or expecting set(-x) to throw, is badly misled.

### [sdk-storage/footgun] Keypath::from_key does not produce a keypath that maps to the given key, and is unused

**File:** `crates/sdk/src/storage/keypath.rs:27-31`

`pub fn from_key(key: &StorageKey) -> Self` pushes the XDR-encoded StorageKey as a path component, so `Keypath::from_key(k).generate_key()` returns `sha256(encode(k))` — a completely different key from `k`. The name strongly suggests round-tripping a known StorageKey (e.g., one from `static_key!`) into the Keypath world; anyone using it to address an existing raw-syscall location will silently read/write a different slot. There is no rustdoc to dispel this, and a workspace-wide grep shows zero callers (not even tests), so it is also dead public API.

### [sdk-storage/missing-docs] Vector push/pop take an exclusive lock on len; concurrent transactions conflict — undocumented

**File:** `crates/sdk/src/storage/vector.rs:87-109`

`push_empty_slot`, `push_with`, and `pop` all call `self.len.set_exclusive(&new_len)`, meaning any two transactions that push to (or pop from) the same vector in the same block conflict and one fails at commit. The SDK's own test relies on this: crates/sdk/tests/vector-tests/tests/test.rs:60-61 — "// vector push acquires exclusive lock on the vector size" followed by `assert_commit_failed_nondeterministically()`. None of the rustdoc on push_with/push_empty_slot/pop mentions this; an author building a shared log/queue with Vector will discover the serialization bottleneck (and nondeterministic commit failures) only in production. Given the runtime's whole design is around concurrency-friendly storage deltas, this is the single most important fact about Vector's write path.

### [sdk-storage/footgun] Bytestring::len()/is_empty() abort on a never-set key, asymmetric with Vector::is_empty

**File:** `crates/sdk/src/storage/bytestring.rs:21-27`

`Bytestring::len()` calls `bytestring_get_len(&self.key)`, which (per src/sys_exn/bytestring.rs) "Throws a syscall exception if nothing is there". So `is_empty()` on a fresh Bytestring aborts the whole transaction instead of returning true, while `Vector::is_empty()` (vector.rs:62, via `len.try_get().unwrap_or(0)`) returns true for never-set storage. The Bytestring type has zero rustdoc on any method — nothing warns that `len`/`is_empty`/`get`/`get_sized` abort on a missing key while `try_get` returns None, and there is no `exists()` method even though `bytestring_exists` exists at the syscall layer. The look-alike methods across the two types behave oppositely on empty storage.

### [sdk-storage/missing-docs] Keypath public API is entirely undocumented, including the collision-prone flattening scheme

**File:** `crates/sdk/src/storage/keypath.rs:33-63`

Keypath and all its methods (root, flattened, generate_key, push_item, push_index, seq_begin/seq_next/seq_end, set_exclusive) have no rustdoc, yet it is the public escape hatch for manual storage layout (tests do `Keypath::root().into()` routinely). Two things a user must know are unstated: (1) `flattened()` concatenates component encodings with no framing or type tags, and `KeypathComponent::Index(i)` flattens to exactly the same bytes as `push_item(&(i as u64))` (both are `encode(u64)`); since `derive(Storage)` assigns field i the component Index(i) (crates/macros/src/storage.rs:120, seq_begin/seq_next) and Vector puts its len at Index(0) and values at Index(1), a user who manually does `root.push_item(&0u64)` silently collides with field 0 of a struct or a Vector's len slot at the same root. (2) `push_item` only guarantees injective keys when each component's XDR encoding is prefix-free in sequence; pushing fixed-size opaques of varying lengths can collide. Also `seq_next`/`seq_end` panic when the last component is not an index — undocumented, and exercised by the derive on hand-rolled From<Keypath> impls.

### [macros/footgun] #[contract_type] enums get identity Mintable while structs recursively mint fields — auth-token children in enum payloads are never minted

**File:** `crates/macros/src/contract_type.rs:34-47`

For structs, `generate_preimage_and_mintable` builds a `Preimage` whose every field is `<T as MintableType>::Preimage` and `mint_into_parent` recurses field-by-field (lines 306-465), so a field like `Signed<T>` (whose Preimage is `AuthTokenPreimage<...>`, see crates/sdk/src/signer.rs:185-187) is properly minted into child auth-token nodes. For enums the macro instead emits `impl MintableType for #ident { type Preimage = #ident; }` and an identity `mint_into_parent` (lines 34-47). An enum variant containing a mintable type therefore never recurses: its preimage *is* the final type and no children are produced. Contrast with `CollectAuthHandles`/`ReceiveAuthHandles`, which DO recurse into enum variant fields (lines 246-297, 467-518) — the asymmetry is invisible to the user and undocumented.

### [native-exec/footgun] Panicking Drop impls turn a failing test into a SIGABRT that hides the original failure

**File:** `crates/native-exec/src/mock_transaction_handler.rs:287-295, 401-413, 463-471`

`MockInvocationFrame::drop` panics if state == Started ("Should be impossible to drop an invocation frame while it is running"), and `MockTransactionHandler::drop` calls `abort_transaction()` whose `assert!(self.root_frame_closed.get(), "cannot abort: root frame is still open")` panics if the root frame was opened but never closed. If a test panics (e.g. an ordinary `assert!` failure) while a root frame has been opened/built but not executed to Close, these Drops run during unwinding, double-panic, and abort the whole test process — the user sees SIGABRT instead of their assertion message. `DBWrapper::drop` and `RuntimeContextWrapper::drop` (test_environment.rs:163-171, 191-199) also panic on FFI error and have the same unwind hazard.

### [native-exec/redundancy] Event decode/topic-match logic duplicated between filter_events and assert_emitted, and already drifted

**File:** `crates/native-exec/src/frame_result.rs:241-261, 306-338`

Both methods reimplement the same sequence: compute `topics_len = 32 * event.num_topics`, bounds-check, slice payload, `cxs::decode::<E>`, `topic0_matches`. They have drifted on the malformed-event case: `filter_events` silently skips an event whose `topics_len > serialized_event.len()` (`return None;`, line 250-252) while `assert_emitted` panics on the identical condition (`assert!(topics_len <= event.serialized_event.len(), ...)`, lines 319-324). So the same corrupt event makes one API quietly return an empty vec and the other abort the test. Also, in `topic0_matches` the `assert!(raw.num_topics == 0)` in the else branch (line 32) is unreachable: the function already returned false when `decoded.num_topics() != raw.num_topics`.

### [spec-tooling/footgun] BytesM<MAX> silently discards its length bound in generated specs (asymmetric with VecM)

**File:** `crates/contract-spec/src/lib.rs:176`

`impl<const MAX: u32> ContractTypeRefSpec for xdr_common::BytesM<MAX> { const INFO: ContractTypeInfo<'static> = <&[u8] as ContractTypeRefSpec>::INFO; }` maps every BytesM<MAX> to `Vector(U8, max_len: u32::MAX)`, throwing away MAX. Directly below, `VecM<T, N>` (lib.rs:228-237) correctly preserves its bound as `max_len: N`. The spec model can express the bound (ContractVectorTypeView.max_len exists), so a contract taking `BytesM<256>` publishes a spec claiming it accepts unbounded bytes; clients/codegen built from the spec will produce inputs the contract rejects at runtime. Same silent bound loss for `StringM<MAX>` and `heapless::String<MAX>` (lib.rs:168-174), though there the spec String type has no bound field to preserve. This is exactly the kind of lossy spec/code drift a contract author would never notice.

### [spec-tooling/footgun] spec-tool panics (instead of returning SpecToolError) on non-UTF-8 spec strings from malformed wasm

**File:** `crates/spec-tool/src/lib.rs:19`

`fn spec_str(s: &ContractSpecString) -> &str { std::str::from_utf8(s).expect("contract spec strings must be valid UTF-8") }`. ContractSpecString is `StringM`, which is `pub struct StringM<...>(pub Vec<u8>)` (xdr-common/src/stringm.rs:70) — XDR decode performs no UTF-8 validation. So `parse_raw_fragments` happily decodes a wasm custom section containing non-UTF-8 names, and `canonicalize_spec` then panics in `validate_interface_key`/error-formatting paths instead of returning `SpecToolError::InvalidRawFragment`, which exists precisely for this case. Any tool embedding `rewrite_wasm` on third-party or corrupted wasm gets a process abort rather than an error.

### [spec-tooling/missing-docs] Primary entry points rewrite_wasm / rewrite_wasm_with_spec / RewrittenWasm have zero rustdoc

**File:** `crates/spec-tool/src/rewrite.rs:12`

`pub fn rewrite_wasm`, `pub fn rewrite_wasm_with_spec`, and `pub struct RewrittenWasm` carry no documentation at all, despite non-obvious semantics a caller must know: (a) both `groundhogspecrawv0` and `groundhogspecv0` custom sections are stripped and a single final section re-emitted; (b) if no raw fragments exist, an existing final section is decoded and re-encoded instead; (c) typedef-only raw fragments yield `spec: None` with ALL spec sections (including a pre-existing final one) silently removed; (d) the call errors if encoded raw-fragment bytes appear in any data segment ("raw spec bytes leaked into linear memory") — an invariant callers can't guess. Similarly `parse_raw_fragments` (raw.rs:10) doesn't document the concatenated-XDR stream format, and `describe::write_output` (describe.rs:71) doesn't document its XDR output as `encode(exception) ++ encode(Vec<ContractTypeDef>)`, a non-self-describing concatenation a consumer must know how to split.

### [small-crates/footgun] TypedEvent::new decodes without verifying the event-type discriminant (topic 0)

**File:** `crates/event/src/emitted_event.rs:69-78`

`TypedEvent::<E>::new(raw)` takes any `TxEmittedEvent`, decodes the payload bytes as `E::Payload`, and panics on decode failure — but never checks that topic 0 (documented in `crates/event/src/lib.rs:30-32` as "conventionally the event-type discriminant") matches `E`'s signature. XDR payloads with compatible shapes (e.g. two events that are both `(address, address, amount)`) will decode cleanly as the wrong type, producing a silently mistyped view. The workspace itself proves the check matters: `crates/native-exec/src/frame_result.rs:12` defines `fn topic0_matches<E: Event>(...)` as a separate guard, and the ERC20 interface implements `TryFrom<EmittedEvent>` with explicit signature checks. But `TypedEvent::new` is re-exported from `sdk::event` (crates/sdk/src/event.rs:6) as a public constructor with no such guard and no doc warning beyond "Panics on decode failure".

### [consumer-experience/redundancy] Five competing spellings for deterministic test nonces/addresses across README and examples

**File:** `README.md:32 (vs example-contracts/counter/tests/test.rs:4, erc1271_caller/tests/lifecycle.rs:11-12)`

For the same task ('make a deterministic test id'), the corpus a new author reads uses: `SHA256Hash::new(hash_to_array!("..."))` (README, sdk/src/mock/contract_test.rs:112), `sdk::sha256_static!("...")` (counter, versioned_caller, bare_impl_caller tests), `hash_ident! { static FOO: SHA256Hash; }` (erc20-test/src/contract_test.rs:16-17, swap_driver tests), `ContractAddress::named("test")` (everywhere for caller addresses), and raw `[0xAA; 32].into()` (erc1271_caller/tests/lifecycle.rs). The first two are not even value-equivalent (see the quote-hashing finding). A new user cannot tell which is canonical; the SDK exports all of them (sdk/src/lib.rs:166-168, util.rs:12) with no rustdoc on any.

### [consumer-experience/missing-docs] groundhog-erc20-if trait omits authorization and error contracts; std-event conversion panics above i128::MAX

**File:** `standard-interfaces/groundhog-erc20/src/lib.rs:23-65`

The published ERC20 standard interface documents nothing a wallet/token integrator needs: `initialize` has no doc at all (it throws `AlreadyInitialized`; who becomes administrator?); `mint`/`burn` don't say they are owner-restricted (`MustBeContractOwner`) or which `ERC20Error` variants each method can throw; `adjust_allowance` deviates from ERC-20 `approve` semantics (signed delta, not absolute) but that is documented only on the ApprovalEvent type (events.rs:135-138), not the trait method clients call. Also, `Amount` is `u128`, but `From<&TransferEvent> for stdevt::TransferEvent` does `TryInto::<i128>::try_into(val.amount).unwrap()` (events.rs:90) — converting an event for a supply above i128::MAX panics, an undocumented invariant, despite `amount_to_diff` in error.rs:20-22 showing the throwing alternative.

### [consumer-experience/weird] Examples disagree on #[contract] storage/Default boilerplate for stateless contracts

**File:** `example-contracts/erc1271_caller/src/lib.rs:10-13 (vs bare_impl_caller/src/lib.rs:7-10, versioned_caller/src/lib.rs:7-9)`

Three stateless caller examples use three different spellings: erc1271_caller writes `#[derive(Default)] pub struct Caller;` + `#[contract(storage = false)]`; bare_impl_caller writes `#[derive(Default)]` + bare `#[contract]`; versioned_caller writes neither Default nor storage flag. Since all their methods are associated functions (no `self`), the storage flag and Default derive are dead weight in all three — the macro only consults `config.storage()`/`Default` for `self` methods (crates/macros/src/common/exports.rs:291-321, with the TODO at line 316 noting the Default requirement is not even asserted). A user copying erc1271_caller will cargo-cult `storage = false` + Default; a user copying versioned_caller learns they're unnecessary. Nothing documents which is right, and `storage = false` with a `&mut self` method silently constructs a throwaway Default object whose mutations are discarded — a trap only a macro-source reader would discover.

## low severity (58 remaining)

### [sdk-core/redundancy] src/internal/track.rs is dead code and a drifted duplicate of call_tracking_contexts.rs

**File:** `crates/sdk/src/internal/track.rs`

internal/mod.rs declares `mod mu_buf; mod serialize; mod static_buf;` and (feature-gated) `mod call_tracking_contexts;` — there is no `mod track;` anywhere, so track.rs is not compiled. Its contents (`CallMethod`, `HandlingMethod`, both `with_call_tracking!` arms) are near-verbatim copies of internal/call_tracking_contexts.rs:5-67 and diagnostics/mod.rs:27-42. The duplicate `with_call_tracking!` definition would actually be a `#[macro_export]` name collision if the file were ever wired back in.

### [sdk-core/missing-docs] Public syscall wrappers and InvokeConfig lack rustdoc on throwing behavior, limits, and default semantics

**File:** `crates/sdk/src/deploy.rs:8-51`

Whole public modules expose undocumented functions whose failure mode is a thrown `LibraryExn` (a non-returning abort): `get_deploy_address`, `get_deploy_address_for_other`, `deploy_contract` (deploy.rs), `get_caller`/`get_current_address`/`get_source_transaction_hash` (contract_op.rs:139-164), `log` (log.rs:5), `sha256` (crypto.rs:9). None state that they throw rather than return errors, and none document size limits (e.g. `log_event`'s 500-byte data cap via `encode_xdr`). `InvokeConfig` (invoke_config.rs:4-17) documents neither field: `gas_limit: u64::MAX` as the default 'forward everything' sentinel and the units of `gas_limit` are exactly what a contract author needs to know before making cross-contract calls.

### [sdk-core/redundancy] src/allocator_panic.rs is dead, and would define a conflicting #[global_allocator] if ever re-included

**File:** `crates/sdk/src/allocator_panic.rs`

lib.rs declares no `mod allocator_panic;` and nothing references the file; its own header says '(This was briefly useful when having difficulty excluding liballoc from builds.)'. It still contains `#[global_allocator] static GLOBAL_ALLOCATOR: Allocator = Allocator;`, which would collide with the allocators force-linked from groundhog-wasm-runtime / groundhog-rv32-runtime (lib.rs:50-54) if anyone wired it back in. Should be deleted.

### [sdk-core/weird] get_block_height and gas_remaining bypass the SDK's syscall_throw convention with bare unwrap/panic

**File:** `crates/sdk/src/block.rs:6`

`raw_GetBlockHeight(&mut block).unwrap();` and stats.rs:11 `unwrap_or_else(|e| panic!("GetTransactionStatistics failed: {:?}", e))` are the only two syscall wrappers in the crate that panic (fault) instead of `syscall_throw(...)` (typed `LibraryExn::Syscall` exception) like contract_op.rs, crypto.rs, deploy.rs, log.rs and event.rs do. Callers get a fault with no selector instead of a catchable library exception, and in non-debugging builds the panic message is lost entirely — a surprising asymmetry for such commonly used getters.

### [sdk-storage/footgun] Map::at_disjoint_mut uses K::PartialEq as a proxy for storage-key disjointness

**File:** `crates/sdk/src/storage/map.rs:67-90`

The safe `at_disjoint_mut` guards against aliasing with `assert!(keys[i] != keys[j], ...)` using `K: PartialEq`, but actual slot identity is `sha256(XDR-encode(key))` (make_entry_keypath → push_item → generate_key). If a user key type's PartialEq is finer than its XDR encoding (e.g., a hand-written PartialEq that compares a field skipped during encoding, or any impl where `a != b` but `encode(a) == encode(b)`), the safe API hands back aliased handles to the same storage slot — exactly the "may silently corrupt data" hazard that the doc on the `unsafe at_disjoint_unchecked_mut` (lines 92-112) says only the unsafe path can produce. The check should compare encoded keys or generated StorageKeys (which are computed right after, on line 85) instead of relying on an unstated `PartialEq`-agrees-with-encoding invariant.

### [sdk-storage/redundancy] NNInt::set/inc/strict_inc duplicate sys_exn::nnint free functions nearly line-for-line

**File:** `crates/sdk/src/storage/nnint.rs:41-71`

`NNInt::inc` (get_values → checked_add → nnint_overflow → set_add) and `NNInt::strict_inc` (try_get_values_force.unwrap_or((0,0)) → two checked_adds → total<0 underflow → set_add) are copies of `non_negative_int_inc` (src/sys_exn/nnint.rs:94-101) and `non_negative_int_strict_inc` (109-121), differing only in passing `self.flags()` instead of `NONE`; same for set/set_exclusive. The free functions carry all the semantic documentation but have no non-test callers in the workspace (grep confirms only tests use them), while the NNInt copies carry one-line docs. Two public spellings of the same operations, with the logic already maintained in two places — a flags parameter on the free functions (or NNInt delegating to them) would collapse this and keep the docs attached to the code people use.

### [sdk-storage/redundancy] Value::load_value bypasses its own KeyCache and re-hashes on every read; NNInt has no key cache at all

**File:** `crates/sdk/src/storage/value.rs:60`

Value stores `key: KeyCache` precisely to memoize the sha256 of the keypath, and `set()` uses it (`bytestring_set(&self.key, ...)` deref-coerces to the cached StorageKey). But the read path does `let key = &self.key.keypath().generate_key();`, recomputing the hash on every `get`/`try_get`/`load_mut` and never populating the cache — `self.key.get()` was sitting right there. Similarly, NNInt (nnint.rs:23-83) stores a bare `Keypath` and calls `self.keypath.generate_key()` in every method, unlike Bytestring/RCQ/Value which all use KeyCache. Both are metered sha256 syscost paid repeatedly for no reason, and the inconsistency suggests the cache discipline has drifted.

### [sdk-storage/redundancy] Vector embeds a Map<u32, S> only for its keypath, then reimplements Map's entry-key derivation

**File:** `crates/sdk/src/storage/vector.rs:13,142-146`

`Vector<S>` has field `values: Map<u32, S>`, but the only use is `self.values.keypath().clone()` inside `make_elem_keypath`, which then does `kp.push_item(&index)` — a copy of the private `Map::make_entry_keypath` (map.rs:114-121). Vector never calls Map::at/at_mut; if Map's key derivation ever changes (e.g., domain separation added), Vector's parallel copy silently diverges and existing vectors' element keys stop matching. Either store the values Keypath directly or route element access through the Map.

### [sdk-storage/redundancy] Dead helper storage_load_error; duplicated test test_map vs test_map_at

**File:** `crates/sdk/src/storage/error.rs:52-54`

`pub fn storage_load_error(_r: ReadError) -> StorageError` has zero callers in the workspace (rcq.rs uses throw_load_exn/throw_store_exn; value.rs maps to StorageError::Serialization inline). Separately, crates/sdk/tests/storage.rs lines 30-43 (`test_map`) and lines 150-163 (`test_map_at`) are byte-identical test bodies (same slot 3, strict_inc(7), same asserts), and `test_map_at_nested` (165-178) tests nothing nested — it is the same single-level `Map<u32, NNInt>` with a different slot number, so the name promises coverage (nested maps) that does not exist.

### [sdk-storage/weird] seq_end panic message names a nonexistent method 'leave_seq()'

**File:** `crates/sdk/src/storage/keypath.rs:79-85`

`seq_end` panics with `"Keypath: leave_seq() called when last path component is not an index"` — there is no `leave_seq` anywhere in the workspace; the method was evidently renamed and the message not updated. A contract author hitting this abort will grep for a function that doesn't exist. Related small oddities in the same area: crates/macros/src/storage.rs:122 has the commented-out `// keypath.seq_end(); // unneeded`, and crates/sdk/tests/internal_nnints.rs:98 carries `// TODO: test with \`delete\`` — the nnint set/add interaction with delete is indeed untested.

### [sdk-storage/missing-docs] RCQ consume_sized/peek_sized omit the wrong-size exception that their try_ twins document

**File:** `crates/sdk/src/storage/rcq.rs:52-79`

`try_consume_sized` and `try_peek_sized` both document "If an entry is present but does not have the expected size, an exception is thrown", but the non-try `consume_sized` ("Consume a fixed-size queue entry.") and `peek_sized` ("Retrieve a fixed-size queue entry without consuming it.") say nothing, even though they throw both on empty queue and on size mismatch (LibraryExn::BytestringWrongSize per the underlying sized-get path). The asymmetry actively suggests the non-try versions don't have the size hazard. The `Option` in the try_ variants only covers absence, not size mismatch — worth stating on all four since a reader naturally assumes `try_*` returns None on any failure.

### [sdk-auth/footgun] Public Default for Signed<T> hands out a sentinel handle guarded only by debug_assert

**File:** `crates/sdk/src/signer.rs:124`

`impl<T> Default for Signed<T>` returns `handle: u32::MAX` and the doc comment says 'Used internally by Mintable ... not for direct use', but it is a public trait impl that derive(Default) on a user struct will happily pull in, and tests themselves use `Signed::default()` (tests/signer_lifecycle.rs:157). The only guards are `debug_assert_ne!(self.handle, u32::MAX, ...)` in collect_auth_handles (signer.rs:142) and consume (signer.rs:283) — compiled out in release, where the bogus u32::MAX handle flows into raw_Invoke/ConsumeAuthToken and surfaces as an opaque Invalid/abort with no hint that the token was never initialized. If 'not for direct use' is the contract, it should not be reachable via `Default`/`derive(Default)`, or the check should be a real assert.

### [sdk-auth/weird] Signed<T> spec says U32 but its wire encoding is zero bytes

**File:** `crates/sdk/src/signer.rs:175`

`ContractTypeRefSpec for Signed<T>` declares `ty: ContractTypeView::U32`, but `WriteXDR for Signed<T>` writes nothing (line 20-25) and `ReadXDR` reads nothing (returns the sentinel); the actual u32 token id travels out-of-band in the auth-token-id list. Any external tooling that generates calldata encoders from the published contract spec will serialize a 4-byte u32 into calldata where the contract expects zero bytes, shifting every subsequent argument. If U32 is the intended external representation (token id in the tx), that mapping deserves a comment, because within this crate the spec and codec visibly disagree.

### [sdk-auth/weird] AssumeTokens supports tuples only up to arity 8 while collect/receive go to 20

**File:** `crates/sdk/src/signer.rs:104`

`impl_assume_tokens!` is instantiated for 0..=8 elements (signer.rs:104-120), but `impl_receive_tuple!` and `impl_collect_tuple!` both cover arities 1..=20 (receive_auth_handles.rs:207, collect_auth_handles.rs:189). A caller destructuring 9+ opaque tokens gets a confusing 'trait not implemented' error in an API family that otherwise handles 20. Also note `AssumeTokens::assume_tokens` panics on length mismatch (assert_eq) rather than returning Result — consistent with tests but worth a doc line since the rest of the receive surface returns AuthTokenError.

### [sdk-auth/redundancy] Stale commented-out receive_auth_handles_from_slice call with misleading live comment in entrypoint codegen

**File:** `crates/macros/src/common/exports.rs:380`

The generated entrypoint body contains `// Fill in Signer handles from the auth token ID list` immediately followed by the dead line `// ::sdk::receive_auth_handles_from_slice(&mut __args, auth_token_ids.as_slice());`. The fill actually happens inside `contract_call_decode` (crates/sdk/src/call/encode.rs:16), which is already called three lines earlier. Anyone auditing the auth path through the macro output reads a comment claiming a step happens at a point where the code is commented out — it took cross-referencing encode.rs to confirm handles are received at all. The pair of lines should be deleted.

### [sdk-auth/redundancy] mint_auth_token / mint_opaque_auth_token duplicate the syscall block; dead timed() helper in contract_test.rs

**File:** `crates/sdk/src/signer.rs:218`

Both mint functions repeat the identical MaybeUninit + uninit_MintAuthToken + syscall_throw + assume_init + Signed::from_handle sequence (signer.rs:236-245 vs 266-275); mint_auth_token could be `mint_opaque_auth_token(expiry, &buf).assume()`-style over a shared private fn, so the two copies can't drift. Separately, `#[allow(unused)] fn timed` in crates/sdk/src/mock/contract_test.rs:275 is dead code (nothing references it; the gas println at line 222 it presumably fed is also commented out) and contains a self-contradictory unreachable panic path ('consumed negative gas' computed with unsigned subtraction after the guard).

### [macros/footgun] `#[contract(storage = false)]` silently discards all state mutations from `&mut self` methods

**File:** `crates/macros/src/common/exports.rs:316-321`

When `config.storage()` is false, the generated entrypoint creates a throwaway value: `let #mutability state_default = <#self_ty as Default>::default(); let #mutability state ... = &#mutability state_default;`, invokes the method, and drops it. A `&mut self` method compiles and runs but every write to `self` vanishes — no error, no warning. The code itself flags the gap (`// TODO: if !config.storage(), assert presence of Default impl`) but nothing rejects `&mut self` methods or documents that `storage = false` means 'stateless, Default-constructed each call'. Nothing in the `#[contract]` macro rustdoc (there is none) warns about this.

### [macros/footgun] Bare-impl #[contract] silently skips non-`pub` methods (including `pub(crate)`)

**File:** `crates/macros/src/common/contract.rs:226-238`

`ContractSrc for syn::ItemImpl::methods()` filters with `is_visible(&impl_fn.vis) || is_trait_impl`, and `is_visible` is `matches!(vis, syn::Visibility::Public(_))` (line 360-362). On a bare `impl` block, a method declared `pub(crate)` or private is silently excluded from exports, the client, the spec, and the mock registry — no warning, no error. Trait impls include everything, so moving a method between a trait impl and a bare impl changes whether it is exported based only on visibility. A contract author who writes `pub(crate) fn admin_reset(...)` thinking 'crate-visible is fine, the macro exports it anyway' ships a contract missing that entrypoint. The rule is undocumented.

### [macros/redundancy] Dead alloc-marker machinery in contract_exception.rs emits only comments (which vanish from token streams)

**File:** `crates/macros/src/contract_exception.rs:96-146`

`alloc_enabled_impl_tokens` returns `quote! { //impl ::sdk::internal::ContractAbiAllocEnabled ... }` in both branches — comments inside `quote!` produce no tokens, so the function always returns an empty TokenStream. That makes `struct_alloc_enabled_impl_tokens`, `enum_alloc_enabled_impl_tokens`, `field_types`, and `enum_variant_field_types` (plus the Result plumbing and the `match ... { Err(err) => return err.into_compile_error() }` at call sites, lines 47-50 and 76-79) entirely dead code that still walks every field of every exception type on each expansion. Either the `ContractAbiAllocEnabled` feature should be revived or this whole chain deleted.

### [macros/redundancy] Copy-pasted auth-handle walkers, a duplicated entrypoint_export_name, and an always-Some Option

**File:** `crates/macros/src/contract_type.rs:174-297,467-518`

(1) `expand_collect_auth_handles_struct`/`expand_receive_auth_handles_struct` and `expand_collect_auth_handles_enum`/`expand_receive_auth_handles_enum` are four near-identical field walkers differing only in the method name and one argument — a single parameterized generator would prevent the drift this pattern invites. (2) `common/entrypoint.rs:17-19` redefines `entrypoint_export_name` byte-for-byte identical to `groundhog_contract_util::entrypoint_export_name` (crates/contract-util/src/lib.rs:119-121), which the crate already depends on. (3) `test_reg::generate_trait_method` (common/test_reg.rs:11-33) returns `Result<Option<syn::ImplItem>>` but every success path is `Ok(Some(...))`; the caller in contract.rs:146-152 then needs an `if let Some(...)` for a case that cannot occur.

### [macros/weird] #[storage] silently swallows attribute arguments and emits an unhygienic unqualified `#[derive(Storage)]`

**File:** `crates/macros/src/lib.rs:119-135`

`pub fn storage(_attr_args: TokenStream, ...)` ignores its arguments entirely, so `#[storage(anything = "here")]` compiles cleanly with the args dropped — no error like the sibling macros produce via darling. It also parses the item only to discard the result, then emits `#[derive(Storage)] #item` with an unqualified `Storage` path: unlike every other generated reference in this crate (which uses `::sdk::...`, e.g. `#[derive(::sdk::ReadXDR, ...)]` in contract_type.rs:49), this only resolves if the user happens to have the `Storage` derive imported, yielding a confusing 'cannot find derive macro `Storage`' error pointing at code the user never wrote.

### [native-exec/footgun] tx.call() may only be invoked once per transaction; second call panics — undocumented

**File:** `crates/native-exec/src/mock_transaction_handler.rs:438-451`

`call()` is documented only as "Convenience: open a root frame targeting the given contract address", but it delegates to `open_root_frame()` which does `assert_eq!(self.state, MockTransactionState::NotStarted, "cannot open root frame: transaction already started")`. The natural test pattern — calling two entrypoints inside one `env.transact(|tx| ...)` closure — panics. The one-transaction-one-root-frame invariant is stated only in a non-rustdoc `//` comment on the struct ("Can run one, and only one, transaction", line 348-349), invisible in generated docs. Neither `call`, `transact`, nor the `TestEnvironment` doc example mentions it.

### [native-exec/footgun] UnwindSafe bounds on run/execute/run_continuable are theater: the impl uses AssertUnwindSafe anyway

**File:** `crates/native-exec/src/mock_transaction_handler.rs:226-245`

`execute` requires `impl FnOnce(&mut Self) -> FrameAction + UnwindSafe` but then immediately does `std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| lambda(self)))`, discarding the bound's guarantee. The bound propagates to `FrameBuilder::run` (frame_builder.rs:59-61), `run_continuable` (99-101), `ContinuableFrame::run_continuable` (136-138), `invoke_wrapper` (invoke_error.rs:219-223), and `MockContract::register_method` (mock_contract.rs:63-65). So test authors who capture `&mut` state or RefCell in a closure get a compile error and must wrap in `AssertUnwindSafe` themselves — pure friction with zero added safety, since the harness already asserts unwind safety internally.

### [native-exec/footgun] transact_manual silently aborts the transaction if the closure forgets to commit

**File:** `crates/native-exec/src/test_environment.rs:518-529`

The rustdoc says "The closure ... is responsible for calling tx.try_commit_transaction() or tx.abort_transaction()", but nothing enforces it. If the author forgets, `MockTransactionHandler::drop` (mock_transaction_handler.rs:463-471) auto-aborts with only an `eprintln!("Cleanup: Aborting mock transaction that was not committed")` — captured and hidden by the test harness — so the test passes while none of its state changes were committed; later `advance_block()`/snapshot reads see nothing and the author debugs the wrong layer. Conversely, calling `tx.try_commit_transaction()` yourself inside the auto-commit `transact()` closure panics on the second commit via `assert_eq!(self.state, MockTransactionState::Running)`; neither doc mentions these interactions.

### [native-exec/weird] exit_error() and return_error() are identical, and the *_error naming lies about non-errors

**File:** `crates/native-exec/src/invoke_error.rs:40-62`

`exit_error()` and `return_error()` both return `InvokeError { error_type: InvokeErrorCode::None }` — two public functions, zero difference, no rustdoc on either. They represent control-flow unwinds (raw_Return / Exit terminating an invocation successfully), so a function named `*_error` carrying code `None` is doubly misleading. `abort_transaction_error()` maps to `InvokeErrorCode::Abandon`, inviting confusion with the entirely different frame `Abort` status that `FrameResult::is_abort()` checks. None of the seven public constructors documents which are real failures vs. unwind markers; host_fn_error.rs:30-41 is the only place the distinction is visible.

### [native-exec/footgun] Harness misuse errors surface as opaque `Box<dyn Any>` panics (eprintln + panic_any pattern)

**File:** `crates/native-exec/src/mock_transaction_handler.rs:270-284`

`MockInvocationFrame::result()` on a not-yet-closed frame does `eprintln!("Cannot get the result of a frame while it is not closed"); panic_any(crate::invoke_error::unrecoverable_error())`. The same pattern is used at frame-open failure (lines 139-145) and close failure (177-180). When this fires outside an `execute` catch_unwind — e.g. calling `ContinuableFrame::finish()` after a closure that returned `FrameAction::Continue` — the test fails with panic payload `Box<dyn Any>` and no message in the panic itself; the explanatory eprintln is on captured stderr where users rarely look. `finish()`'s rustdoc says "Call this after the frame has been closed" but doesn't say what happens otherwise. Plain `panic!` with the message would diagnose itself.

### [native-exec/redundancy] FFI trace/events decoding copy-pasted between MockInvocationFrame and TransactionHandler, with drift

**File:** `crates/native-exec/src/mock_transaction_handler.rs:182-209`

`close_frame_internal` (mock path) and `TransactionHandler::root_invoke` (transaction_handler.rs:95-119) contain near-identical blocks decoding `serialized_trace` and `serialized_events` from FFI results. They have already drifted: the mock path decodes the trace with bare `.unwrap()` (line 190) while the real path uses `unwrap_or_else(|e| panic!("failed to decode trace: {}", e))`; the events panic messages differ ("failed to decode frame events" vs "failed to decode events"). Any future change to the wire format must be applied in both places; a shared helper would remove the divergence.

### [native-exec/weird] Mock transactions hardcode a zero-weight auth claim and zero tx hash (TODO acknowledged)

**File:** `crates/native-exec/src/mock_transaction_handler.rs:360-381`

`MockTransactionHandler::new` always opens the syscall handler with `AuthClaim::kSourceAddressAuthData { allowed_keys: 0, required_weight: 0 }`, `source_tx_hash = [0; 32]`, and `enable_tracing = 1`, flagged with `// TODO(geoff): ways to mock this out`. There is no constructor parameter or builder to override any of these, so auth-dependent contract code paths and tx-hash-dependent logic cannot be exercised through the fluent mock API; the real `TransactionHandler` derives all of this from the SignedTransaction, so behavior diverges between the two harness paths precisely where contracts are most security-sensitive. Worth either exposing a configurable constructor or documenting the limitation on the type.

### [spec-tooling/footgun] `spec-tool rewrite` destructively overwrites the input wasm in place and silently deletes the --json-out file

**File:** `crates/spec-tool/src/main.rs:65`

`run_rewrite` does `let bytes = fs::read(&wasm_path)?; ... fs::write(&wasm_path, rewritten.wasm)?;` — the flag is just `--wasm`, with no `--out`/`--in-place` option and no help text, so users won't expect their input artifact to be mutated (and a failed/interrupted write corrupts it). Additionally, when no spec is found the tool deletes the user-supplied `--json-out` path (main.rs:71-75 `fs::remove_file(&json_out)`); pointing `--json-out` at an existing unrelated file destroys it. Neither behavior is documented anywhere (there is no usage/--help output at all; errors are terse strings like "unexpected argument" without naming the offending argument).

### [spec-tooling/footgun] canonicalize_spec never rejects duplicate exception selectors with different payload types

**File:** `crates/spec-tool/src/lib.rs:358`

`canonicalize_exception_list` sorts by `canonical_type_key(ty)` then `selector`, then calls `types.dedup()`, which only removes fully-identical adjacent entries. Two `throws` entries with the same `selector` but different `ty` (e.g. selector 7 -> Named("A") and selector 7 -> Named("B")) pass validation silently, even though selector-based exception decoding is then ambiguous for any client. The tool carefully rejects conflicting methods, typedefs, events, and whole throws-lists (ConflictingInterfaceThrows), but not the one collision that actually breaks runtime error decoding. Note also the canonical order is by type-name rather than selector, which is surprising.

### [spec-tooling/weird] The CLI's only JSON output is the explicitly non-round-trippable debug format; the round-trippable format is unreachable

**File:** `crates/spec-tool/src/lib.rs:283`

`render_spec_json` is documented as "**not** guaranteed to be parseable back into a ContractSpec", yet it is what `rewrite --json-out` writes (main.rs:69). The lossless pair `spec_to_json`/`parse_spec_json` (lib.rs:273-281, tested in tests/json_roundtrip.rs) is exported from the lib but not exposed by any CLI flag. Downstream tooling consuming the sidecar JSON is forced onto the ad-hoc hand-rolled format (interface_to_json/type_to_json), and the two JSON renderers duplicate the entire type-tree walk and have already drifted in key naming.

### [spec-tooling/weird] Inconsistent JSON keys in the rendered spec: "ty" vs "type", "inner"/"max_len" vs "element"/"len"

**File:** `crates/spec-tool/src/lib.rs:632`

`field_to_json` emits the field type under key `"ty"` (lib.rs:632-637) while `exception_type_to_json` emits it under `"type"` (lib.rs:617-622). In `type_to_json`, vectors nest the element under `"inner"` with bound `"max_len"` (lib.rs:670) but arrays use `"element"` with `"len"` (lib.rs:680), and options use `"inner"`. Since this JSON is the published sidecar artifact of `spec-tool rewrite`, every cross-language consumer must special-case these spellings; once external tooling depends on them they are frozen, so worth unifying now.

### [spec-tooling/footgun] Typedef-only raw fragments are silently swallowed, and can erase a pre-existing final spec section

**File:** `crates/spec-tool/src/rewrite.rs:25`

In `rewrite_wasm_with_spec`, when raw fragments exist but contain no interface (`canonicalize_spec` -> `NoSpecFragments` because `interfaces.is_empty()`, lib.rs:189-191), the error is mapped to `spec = None` (rewrite.rs:30) and every spec section — including any valid pre-existing `groundhogspecv0` final section — is stripped without being re-emitted (the `RAW_SECTION_NAME | FINAL_SECTION_NAME => continue` arm at rewrite.rs:44). A contract author who exported types but forgot the interface gets a wasm with no spec and no warning (the CLI even deletes the json sidecar), and rerunning the tool over an already-finalized wasm that picked up a stray typedef-only raw section silently destroys the previous spec. Pinned by test `rewrite_omits_final_section_when_raw_has_only_typedefs` (tests/rewrite.rs:551), so it's intentional but trap-shaped.

### [spec-tooling/redundancy] ContractTypeInfo is a one-field wrapper whose docs describe data that no longer exists

**File:** `crates/contract-spec/src/lib.rs:59`

`pub struct ContractTypeInfo<'a> { pub ty: view::ContractTypeView<'a> }` wraps a single field, and `ContractTypeRefSpec::INFO` is documented as "The type reference and its transitive type definitions" (lib.rs:69-70) — but the struct carries no type definitions; that role evidently moved to the hidden `__GROUNDHOG_SPEC_ANCHOR_ROOT` linker-anchor mechanism. The doc is stale/misleading, and the wrapper buys nothing over `const INFO: ContractTypeView<'static>` (the `type_ref::<T>()` helper at lib.rs:324 just unwraps it). It is used verbatim by macros and sdk impls, so removal is breaking, but the doc should at least be fixed.

### [spec-tooling/weird] Unknown-variant handling is inconsistent: silent skip in type collection vs panic in canonical keys

**File:** `crates/spec-tool/src/lib.rs:505`

`collect_named_types` ends with a silent `_ => {}` after exhaustively listing primitives (lib.rs:493-505), while `canonical_type_key` (lib.rs:591), `type_def_symbol` (lib.rs:600), `type_to_json` (lib.rs:682), and `resolve_reachable_type_defs` (lib.rs:442) all `panic!("unknown ContractType variant")` in the same situation. If groundhog-spec adds a ContractType variant that can embed a Named reference, `collect_named_types` silently fails to resolve its typedefs (producing a spec that passes validation but omits definitions), whereas the sibling functions would crash. One policy should win — either error consistently or skip consistently.

### [spec-tooling/weird] ContractType::Bytes (XDR tag 9) exists in the spec model but no Rust SDK type can ever produce it

**File:** `crates/contract-spec/src/lib.rs:150`

`[u8]`, `&[u8]`, and `BytesM` all lower to `Vector(U8, u32::MAX)` (lib.rs:150-179), never to `ContractTypeView::Bytes` (view.rs:64); the macro tests even assert the absence of `ContractTypeView :: Bytes` (crates/macros/src/common/contract.rs:547,566,591). So the variant is deliberately unreachable from the Rust SDK, yet it remains in the wire enum, round-trip tests, JSON rendering, and the type-collection switch. Nothing documents whether `bytes` and `vec<u8>` are wire-equivalent or which one other-language SDKs should emit — two spellings for the same payload is a cross-language spec-comparison hazard (identical interfaces hashing/comparing unequal). At minimum the intent deserves a comment on the variant.

### [spec-tooling/redundancy] Leak check re-encodes every raw fragment once per data segment

**File:** `crates/spec-tool/src/rewrite.rs:126`

`ensure_raw_fragments_stay_out_of_linear_memory` calls `encode(fragment).expect(...)` inside the inner `for fragment in raw_fragments` loop, which itself sits inside the per-segment loop — every fragment is XDR-encoded segments×fragments times, then scanned with a naive `windows()` substring search over the full data segment. The encodings are invariant and should be computed once before the loops; for large contracts (many fragments, multi-MB data segments) this makes `rewrite_wasm` needlessly quadratic.

### [small-crates/footgun] contract_entrypoint_id hashes fields with no domain separators, unlike its sibling selectors

**File:** `crates/contract-util/src/lib.rs:109-117`

The entrypoint ID is computed as `sha256(INTERFACE_NAME || MAJOR_VERSION || METHOD_SIGNATURE)[0..8]` with the three fields concatenated directly: `hasher.update(interface_name...); hasher.update(major_version...); hasher.update(method_sig...)`. Because there is no separator, distinct triples can produce identical IDs whenever the interface name ends in characters that the version string could supply, e.g. interface "Foo1" + major "2" + method "x" hashes the same bytes as interface "Foo" + major "12" + method "x" (both "Foo12x"). The two sibling functions in the same file, `exception_selector` and `event_signature` (lines 144-152, 168-174), both insert `b"::"` separators, so the omission here looks like an oversight rather than a design choice. Entrypoint IDs route contract method dispatch, so a collision silently routes a call to the wrong method. Likelihood is low (names are author-controlled), but the inconsistency within one file is concrete and the failure mode is silent misrouting.

### [small-crates/footgun] EmittedEvent::payload silently clamps when num_topics overruns the buffer; topic() signals the same condition differently

**File:** `crates/event/src/emitted_event.rs:14-23`

`payload()` computes `topics_extent = min(TOPIC_SIZE * event.num_topics as usize, event.serialized_event.len())` and returns `&event.serialized_event[topics_extent..]`. For a malformed event where `num_topics * 32 > serialized_event.len()`, this silently returns an empty payload (which may then XDR-decode "successfully" for zero-field payload types) instead of surfacing the corruption. Meanwhile `topic()` (lines 35-37) returns `None` for exactly the same out-of-bounds condition. Two accessors on the same view classify the same malformed input as "fine, empty" vs "absent". Also `num_topics as usize * 32` relies on the clamp to avoid garbage; nothing validates the documented 0..=3 topic cap. A contract author asserting on payload contents will get a confusing decode error (or false-positive empty match) rather than "event is malformed".

### [small-crates/redundancy] wasm and rv32 bump allocators are copy-pasted and have already drifted (checked vs unchecked arithmetic, stale name/docs)

**File:** `crates/wasm-runtime/src/allocator.rs:28-31`

The two guest allocators are near-identical copies. The rv32 version (crates/rv32-runtime/src/allocator.rs:34-42) was hardened to use `checked_add(...)` and returns null on overflow; the wasm version still does unchecked `let aligned = (cursor + align - 1) & !(align - 1); let new_cursor = aligned + size;` — it is only saved from wrap-around by `Layout`'s size<=isize::MAX invariant, and the divergence shows the copies are no longer maintained together. The wasm struct is also named `SlabAllocator` though it is a bump allocator (the rv32 copy says `BumpAllocator`), and the rv32 module doc still says "Mirrors `sdk::allocator::SlabAllocator` (the WASM version)" — a path that no longer exists since the allocator moved into crates/wasm-runtime. The shared logic could live in one target-independent function (like rv32's `dispatch.rs` pattern) with only the bound differing.

### [small-crates/missing-docs] StorageCostModel public API has zero rustdoc: no units, no warning on no_cost_model(), missing accessors for three model fields

**File:** `crates/storage-cost/src/model.rs:3-30`

`pub struct StorageCostModel`, `pub fn new`, `pub fn no_cost_model`, `pub fn serialize`, and `pub fn as_xdr` carry no rustdoc at all. The cost methods that are documented say only e.g. "Cost of a bytestring of the given length" with no statement of units (token base units? gas?) or why the return type is `i128` for `u64` fields. `no_cost_model()` constructs an all-zero cost model — fine for tests, but nothing says "test-only"; a user estimating storage fees against it gets 0 for everything with no signal. Additionally the XDR model has `auth_per_key_cost`, `rcq_cost_per_entry`, and `replay_cache_cost_per_entry` fields (lines 20-22) for which no cost accessor exists, so the wrapper covers only a subset of the model and callers must escape-hatch through `as_xdr()` without knowing the coverage is partial.

### [small-crates/weird] hogd-sys build errors and env vars still say "native_exec", which is now a different crate in this workspace

**File:** `crates/hogd-sys/build.rs:330-334`

The wasm-target rejection reads `"native_exec does not build for wasm32-unknown-unknown"`, the version-mismatch error (line 419) instructs "Please update TEST_FFI_VERSION in native_exec/build.rs to match the header", and the force-reconfigure env var is `NATIVE_EXEC_ALWAYS_CONFIGURE` (lines 339, 969). The package is `groundhog-sys` in `crates/hogd-sys`, and the workspace contains a separate `crates/native-exec` crate (which has no build.rs constant to update). A user hitting the version-mismatch error is directed to edit the wrong crate. Evidence of a rename that never updated the user-facing strings.

### [small-crates/footgun] Download auth passes user:password on the curl command line; empty HOGD_FFI_USER silently disables auth

**File:** `crates/hogd-sys/build.rs:655-668`

`add_auth_to_curl` does `cmd.arg("-u").arg(format!("{user}:{pass}"))`, putting the secret from `HOGD_FFI_PASSWORD` into curl's argv where it is visible to every local process via `ps` for the duration of the (retried, possibly minutes-long) download — curl supports `--netrc`/stdin config to avoid this. Also, the auth is added only `if !user.is_empty()`: if a user exports `HOGD_FFI_USER=""` (or a CI template leaves it blank) while the password is set, the build silently downloads unauthenticated — the "password not set" warning is in the other branch, so no diagnostic fires. Related: if the `.sha256` checksum download fails, verification is skipped with only a cargo:warning (lines 769-774, 844-854), so an authentication or CDN failure degrades to unverified binaries linked into the test harness.

### [small-crates/redundancy] bottle-test main.rs carries dead residue: commented-out grants, unused address, duplicate nonce constants

**File:** `crates/bottle-test/src/main.rs:15-37`

`TARGET_NONCE` and `TESTER_NONCE` are two constants with the identical value `SHA256Hash::new([7; 32])` (lines 15-16) — they only avoid producing colliding deploy addresses because the deployers differ, and nothing notes that. Both `setup_grant_storage_tokens` calls are commented out (lines 31, 37: `// env.setup_grant_storage_tokens(&tester_addr, 1_000_000_000);`) and `_target_addr` (line 35-36) is computed via `deploy_address` and never used, suggesting an unfinished migration to the new fluent harness (consistent with recent commit "Standardize tests on the fluent native-exec pattern"). If storage grants are genuinely unneeded under `TestEnvironment::no_cost()`, the comments and the dead address computation should be deleted rather than left as zombie code.

### [small-crates/weird] bottle-test-if breaks every workspace inheritance convention used by its sibling crates

**File:** `crates/bottle-test-if/Cargo.toml:3-9`

Unlike every other crates/* package (which use `version.workspace`, `authors.workspace`, `license.workspace`, etc.), bottle-test-if hardcodes `version = "0.1.0"`, `authors = ["Groundhog Team"]`, `license = "MIT OR Apache-2.0"`, and `repository = "https://github.com/groundhogchain/sdk_rs"`, and its root-workspace pin is path-only (`bottle-test-if = { path = "crates/bottle-test-if" }`, root Cargo.toml:78) with no `registry`/`version ="=..."` like the other 14 pins. It will silently miss every workspace-wide version bump (the bump-version flow keys off the inherited fields). Also `#![cfg_attr(not(any(feature = "dev")), no_std)]` in src/lib.rs:4 wraps a single predicate in `any(...)` — harmless but a reread-trigger suggesting clauses were deleted without cleanup.

### [consumer-experience/footgun] Counter example demonstrates unguarded initialize() and a silent i128→u64 truncating cast

**File:** `example-contracts/counter/src/implementation.rs:17-31`

The flagship-simple example's `initialize(&mut self, initial_value: u64)` is just `self.value.strict_inc(initial_value as i128)` — callable any number of times by anyone, each call adding to the counter. Other code in the same repo shows two different guard patterns (swap_driver/src/swapv2_driver.rs:25 uses `panic!("Already initialized.")`, i.e. a fault instead of a typed throw; groundhog-erc20-if defines `ERC20Error::AlreadyInitialized`). A contract author copying counter's pattern into a contract where initialize sets an owner gets a re-initialization/ownership-takeover bug. Additionally `get()` returns `self.value.try_get().unwrap_or(0) as u64` — NNInt stores i128 (storage/nnint.rs:28), so values above u64::MAX (reachable via repeated `increment`) are silently truncated.

### [consumer-experience/footgun] Generated client builder helpers have asymmetric receivers: with_gas_limit(&self) vs with_commit_locking(self)/with_read_only(self)

**File:** `crates/macros/src/common/client.rs:122-152`

Every generated client gets three look-alike builder methods, but `with_gas_limit` takes `&self` and clones (the original client stays usable — counter/tests/test.rs:90-93 even has a comment proving this non-obvious behavior), while `with_commit_locking` and `with_read_only` take `self` by value and consume the client. Chaining `client.with_gas_limit(n)` then continuing to use `client` works; doing the same after `with_commit_locking()` is a move error (or, with a rebind, silently a different client than the user thinks). An API family this symmetric-looking should have uniform receivers; none of the three is documented (the generated methods carry no rustdoc).

### [consumer-experience/redundancy] example-contracts workspace lists rcq_test twice and pins a different xdrs version than the root workspace

**File:** `example-contracts/Cargo.toml:15,20`

`members` contains "rcq_test" at two positions (after "hasher" and after "test_bytestring") — harmless to cargo but clear copy-paste drift in exactly the file the root manifest warns must be manually kept in sync (sdk_rs/Cargo.toml:53-55). The embedded workspace also pins `xdrs = "0.7.0"` with no features while the root pins `xdrs = { version = "0.7.1", features = ["int128", "heapless"] }`. Related drift: the rcq_test crate is disabled via an empty marker file `example-contracts/rcq_test/disabled` that only scripts/dev.sh:406 understands — an undocumented mechanism a reader of the example corpus has no way to discover.

### [consumer-experience/weird] groundhog-erc20-metadata-if: wrong crate description, unconditional no_std, and undocumented initialize overlap with ERC20

**File:** `standard-interfaces/groundhog-erc20-metadata/Cargo.toml:8 and src/lib.rs:1,13`

The Cargo description is "ERC20 standard interface for Groundhog smart contracts" — copy-pasted from groundhog-erc20-if; it should say metadata. The crate uses bare `#![no_std]` while its two sibling interfaces use `#![cfg_attr(not(feature = "dev"), no_std)]`. More substantively, `ERC20Metadata::initialize(administrator, name, symbol, decimals)` is undocumented and silently subsumes `ERC20::initialize`: erc20-test/src/contract_test.rs:34-53 calls ONLY the metadata initialize when the `metadata` feature is on (the plain `client.initialize` is under `#[cfg(not(feature = "metadata"))]`), so a conforming token must make the metadata initialize also perform ERC20 initialization — an interface contract stated nowhere.

### [consumer-experience/footgun] swap_driver test setup silently discards the airdrop transaction result

**File:** `example-contracts/swap_driver/tests/common/setup.rs:25`

`exec_tx(&env, &TEST_ADDR, || airdrop_tokens(&token_addrs));` drops the returned `Result<T, TransactResult>` — every other setup step on lines 20, 30, 37 uses `?`. The `unused_must_use` warning that would catch this is suppressed by the blanket `#![allow(unused)]` in tests/common/mod.rs:1. If the airdrop transaction aborts, setup 'succeeds' and the failure surfaces later as baffling balance assertions in buy/swap/redeem tests. As the largest end-to-end example in the corpus, this is exactly the multi-tx test scaffolding users will copy, blanket-allow included.

### [cross-crate/weird] sys_exn vs sys_result: same function names with opposite failure semantics, and the two stacks have drifted apart

**File:** `crates/sdk/src/lib.rs:81-92`

The sdk deliberately ships two parallel primitive-storage APIs: `sys_result` (returns `SysResult`) and `sys_exn` (throws), with identical function names — `sdk::bytestring_get_len(key) -> u32` (throws) vs `sdk::sys_result::bytestring::bytestring_get_len(key) -> Result<u32>`. Only the import path distinguishes them, and `sys_exn` is `#[doc(hidden)]` while its contents are glob-re-exported at the root, so rustdoc shows the throwing functions as root items with no visible counterpart. Worse, the two stacks have drifted: nnint exists only in sys_exn (no Result API at all); sys_exn/rcq.rs has 9 functions (alloc + try_ variants) while sys_result/rcq.rs has 4 and is the only place `retrocausal_queue_peek_len` exists; sys_exn/delete.rs additionally has `delete(key, exclusive: bool)`, a bool-flag duplicate of delete_shared/delete_exclusive (the kind of redundant spelling commit 49b7f08 claimed to remove). A user switching between styles will find functions missing or behaving differently under the same name.

### [cross-crate/footgun] sdk `panic_logging` feature silently does nothing on rv32 targets

**File:** `crates/sdk/Cargo.toml:97`

`panic_logging = ["groundhog-wasm-runtime/panic_logging"]` is documented in sdk's Cargo.toml as 'send panic messages to the log() syscall' with no target qualifier, but it forwards only to the wasm runtime. groundhog-rv32-runtime has no panic_logging feature (crates/rv32-runtime/Cargo.toml features: default, alloc) and its `#[panic_handler]` (crates/rv32-runtime/src/panic.rs) discards `_info` entirely and issues `ebreak` — the file's own TODO admits panic info routing is unimplemented. A contract author debugging an rv32 contract enables the feature, rebuilds, and gets no panic messages with no warning. The asymmetry between the two guest runtimes (wasm: optional formatted ConsoleLog; rv32: silent trap) is undocumented at the sdk level.

### [cross-crate/redundancy] Two deploy-address computations with confusable names: syscall-based get_deploy_address* vs host-side deploy_address

**File:** `crates/sdk/src/mock/contract_test.rs:300`

`sdk::deploy::get_deploy_address(nonce, hash)` / `get_deploy_address_for_other(addr, nonce, hash)` ask hogd via syscall (crates/sdk/src/deploy.rs:8-38), while `sdk::mock::contract_test::deploy_address(deployer, nonce, hash)` re-implements the scheme host-side by XDR-encoding ContractDeploymentHashPreimage and sha256-ing it. Three problems: (1) the host copy silently drifts if hogd's scheme changes — nothing ties them together (no shared test comparing the two); (2) under the `mock` feature both `deploy_address` and `get_deploy_address_for_other` are exported at the sdk root with no naming pattern distinguishing 'computed locally' from 'asked the VM'; (3) `deploy_address` (pure computation) sits one keystroke from `deploy_contract` (crates/sdk/src/deploy.rs:40, actually performs a deployment) — neither has rustdoc warning about the difference. crates/bottle-test/src/main.rs already mixes both (`deploy_address(&OWNER, ...)` then `deploy_contract(...)`) and relies on a comment to keep them consistent.

### [cross-crate/redundancy] exit_error() and return_error() are identical, and both construct an 'error' whose code means success

**File:** `crates/native-exec/src/invoke_error.rs:43-50`

Both functions return `InvokeError { error_type: InvokeErrorCode::None }` — byte-for-byte duplicates with different names. They are used as panic payloads for the Return/Exit host-fn codes (crates/native-exec/src/host_fn_error.rs:31-36), i.e. non-error control flow: `invoke_wrapper` later downcasts and reports `panic: None.0`, which the FFI layer treats as a normal return. Naming things `*_error` with `InvokeErrorCode::None` is doubly misleading for anyone extending the mock-syscall layer — a third party matching on `error_type` to detect failure will treat a successful Exit as an error (or vice versa). Either collapse the two into one honestly-named constructor (e.g. `control_flow_unwind()`) or document why two names exist.

### [cross-crate/weird] Hasher panics on syscall failure while every sibling syscall wrapper throws; Drop can panic

**File:** `crates/sdk/src/hasher.rs:50`

`Hasher::update`/`Hasher::finish` handle syscall failure with `panic!("SHA256Update: syscall failed: {:?}", e)`, whereas the workspace-wide convention for guest syscall wrappers is `syscall_throw(SyscallNo::..., errno)` — see the one-shot `sha256` in the very same domain (crates/sdk/src/crypto.rs:12) and all of sys_exn/. On guests, panic vs syscall_throw produce different failure surfaces (trap vs structured exception), so the incremental and one-shot hashing APIs fail differently for the same underlying errno. Additionally `Hasher::new` panics when all 8 slots (N_HASHERS) are in use — documented — but `Drop for Hasher` also panics (`Hasher::drop: internal error: already freed`), which converts any unwinding path that double-frees into an abort. The slot-exhaustion limit interacts badly with the niche `Drop` strategy of leaving VM slots dirty; none of this is mentioned on `finish`/`update`.

### [cross-crate/missing-docs] Root-exported syscall modules crypto.rs and deploy.rs have zero rustdoc; verify_signature_ed25519 swallows error causes

**File:** `crates/sdk/src/deploy.rs:8`

`grep -c '///'` returns 0 for both crates/sdk/src/crypto.rs and crates/sdk/src/deploy.rs, yet their items (`sha256`, `verify_signature_ed25519`, `get_deploy_address`, `get_deploy_address_for_other`, `deploy_contract`) are core, root-exported contract-author API. Undocumented hazards a user will hit: (1) all of them throw syscall exceptions on failure (not Result, not panic) — invisible from the signatures; (2) `verify_signature_ed25519` returns `sys::uninit_VerifySignature(...).is_ok()`, conflating 'signature invalid' with every other syscall failure (bad input length, out of gas at the boundary, etc.) into `false`; (3) `deploy_contract` performs an actual deployment while its name pattern matches the pure `get_deploy_address` computations; (4) `get_deploy_address` vs `get_deploy_address_for_other` differ only in the implicit self-address — none of the nonce/hash domain semantics are stated.

### [cross-crate/redundancy] Dependency pins duplicated outside [workspace.dependencies], one already diverging (sha2 default features); root workspace pins groundhog-syscall that no member uses

**File:** `crates/native-exec/Cargo.toml:43`

The workspace table pins `sha2 = { version = "0.11.0", default-features = false }`, but native-exec declares `sha2 = { version = "0.11.0" }` directly — same version, different features (defaults on), bypassing the single-source-of-truth table. Likewise hogd-sys declares `thiserror = { version = "2.0.12", default-features = false }` directly instead of `workspace = true` (the workspace table has the identical entry), and `syn = "2.0.93"` is spelled out independently in crates/contract-util/Cargo.toml and crates/macros/Cargo.toml. Also: root Cargo.toml:84 pins `groundhog-syscall = "=0.17.0"` in [workspace.dependencies], but no member of the root workspace depends on it — only the separate integration_test workspace, which carries its own copy of the pin (integration_test/Cargo.toml:62), so the root entry is dead and the two copies can drift. example-contracts/Cargo.toml:79 pins `xdrs = "0.7.0"` vs the root's `0.7.1` with features ["int128", "heapless"]. The root Cargo.toml comment already admits these embedded-workspace configs 'need updating' manually whenever versions bump.

### [cross-crate/redundancy] Three public spellings for the XDR vocabulary and two names for the hash type at the sdk root

**File:** `crates/sdk/src/serialize.rs:1`

`sdk::serialize` re-exports {ReadError, ReadResult, ReadXDR, ReadXDROwned, WriteError, ...} from xdrs; `sdk::xdr` is a full `pub use xdrs::*` glob (crates/sdk/src/xdr.rs); and lib.rs re-exports an overlapping subset at the root both via `pub use serialize::*` (line 142) and via `pub use xdr::{ReadError, ...}` (line 150) — so `sdk::ReadXDR`, `sdk::xdr::ReadXDR`, and `sdk::serialize::ReadXDR` all coexist, one commit after #236 'narrowed the xdrs glob at the sdk root'. Similarly `crates/sdk/src/hash.rs:5` declares `pub type Hash256 = crate::crypto::SHA256Hash;` and the root re-exports both names (`pub use hash::*` and `pub use types::*`), so the same 32-byte hash type has two interchangeable public names (deploy_address takes `&Hash256`, deploy_contract takes `&SHA256Hash`). There is also a name collision in spirit: public `sdk::serialize::encode_xdr` returns `Vec<u8>` while doc-hidden `sdk::internal::encode_xdr` returns a stack `MUBuf` — same name, different buffer semantics.

### [cross-crate/redundancy] Guest bump allocators are copy-pasted and have drifted (checked vs unchecked arithmetic, stale cross-reference)

**File:** `crates/wasm-runtime/src/allocator.rs:28`

rv32-runtime/src/allocator.rs and wasm-runtime/src/allocator.rs are the same bump allocator duplicated, but the rv32 copy was hardened with `checked_add` overflow guards while the wasm copy still computes `(cursor + align - 1) & !(align - 1)` and `aligned + size` unchecked (safe today only because Layout caps size at isize::MAX and the page is 64 KiB — an invariant nobody wrote down). The rv32 file's module doc also says it 'Mirrors `sdk::allocator::SlabAllocator` (the WASM version)' — a stale path; the wasm allocator lives in groundhog-wasm-runtime, not sdk, since the runtime split. Future fixes (like the checked-add hardening that already happened once) must be applied twice and demonstrably aren't.


## Completeness critic notes (none addressed)

Additional observations (areas no finding touched):

1. **PR CI never runs integration tests, examples, lint, fmt, or the wasm-string guard — and one skip flag is dead** (`.github/workflows/sdk_tests.yml:34-41`, `scripts/schedules/ci.toml:43`). The GitHub workflow sets `SKIP_DOCS/EXAMPLES/INTEGRATION/LOGGING/LINT/FMT/WASM_STRINGS=1` and runs only sdk unit tests; the internal scheduled CI conversely sets `SKIP_SDK_TESTS=1`. No single gate runs everything, and `SKIP_SPEC_TESTS="1"` in sdk_tests.yml matches no `skip SPEC_TESTS` point in `scripts/dev.sh` — a stale flag implying a spec-test stage silently vanished.

2. **~8 of 35 integration_test contracts are silently disabled via two inconsistent mechanisms** (`integration_test/Cargo.toml:8-37`, `integration_test/contracts/*/disabled`). Four (cxs_roundtrip, txs_roundtrip, manual_revert_*) are commented out of `members`; four more (nnint_output_fault_test, output_alias_test, syscall_parse_hardening_test, rv32_smoke_counter) are not members at all and are skipped only by a bare `disabled` marker file that `scripts/dev.sh ls_contract_packages` honors — nothing even type-checks them, so they bitrot invisibly. None of the markers say why. Notably rv32_smoke_counter is disabled even though rv32 dispatch is active recent work, and `scripts/check-rv32-dispatch.sh` has no in-repo caller (only a comment mention in `crates/rv32-runtime/Cargo.toml`).

3. **Stale allowlist in the wasm-size guard** (`scripts/check-wasm-size.sh:30`): `ALLOW_LIST="exceptions swap_router"` — no `swap_router` package exists anywhere (example-contracts has `swap`, `swap_driver`, `swap_request`), so the entry is dead and either a renamed debugging-built contract now fails the check or was quietly dropped.

4. **Embedded-workspace hygiene drift** — `integration_test/Cargo.toml:40-41` declares `repository`/`homepage` as `github.com/groundhogchain/example-contracts.git` (copy-paste, wrong repo); root `.gitignore` ignores only `/target/`, and integration_test/ and logging-test/ have no `.gitignore` (example-contracts does), so `integration_test/target/` is already polluting `git status`. The root Cargo.toml comment admits version pins in these embedded workspaces "must unfortunately be updated manually" — the root cause behind the consumer-experience xdrs-pin-mismatch finding, but no one named the structural problem.

5. **The committed root `TODO` file already confesses the sweep's biggest themes** (`TODO:1-8`): "tests to fix: test_bytestring disabled, tests/aborts disabled in Cargo.toml, itc: several are disabled?", and "finish converting syscalls to abort-on-err" — direct corroboration of the cross-crate sys_exn/sys_result drift finding and the erc20-test disabled-abort-checks finding. Known debt is tracked only in a flat TODO with no issue links, so reviewers rediscovered it independently.

Coverage gaps: no reviewer looked at CI/workflow wiring (`scripts/dev.sh`, `.github/workflows/`, `scripts/schedules/`), integration_test/ (35 contracts, zero findings), logging-test/, `crates/rv32-runtime` (the disabled smoke contract is its only end-to-end exercise), `crates/test-vectors`, or `etc/`. Systemic theme nobody named: **test debt is managed by silent disablement** — `disabled` marker files, commented-out workspace members, SKIP_* env flags, off-by-default abort checks — with no CI signal that anything is skipped, so coverage erodes invisibly across three separate disablement mechanisms.
