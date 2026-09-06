# sdk_rs changelog: 28 Aug – 6 Sep 2026

Releases in this window: SDK `0.24.0` → `0.31.0`, standard interfaces `0.1.16` → `0.1.24`.
Pins moved with them: protocol-rs `0.27.0` → `0.32.0`, `groundhog` CLI `0.9.0` → `0.15.0`, test-ffi `0.26.0` → `0.30.0`.
Build contracts with the pinned CLI (`scripts/groundhog-cli-version.sh`); older lowerers reject the new interface-table section.

## Breaking changes

- **`{Client}Error` enums are gone.** `try_*` client methods return `CallError`; a caught exception stays undecoded and is matched with `match_exn!` (or `MatchExn::try_as`) against any exception type. Drops ~5% of deployed code in the `exn_catch` example. (#385)
- **ERC20 mint and burn are their own events.** `MintEvent { to, amount }` / `BurnEvent { from, amount }` replace zero-address `Transfer`s; `TransferEvent` no longer special-cases the zero address. A `groundhog-erc20-if = "0.1"` pin picks this up on the next `cargo update`. (#436)
- **One `dev` feature.** `mock`, `debugging` and `call_tracking` are removed; `dev` is the only feature to enable for native testing. Diagnostics are now target-gated, not feature-gated. (#380, #381)
- **`RawCallError` gained variants.** `Reentrance`, `DepthExceeded` and `InvokeRejected(SysErr)` are distinct from `Fault`; `Fault` now carries a `FaultInfo`. Exhaustive matches need new arms. `CallingError` gains discriminants 4 and 5 (additive on the wire). (#421, #422)
- **Signed-operation wire format changed** (see below); `groundhog-erc1271-if` and the `erc1271_*` examples are removed. `SignedOp` is the ERC-1271 equivalent. (#429)
- **Native `U256` / `I256` spec types.** Specs emit native 256-bit discriminants instead of named typedefs; the raw spec section moved to format v2. (#425)
- **protocol-rs type identity.** The SDK's public API takes and returns `groundhog-xdr` types, so every move of the protocol pin (0.27 → 0.29 → 0.30 → 0.31 → 0.32) is a breaking bump for consumers on a caret pin. (#379, #386, #392, #426, #432)

## New

- **Signed operations** (Groundhog's EIP-712 analogue): `groundhog-signed-op-if` + `groundhog-signed-op`. A typed, domain-separated message signed off-chain authorizes an on-chain action. Replay protection uses the chain's `Uniquifier` `(nonce, block_expiry_height)`: unordered, signer-chosen nonces, expiry as cancellation, and the verifying key comes from contract state rather than the wire. `signed_op_wallet` example. (#390, #429)
- **ERC20 permit** (ERC-2612 analogue): `groundhog-erc20-permit-if`, with `permit` (writes an allowance) and `transfer_from_signed` (moves tokens directly). The token consumes a `Signed<PermitGrant>` auth token minted by the owner's wallet, so it never sees a key or signature. `erc20_permit` + `permit_wallet` examples. (#428)
- **Interface ids and ERC-165.** Every `#[contract]` trait has an `INTERFACE_ID`; `#[contract]` impls register in a linker-collected interface table; `sdk::implements!(id)` scans it. `groundhog-erc165-if` adds `impl_erc165!()` and `probe_interface`. Costs ~290 B on `erc20_pausable`; needs CLI ≥ 0.15.0. (#433)
- **`ERC20Pausable` and `AccessControl` interfaces**, plus an embeddable `AccessControlState` component under `standard-impls/`, and the `erc20_pausable` example wiring all four token interfaces together. (#373)
- **`I256` and the OpenZeppelin math surface** in `groundhog-u256`: `mul_div`, `mul_shr`, `mod_exp`, `inv_mod`, `ceil_div`, `integer_sqrt` and `log2`/`log10`/`log256` with rounding variants, `SignedMath`, and `TryFrom` across every primitive width. Division truncates toward zero like Rust and Solidity; `MIN / -1` panics. (#382)
- **Block context:** `sdk::block::get_network_hash()` and `get_block_timepoint()`. `TimePoint` and `BlockHeight` arithmetic (against `Duration` and block counts) is checked in every profile; `Duration` is in the prelude. (#386, #392)
- **`heapless::Vec<u8, N>` is a contract type**, a bounded byte buffer that decodes without an allocator (unlike `BytesM<N>`). (#435)
- **Test-support crates published:** `bottle-test-if`, `erc20-test`, `erc20-native-test`, `erc20-bottle-test`, so downstream workspaces can pin by version instead of git commit. (#377)

## Diagnostics and correctness

- Diagnostics no longer change what a contract computes: `FaultString` encodes as empty on every target, `ABORT_MAX` drops 544 → 28 and is pinned by a const assertion. Host tests now exercise the encoding that ships. (#380)
- Reentrance and the invoke depth bound surface as their own rejections instead of being indistinguishable from a callee panic; `is_rejected_before_call()` guarantees the callee never ran. (#421)
- A panicking contract's message and source location reach host tests as a value (`fault_message()`, `fault_panic()`), not just stderr. (#422)
- `#[storage]` and `#[topic]` compile errors point at the offending field. `IntoTopic` is implemented for `ContractAddress`, `SHA256Hash` and `&T`, which fixes `SHA256Hash` topics not compiling at all. (#404)
- Generated client builders (`with_gas_limit`, `with_commit_locking`, `with_read_only`) all take `&self` and are documented; the original client is no longer moved. (#394)
- `sdk`'s crate root no longer re-exports hidden `mintable` internals. (#406)
- Host entrypoint guard symbols are ELF-safe for Linux linker version scripts. (#374)

## Build, release, docs

- CI builds `sdk` for the rv32 guest target first, so a guest-only break fails in seconds. (#381)
- Publish-order check in `version-check` catches the vendoring bug that broke the 0.28.0 release; permit interface pre-vendored after 0.30.0 failed. (#418, #431)
- `LICENSE` (Apache-2.0) added; crate descriptions and repository URLs fixed. (#397, #403)
- Docs: Cargo features documented, front page and storage type tables corrected, quickstart follows the scaffold path, WASM-era wording removed. (#384, #393, #411, #402, #423, #442)
