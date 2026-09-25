# ERC20 reference token for Stellar assets

[GH-414](https://linear.app/grndhg/issue/GH-414/add-a-sep-41-style-reference-token)
uses the standard Groundhog ERC20 interface. Issuer controls are optional
extensions. Applications should use the same balance, transfer and allowance
calls for ordinary tokens, native HOG and wrapped Stellar assets.

## Interface

- `ERC20`: transfers, balances, supply, `increase_allowance(spender, difference,
  exclusive)`, `clear_allowance(spender)` and `clear_balance()`. Transfer amounts
  are `u128`, bounded to `i128::MAX`; allowance differences are `i128`.
- `ERC20Client::approve`: an absolute replacement library helper. It is not a
  token entrypoint.
- `ERC20Metadata`: names, symbols and decimals. Symbols now allow 12 bytes,
  including classic Stellar asset codes.
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

## Allowance comparison

This branch is an alternative to [SDK #479](https://github.com/groundhogchain/sdk_rs/pull/479).
The issuer controls and signed-token model are shared. The core allowance calls differ:

| Operation | #479 | This branch |
| --- | --- | --- |
| Grant or reduce | Absolute `approve` | Signed `increase_allowance` difference |
| Concurrency | Every approval is exclusive | Caller selects `exclusive` |
| Absolute replacement | Token entrypoint | `ERC20Client::approve` library helper |
| Revoke and free storage | Set zero, retain entry | Exclusive `clear_allowance` deletion |
| Empty balance cleanup | No method | Caller-only `clear_balance` deletion |

`increase_allowance` checks that the local result stays in `0..=i128::MAX`.
With `exclusive=false`, concurrent differences and spending may merge within
those bounds. With `exclusive=true`, other writes to that owner/spender pair
conflict, even when the difference is zero. Other pairs remain independent.

The `approve` helper reads the calling contract's allowance, computes target
minus current, and invokes an exclusive increase. A spender can still consume
the old allowance before this transaction executes. It is a convenience for
absolute intent, not protection against that ordering race.

`clear_allowance` revokes any remaining amount and deletes the entry exclusively.
`clear_balance` deletes only the caller's empty balance, otherwise throwing
`BalanceNotZero`. Both accept missing entries and conflict with concurrent writes.
Balance cleanup leaves supply and issuer authorization unchanged and emits no
movement event. Refund routing, sponsorship and automatic zero-entry collection
are outside this comparison.

Explicit differences emit `AllowanceChangeEvent { owner, spender, difference }`.
The library `approve` therefore emits a delta event too. `clear_allowance` emits
absolute `ApprovalEvent` with zero. Signed `permit` retains absolute replacement
and its absolute event. These event types have different signatures; consumers
must not reinterpret deltas as remaining allowances. Spending emits only the
token movement, so callers still query `allowance` for the remainder.

Standing allowances do not expire. For a transfer with a deadline, use
`ERC20Permit::transfer_from_signed` with `PermitAction::Transfer` and an expiring
signed operation. The action is bound in both the signed request and the grant;
a transfer grant cannot create a standing allowance. The wallet must bound the
grant lifetime to the operation expiry.
Signed `permit` instead records an allowance that survives the operation's expiry.
A bridge withdrawal must authenticate its destination and operation separately.

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

1. Compare the SDK primitives, reference implementations and shared tests with
   #479. Test mergeable differences, exclusive zero changes, replacement,
   grant/spend/clear, deletion conflicts and empty balance cleanup.
2. After reviewing the SDK design, update a separate ITC/native-HOG companion
   and RPC/CLI event consumer. The existing companion PRs still target #479.
3. Publish the selected SDK interface and replace temporary Git pins in consumers
   before merging them. Do not publish both alternatives at the same version.

The comparison starts with SDK test-runtime execution. It does not establish
native-HOG endpoint support, charged storage refunds, live event ingestion or a
working bridge. RV32 validation and exact local test results belong in the PR.

Bridge integration remains tracked under GH-55/GH-103 and broader Stellar assets
under GH-429. Existing [bridge notes](https://linear.app/grndhg/document/using-the-reference-token-in-the-bridge-b9af0e95e0d1)
continue to describe the original PR until this alternative is selected.
