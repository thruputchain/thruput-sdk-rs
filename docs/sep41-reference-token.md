# Stellar asset reference token

`example-contracts/erc20_issuer` uses Groundhog ERC20 with optional issuer
controls. [SEP-0041](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
informs the issuer behavior; this is not the Soroban ABI.

## Semantic differences

- Allowances are signed deltas with optional exclusivity, rather than absolute
  approvals. Ordinary deltas and spending can merge. Exclusive operations keep
  their lock through later writes in the transaction, including clear/regrant.
- Standing allowances do not expire. Expiring a signed allowance increase
  does not expire the allowance it creates. Bounded signed transfers consume
  the whole grant even when spending less than its amount.
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

The clear methods delete storage but do not yet route released collateral to
the caller ([GH-431](https://linear.app/grndhg/issue/GH-431)). Tests disable
resource charges. Native HOG and RPC/CLI adoption are separate follow-ups in
[GH-414](https://linear.app/grndhg/issue/GH-414); the native-HOG conformance test
remains disabled until its entrypoints match.
