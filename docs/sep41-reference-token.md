# Stellar asset reference token

`example-contracts/erc20_issuer` exposes Groundhog ERC20 and `groundhog-sep41-if`
over the same balances, supply and issuer policy. The latter follows
[SEP-0041 v0.5.2](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
token operations using Groundhog's ABI and caller authorization.

## Allowances

ERC20 keeps delta grants and optional exclusivity. SEP-41 `approve` replaces
an absolute amount and inclusive expiry, measured in Groundhog blocks.
Replacement exclusively writes the grant; ordinary spends can commute within
the remaining amount. A spender can still consume an old grant before its
replacement executes.

The two interfaces have **independent grants**. Clearing or expiring a SEP-41
grant does not revoke a separate ERC20 grant, and neither interface can spend
the other's grant. This keeps the native interface unchanged without allowing
it to bypass SEP-41 expiry. Applications should choose one allowance interface.

## Compatibility boundaries

- Explicit authorizers must equal the Groundhog caller. Soroban authorization
  trees and muxed addresses are not supported.
- Metadata uses Groundhog's bounded name and symbol types. Initialize through
  `ERC20AdminClient::initialize` to supply it.
- Movements use canonical Groundhog events; approvals use `SEP41ApprovalEvent`.
  These are not Soroban event encodings. Clawback still emits a canonical burn
  plus a contextual clawback event; count one supply change.
- Authorization and role changes reach other transactions in the next block.
  A same-block transfer can still use the previous permissions.
- Issuer policy is immutable and clawback eligibility is uniform. Stellar
  trustline history and policy synchronization belong in the bridge.

Deploy and initialize atomically; an uninitialized deployment can be claimed.
The admin can replace the minter, so a policy relay should not receive full
admin authority by default.

A holder burn or clawback does not request a bridge withdrawal. Source proofs,
asset identity, destinations and replay protection still belong in the bridge.
See the [bridge notes](https://linear.app/grndhg/document/using-the-reference-token-in-the-bridge-b9af0e95e0d1).
Storage-refund routing remains separate ([GH-431](https://linear.app/grndhg/issue/GH-431)).
Tests disable resource charges. Native HOG does not expose this SEP-41 interface.
