# Stellar asset reference token

`example-contracts/erc20_issuer` uses Groundhog ERC20 with optional issuer
controls. [SEP-0041](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
informs the issuer behavior; this is not the Soroban ABI.

## SEP-41 as client code

No token implements a SEP-41 interface. `SEP41View` in
`groundhog-erc20-controls-if` provides SEP-41's operations over any token
implementing `ERC20`, `ERC20Burn` and `ERC20Metadata`. It runs in the holder's
contract, so SEP-41's explicit authorizers are the Groundhog caller.

`approve` is `reset_allowance` with the expiry, then `adjust_allowance` with
the amount, in one transaction. The reset's exclusive lock covers the
re-grant, so a concurrent spend conflicts instead of landing beside it. Zero
approval is `delete_allowance`. Expiry is inclusive and measured in Groundhog
blocks, not Stellar ledgers.

## Semantic differences

- Native allowances are signed deltas rather than absolute approvals. Deltas
  and spending can merge. An absolute set is `delete_allowance` followed by
  `adjust_allowance`; the delete keeps its lock through the re-grant.
- `reset_allowance` zeroes an allowance and gives it an inclusive block
  expiry; `delete_allowance` removes both. An expired allowance reads zero
  and adjusting it does not make it spendable again until the next reset or
  delete.
- Expiring a signed allowance increase does not expire the allowance it
  creates. Bounded signed transfers consume the whole grant even when
  spending less than its amount.
- Events are native: transfers, mints and burns map onto SEP-41's, but
  approvals appear as allowance deltas, not SEP-41 approve events.
- Authorization and role changes reach other transactions in the next block.
  A same-block transfer can still use the previous permissions.
- Issuer policy is immutable and clawback eligibility is uniform. Stellar
  trustline history and policy synchronization belong in the bridge.

## Integration boundaries

Deploy and initialize atomically; an uninitialized deployment can be claimed.
The admin can replace the minter, so a policy relay should not receive full
admin authority by default.

A holder burn or clawback does not request a bridge withdrawal. Clawback emits
both a canonical burn and contextual clawback event; count one supply change.
Bridge destinations, deadlines and replay protection need their own signed
operation and records. See the [bridge notes](https://linear.app/grndhg/document/using-the-reference-token-in-the-bridge-b9af0e95e0d1).

The delete methods free storage but do not yet route released collateral to
the caller ([GH-431](https://linear.app/grndhg/issue/GH-431)). Tests disable
resource charges. Native HOG and RPC/CLI adoption are separate follow-ups in
[GH-414](https://linear.app/grndhg/issue/GH-414); the native-HOG conformance test
remains disabled until its entrypoints match.
