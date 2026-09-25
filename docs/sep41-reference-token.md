# ERC20 reference token for Stellar assets

[GH-414](https://linear.app/grndhg/issue/GH-414/add-a-sep-41-style-reference-token)
uses the standard Groundhog ERC20 interface. Issuer controls are optional
extensions. Applications should use the same balance, transfer and allowance
calls for ordinary tokens, native HOG and wrapped Stellar assets.

## Interface

- `ERC20`: caller-based transfer, delegated transfer, balances, supply and absolute
  `approve(spender, amount)`. Zero revokes. Amounts are `u128`, bounded to
  `i128::MAX` to match storage and Stellar's positive amount range.
- `ERC20Metadata`: names, symbols and decimals. Symbols now allow 12 bytes,
  including classic Stellar asset codes. Older clients with a 10-byte bound
  need updating to decode longer symbols.
- `ERC20Admin`: initialize metadata and immutable policy; rotate admin/minter.
- `ERC20Authorization`: authorize or freeze holders.
- `ERC20Clawback`: forced destruction when policy permits it.
- `ERC20Burn`: holder burns and allowance-funded burns.
- `ERC20Permit`: existing signed grants, including immediate signed transfers.

The example is `example-contracts/erc20_issuer`. New controls live in
`groundhog-erc20-controls-if`; there is no separate SEP-41 balance or transfer
interface. This is not an ERC-3643 implementation: identity registries,
compliance rules, recovery and forced transfers remain outside this change.

Deploy and initialize in one transaction. The issuer initializer makes its
caller admin and takes a separate minter. The simpler ERC20 and metadata
initializers make the supplied administrator the minter, with authorization,
revocation and clawback disabled. Every initializer shares one exclusive guard;
a publicly deployed but uninitialized instance can be claimed.

The admin can replace the minter, including with itself. The minter has no issuer
control authority. `ERC20::mint` uses that configured mint authority. Both
administrator `ERC20::burn` and explicit `ERC20Clawback::clawback` require the
admin and the same clawback policy; neither bypasses a disabled policy.
Holder-authorized burns use `ERC20Burn`. Policy rejections are additional
`ERC20ControlsError`s, exposed through the standard client's unknown-error path, as with
`ERC20Pausable`. Ordinary range and administrator checks retain `ERC20Error`.

## Allowances and signed actions

`approve` replaces the remaining grant. Exclusive replacement conflicts with
spending and another replacement of that owner/spender pair
in the same block. Different pairs remain independent. Ordinary spends use
checked non-negative deltas and may merge within the available allowance.
A spender may still use an old allowance before a replacement executes; absolute
approval does not eliminate that familiar ordering race.

`approve` and signed `permit` both replace the remaining allowance and emit
one absolute `ApprovalEvent`. The old relative endpoint and delta event are
removed. Spending emits only the token movement; query `allowance` for the
remaining grant. This preserves mergeable allowance debits.

This is a coordinated breaking change. `ApprovalEvent` retains its signature
but now carries an absolute amount, so historical delta events require the
old contract/release context. Rebuild clients and deploy the matching native
runtime together. Previously issued additive permits must be invalidated or
allowed to expire before migrating an existing token. Renaming the controls
crate and interfaces also changes their event signatures and method selectors.

There is no expiry field on a standing allowance. `Signed<PermitGrant>` expires
at the runtime's Groundhog block height. `transfer_from_signed` consumes it and
moves tokens immediately without writing an allowance. The existing grant also
authorizes `permit`, which can create an allowance before expiry that survives
it. A bridge withdrawal requiring a strict deadline must authenticate its own
operation and destination, not treat a general permit as a deadline-bound
withdrawal. Signed payload names alone do not separate permissions: auth tokens
bind a consumer, while the payload must distinguish allowed operations on wire.

## Stellar comparison

[SEP-0041](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
defines Soroban token calls, signed amounts and expiring absolute allowances.
Groundhog keeps its existing caller-based ERC20 interface and signed-action
model. The reference supplies the local issuer operations needed for bridging;
it does not reproduce Soroban's ABI, authorization tree or ledger expiry.

Frozen holders cannot send, receive, mint into their balance or voluntarily
burn. They may grant or revoke allowances; spending still checks both holders.
Clawback bypasses holder authorization but requires immutable enabled policy.
Freeze and role updates reach other transactions in the next block. The bridge
must account for that delay and for source-chain policy history.

Transfer, mint and burn retain standard Groundhog events. Every forced burn
emits one canonical `BurnEvent` plus a `ClawbackEvent` carrying context. Count
only the burn as a supply movement. Approvals and issuer-control events are not
balance movements. Groundhog RPC/CLI is the first consumer; a Stellar-facing
representation still needs agreement with downstream users.

## Work plan and validation

1. Add canonical absolute approval while retaining existing selectors/events.
2. Replace the separate token core with issuer extensions and this ERC20 example.
3. Add matching native HOG approval in [hogd#4145](https://github.com/groundhogchain/hogd/pull/4145). The shared SDK approval suite passes against that runtime build.
4. Update bridge tracking and downstream event fixtures. Integrate Quickbridge
   under GH-55/GH-103; broader asset and issuer synchronization stays in GH-429.

Runtime tests cover replacement, revocation, concurrent writes, checked supply,
rollback, roles, authorization, clawback and signed-wallet flows. They run using
native dispatch and the compiled RV32 token with resource charges disabled.
This does not prove charged execution, live indexer ingestion or a cross-network
round trip. Native HOG needs the companion runtime change before new clients
can use `approve` there; an SDK release alone cannot add that endpoint.

The interface family is bumped to `0.1.37`; controls starts at `0.1.0`.
Package verification builds controls against the packaged ERC20 and metadata
crates, so it cannot silently pick up the older published symbol limit.
The native companion passes 60 allowance/parity tests, including a charged
approval test. To run the four shared SDK approval tests against its FFI:

```sh
ERC20_APPROVE_NATIVE=1 HOGD_FFI_LIB_DIR=/path/to/test-ffi \
  cargo test -p erc20 --test approve --locked
```

Run from `example-contracts/`; the FFI directory must include `bindings.rs`.
The native precompile conformance test is explicitly ignored against the older
bundled FFI. Run it with the matching runtime before release:

```sh
HOGD_FFI_LIB_DIR=/path/to/test-ffi cargo test -p erc20-native-test --test native_token_precompile -- --ignored
```

Remove the ignore when the SDK pins an FFI release containing native approval.
No published native deployment is implied by these local tests.

Bridge accounting, recipient authorization, replay and migration requirements
are tracked in the [Groundhog Bridge notes](https://linear.app/grndhg/document/using-the-reference-token-in-the-bridge-b9af0e95e0d1).
