# Using the reference token in the bridge

The GH-414 reference token supplies local balances, allowances, issuer controls
and events. A backed wrapper still needs bridge records, custody accounting and
an asset policy. These notes capture the 2026-09-24 comparison with Stellar for
the implementation team. See [the token spec](sep41-reference-token.md) for the
interface and the Stellar comparison.

## Tracking

- [GH-55: core transfer path](https://linear.app/grndhg/issue/GH-55/prototype-core-bridge-transfer-path):
  wrapper policy, accounting and the Quickbridge round trip.
- [GH-103: events and replay protection](https://linear.app/grndhg/issue/GH-103/prototype-bridge-transfer-event-schema-and-replay-protection):
  authenticated transfer records, atomic processing and duplicate delivery.
- [GH-429: broader assets and issuer synchronization](https://linear.app/grndhg/issue/GH-429/define-broader-sep-41-asset-support-and-issuer-synchronization):
  asset admission, holder mapping, issuer updates and custody losses.

The existing v1 asset decision in
[GH-53](https://linear.app/grndhg/issue/GH-53/approve-v1-asset-scope-and-issuer-agnostic-bridge-model)
remains closed. The full bridge design allows revocable/clawback-enabled assets
while accepting custody risk; it generally excludes auth-required assets and
excludes arbitrary contract tokens. Quickbridge's proposed issued asset disables
authorization requirements, revocation and clawback.

## Lock, mint, withdraw and release

Bind a source network and token contract to each wrapper. Preserve raw units or
specify checked conversion. Names and symbols are display metadata, not asset
identity. The reference supports 12-byte symbols and 100-byte names, enough for
valid SAC metadata at the reviewed revision. Custom contract tokens may need
metadata mapping. Destination balances and total supply cannot exceed i128::MAX.

Commit source custody receipt with its deposit record. On delivery, consume the
source transfer ID in the same transaction as destination minting or release.
Bind the record to both networks, bridge generation, asset, amount and recipient.
Use exclusive consumption records on Groundhog so two deliveries of the same
claim cannot both commit in one block.

A withdrawal must authenticate the owner's chosen destination and atomically
record the obligation with token destruction. The bridge can use `TokenBurn::burn_from` with an owner allowance and itself
as spender, or pull tokens into escrow and burn its own balance. Authenticate
the Stellar recipient separately. `ERC20::burn(owner, amount)` is the
administrator operation and obeys clawback policy; it is not a holder withdrawal.

Prefer an expiring signed bridge operation for withdrawals. Bind networks,
asset, amount, recipient and transfer ID in its payload. The standard
`Signed<PermitGrant>` also authorizes creation of a standing allowance, and its
spender chooses the receiver, so it is not by itself an expiring withdrawal.
Use a wire-distinct operation payload, not just another Rust struct name.

## Destruction and custody accounting

A direct holder burn creates no withdrawal claim. Neither token burn nor clawback
events contain a bridge transfer ID or destination, so neither can independently
trigger custody release. For example, burning 10 of 100 wrappers leaves custody
at 100, supply at 90, and no pending withdrawal.

Choose whether the wrapper rejects direct destruction or tracks the remaining
backing as unclaimed. The same decision applies to administrative clawback.
Reconcile custody with supply, pending deposits, pending withdrawals and
identified surplus at consistent checkpoints. Generic token movement events
serve balance consumers; bridge records serve delivery and reconciliation.
Forced destruction emits a canonical `BurnEvent` and a `ClawbackEvent` carrying
context. Count the burn once; do not treat the context event as another debit.

## Issuer controls

The minter cannot freeze holders, claw back or change roles. The admin can replace
the minter, including with itself. A relay given unrestricted admin authority
therefore controls minting too. Use a restricted administrative contract or
explicitly accept that trust model.

Groundhog freeze and role changes become visible to other transactions in the
next block. Same-block transfers may use the old authorization, and mints may use
the old minter. Relay delay adds to this boundary. Define update ordering, stale
update rejection, and behaviour while the relay is behind.

A Stellar depositor and the current Groundhog holder may be different people.
Define authenticated account relationships, including contract holders. Map
source issuer flags and historical clawback eligibility explicitly: the reference
uses an immutable policy with uniform holder eligibility.

Freezing pooled Stellar custody affects all redemptions. Clawing it back can leave
wrappers without sufficient backing. Local freeze/clawback methods do not decide
who bears that loss. Define route suspension, reconciliation, holder treatment and
pending-claim handling before admitting such assets. For arbitrary SEP-41
contracts, method compatibility alone is insufficient; verify actual custody
receipt and supported balance/policy behaviour.

## Integration checks

Demonstrate one configured asset through lock, mint, application transfer,
authorized withdrawal and release. Exercise:

- Duplicate delivery in the same block, later blocks and after relay restart.
- Altered transfer payloads, unauthorized withdrawal recipients, and failed
  mint/release with no consumed claim or surviving token events.
- Pending obligations, direct destruction, reconciliation and visible failure
  status; use persistent relay state and explicit pause/rotation controls.
- Restricted issuer authority and reordered freeze/unfreeze updates when those
  features are introduced.

Changing verifier or deploying a replacement bridge must preserve consumed claims,
pending obligations and custody authority, or explicitly migrate them. Rotating
the token minter alone does not achieve this. Keep verification outside the token
so Quickbridge can precede federation.

The token tests exercise native and RV32 execution, including signed-wallet
flows, supply races and role visibility, with resource charges disabled. They
do not prove live indexer ingestion, Stellar consumer compatibility or a
cross-network round trip. Those checks remain part of integration.
