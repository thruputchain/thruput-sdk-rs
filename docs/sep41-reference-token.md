# ERC20 reference token for Stellar assets

[GH-414](https://linear.app/grndhg/issue/GH-414/add-a-sep-41-style-reference-token)
uses the standard Groundhog ERC20 interface. Issuer controls are optional
extensions. Applications should use the same balance, transfer and allowance
calls for ordinary tokens, native HOG and wrapped Stellar assets.

## Interface

- `ERC20`: transfers, balances, supply, `adjust_allowance(spender, difference,
  exclusive)`, `clear_allowance(spender)` and `clear_balance()`. Transfer amounts
  are `u128`, bounded to `i128::MAX`; allowance differences are `i128`.
- `ERC20Metadata`: names, symbols and decimals. Symbols now allow 12 bytes,
  including classic Stellar asset codes.
- `ERC20Admin`: initialize metadata and immutable policy; rotate admin/minter.
- `ERC20Authorization`: authorize or freeze holders.
- `ERC20Clawback`: forced destruction when policy permits it.
- `ERC20Burn`: holder burns and allowance-funded burns.
- `ERC20Permit`: bounded signed transfers, signed allowance increases and signed
  allowance clearing.

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
The issuer controls are shared. Allowances and signed transfers differ:

| Operation | #479 | This branch |
| --- | --- | --- |
| Grant or reduce | Absolute `approve` | Signed `adjust_allowance` difference |
| Concurrency | Every approval is exclusive | Caller selects `exclusive` |
| Absolute replacement | Token entrypoint | Left to opt-in compatibility clients |
| Signed authorization | Standing allowance or immediate transfer | Bounded transfer, allowance increase or separate clear grant |
| Revoke and free storage | Set zero, retain entry | Exclusive `clear_allowance` deletion |
| Empty balance cleanup | No method | Caller-only `clear_balance` deletion |

`adjust_allowance` checks that the local result stays in `0..=i128::MAX`.
With `exclusive=false`, concurrent differences and spending may merge within
those bounds. With `exclusive=true`, other writes to that owner/spender pair
conflict, even when the difference is zero. Later spending, adjustments and
credits preserve the lock. Other pairs remain independent.

Absolute approval is not part of the standard client. A compatibility client can
read the allowance and apply an exclusive difference; that does not prevent
spending that occurs before its transaction executes.

`clear_allowance` revokes any remaining amount and deletes the entry exclusively.
`clear_balance` deletes only the caller's empty balance, otherwise throwing
`BalanceNotZero`. Both accept missing entries and conflict with concurrent writes.
Balance cleanup leaves supply and issuer authorization unchanged and emits no
movement event. Both clear methods SHOULD refund released storage cost to the
caller; this reference does not implement refund routing. Sponsorship and
automatic zero-entry collection remain outside this comparison.

Allowance changes emit `AllowanceChangeEvent { owner, spender, difference }`.
Clearing emits the negative remaining allowance under the same exclusive lock.
Spending emits only the token movement; query `allowance` for the remainder.

Standing allowances do not expire. `ERC20Permit::transfer_from_signed` consumes
an entire grant while transferring at most its amount, without changing stored
allowances. The same permit grant can instead authorize `increase_allowance_signed`;
`clear_allowance_signed` requires a separate clear grant. Signed increases can merge with other allowance deltas; signed clears are
exclusive. Wallets must check the operation expiry and bound the
grant lifetime to it. Expiring a signed increase does not expire the resulting
allowance. Bridge withdrawals must authenticate their destination and operation
separately.

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
only the burn as a supply movement. Allowance and issuer-control events are not
balance movements. Groundhog RPC/CLI is the first consumer; a Stellar-facing
representation still needs agreement with downstream users.

## Remaining integration work

The SDK and integration-contract example use the landed storage-flags API.
Native HOG and the RPC/CLI companion PRs still target #479 and need to adopt the
selected interface before release. The native-HOG conformance test is disabled
until its runtime entrypoints match.

Storage refund routing is tracked in
[GH-431](https://linear.app/grndhg/issue/GH-431/refund-released-token-storage-collateral-to-the-clearing-caller).
Tests here use the hogd test runtime with charges disabled; they do not prove
refund accounting, live event ingestion or bridge operation.

Bridge work remains under GH-55/GH-103; broader asset support and issuer
synchronization remain under GH-429. The
[bridge notes](https://linear.app/grndhg/document/using-the-reference-token-in-the-bridge-b9af0e95e0d1)
record the custody and policy constraints.
