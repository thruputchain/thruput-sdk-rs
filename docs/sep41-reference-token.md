# Reference token for Stellar integration

Tracking: [GH-414](https://linear.app/grndhg/issue/GH-414/add-a-sep-41-style-reference-token)

Status: first local implementation passes native and RV32 runtime tests. This is a Groundhog adaptation of
SEP-41, not a Soroban ABI implementation or a deployed bridge.

## Purpose

Give Groundhog applications and Quickbridge a reference fungible token with
familiar Stellar behaviour. Build it before the bridge settles. The existing
ERC20 interfaces and contracts remain available.

Reference: [SEP-0041 version 0.5.2](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md),
[SAC](https://developers.stellar.org/docs/tokens/stellar-asset-contract).
SEP-41 defines the common token interface. Issuer authorization and clawback
are additional SAC-style capabilities. A token can support them locally before
we implement synchronization with Stellar.

## Interface and behaviour

Add `groundhog-sep41-if` under `standard-interfaces` and a `sep41` reference
contract under `example-contracts`. Keep the token and administration interfaces
separate. Use the existing SDK contract macros, native test runtime and RV32 build.

The token interface provides `balance`, `allowance`, `approve`, `transfer`,
`transfer_from`, `burn`, `burn_from`, `name`, `symbol`, and `decimals`.

- Amounts are signed 128-bit integers at the interface. Reject negative amounts;
  balances, supply and allowances stay between zero and `i128::MAX`.
- Transfers and burns require the owner's authorization. Delegated transfers
  and burns require the spender's authorization and consume its allowance.
- `approve` replaces the remaining allowance and its expiry. It does not lock
  funds and may exceed the owner's balance. Multiple spends consume one grant.
- An allowance is valid through its expiry height, inclusive; afterwards it
  reads as zero. Compare by widening the u32 expiry, never truncating the current
  height. After `u32::MAX`, all grants are expired and only revocation is allowed.
  A nonzero grant cannot expire in the past. Zero revokes even
  with an expiry in the past. Spending never extends expiry.
- Replacing a live allowance retains the usual approval race across blocks.
  Clients should revoke, check intervening spends, then grant the new amount.
- A separate `adjust_allowance` extension may be added after the first slice.
  It must not change the meaning of `approve` or emit a delta as an absolute
  approval. Its expiry and concurrent-update rules need their own tests.
- `burn` is holder-authorized destruction. `clawback` is administrator-authorized
  destruction. Both reduce supply; only clawback emits a clawback event.

## Groundhog adaptations

| Area | First implementation |
| --- | --- |
| Addresses and calls | Groundhog `ContractAddress`, generated clients and CXS encoding. No Soroban ABI, G-address or muxed-address support. |
| Authorization | Explicit `from` or `spender` must equal the calling contract. Wallets call as owners; applications call as spenders. Soroban invocation authorization trees are not reproduced. |
| Expiry | `live_until_ledger: u32` means the inclusive Groundhog block height, not the Stellar ledger height. Heights from the two networks must not be copied across. |
| Metadata | Required name, symbol and decimals; name up to 100 bytes and symbol up to 12 bytes. These cover valid SAC metadata; arbitrary contract tokens may need display-name mapping. |
| Events | Transfer, mint and burn reuse the established Groundhog token events. Approval and issuer controls have separate typed events. Topic zero is a Groundhog signature; addresses and payload use Groundhog encoding. They are not byte-identical Stellar events. |
| Allowance concurrency | One exclusive amount/expiry record per owner/spender pair. Competing mutations of that pair cannot both commit in one block. Separate pairs remain independent. |

The first implementation favours a simple, checked allowance record over a new
mergeable allowance scheme. Relative arithmetic alone would not make a
read/modify/write record safe. Multiple calls in one transaction may update the
same record; conflicting transactions must retry in a later block. Do not claim
parallel allowance throughput from host sequential tests.

Balances use the SDK's checked debits and mergeable credits. Total supply uses
checked updates. All failures roll back balance, allowance, supply and events.
Initialization and mutable administrative records use exclusive writes.
Freeze and role changes become visible to other transactions in the next block;
they do not retroactively stop transactions using the current block snapshot.

## Issuer controls

Initialize once, in the deployment transaction. The initializer becomes admin
and names the minter, metadata and immutable policy flags. Deploying an
uninitialized contract for someone else to initialize is unsupported.

- `auth_required`: new holders default to unauthorized when enabled.
- `auth_revocable`: permits the admin to freeze a holder. Reauthorizing is
  allowed; freezing fails if revocation is disabled.
- `clawback_enabled`: permits the admin to claw back balances, including frozen
  balances. It requires revocation to be enabled at initialization.
- `authorized` and `set_authorized` expose per-holder status. Transfers check
  both holders; mint and holder burns check the affected holder. Approvals, including positive grants and revocation, remain
  allowed while frozen; they do not authorize movement of a frozen balance. An allowance cannot bypass a holder freeze.
- `admin`, `set_admin`, `minter`, and `set_minter` separate issuer administration
  from minting. The minter can mint but cannot freeze, claw back, or change roles.
  The admin can replace the minter, including with itself. It therefore controls
  mint governance; do not give an issuer-sync relay this authority unless that
  trust is intended. A restricted administrative contract can limit relay calls.
- `total_supply` is a Groundhog extension for tests and bridge accounting.

This is a fixed token policy, not a complete reproduction of SAC trustlines,
issuer-account special cases, historical flag changes, or Stellar account setup.
Clawback policy applies uniformly to this contract's holders.

## Events and downstream work

| Operation | Indexed addresses | Payload |
| --- | --- | --- |
| transfer / transfer_from | from, to | amount |
| approve | from, spender | amount, live_until_ledger |
| burn / burn_from | from | amount |
| mint | to | amount |
| clawback | from | amount |
| set_authorized | id | authorize |
| set_admin | previous admin | new admin |
| set_minter | previous minter | new minter |

Spending allowance emits the transfer or burn event, not another approval.
Freeze changes do not emit balance movements. Clawback emits no additional burn
or transfer. Zero-value successful operations retain their operation's event.

`groundhog-sep41-if` reexports `TransferEvent`, `MintEvent` and `BurnEvent`
from `groundhog-erc20-if`. These signatures and payloads are part of its public
interface. This lets existing Groundhog token consumers recognize movements
without learning a second signature for the same operation. Native and RV32
tests decode real emitted movements through the existing event decoders and
check the raw topic count, addresses and amount.

The released CLI 0.19.0-pre.1 filters these three signatures. The companion
`groundhog-rpc` branch `mm/gh-414-token-history` adds clawbacks as a distinct
movement kind and advances this command's JSON schema to 3. Clawbacks have a
sender and no recipient: `--from` and `--account` include them; `--to` excludes
them. Its raw `events list` command preserves all eight reference event types.
Approval stays separate: the existing ERC20 `ApprovalEvent` carries a delta,
where SEP-41 carries an absolute grant and expiry. A clawback emits one clawback
event, with no extra burn that could double-count the same movement.

Stellar-facing downstream compatibility remains open. Agree the representation
and test ingestion with a consumer before claiming it. SAC's asset-identifier
topic and SEP-41's supported event formats need an explicit mapping; changing
only an event name does not provide compatibility.

### CLI integration and release order

The CLI branch depends on `groundhog-sep41-if =0.1.0`. Until that package is
published, its local checks use a temporary Cargo patch pointing at the extracted
interface package. Use the normalized package, not the source workspace, so its
SDK and ERC20 dependencies resolve to the same registry crates as the CLI.
The current CLI lockfile records this local override and is not ready to ship.

Release the SDK interface first, then remove the temporary override, regenerate
the CLI lockfile against the registry and repeat CLI validation. No package has
been published. Track the SDK PR in GH-414; the companion CLI branch is local.

The CLI tests serve reference-token events over a local HTTP test server and run
the real CLI binary. They check raw event preservation, movement decoding,
participant filters, malformed data, human output and pagination. The event
fixture comes from the SDK's ignored `export_cli_event_fixture` test. Native and
RV32 generation produce identical bytes; the HTTP test supplies synthetic ledger
identities. This verifies the consumer boundary, not indexer ingestion on a live
node. Live-node validation is the next integration check before bridge work.

### Upstream checks

The implementation was checked against SEP-0041 v0.5.2 and these local Stellar
source revisions. These links pin what was inspected, not a claim to track the
latest upstream commit:

- [SAC allowance implementation](https://github.com/stellar/rs-soroban-env/blob/883fd56374fc619afaa4c4c7489563555b7173f2/soroban-env-host/src/builtin_contracts/stellar_asset_contract/allowance.rs):
  replacement approval, inclusive expiry and spending without extending expiry.
  SAC also bounds expiry by its storage TTL; Groundhog does not reproduce that
  limit or automatically delete expired allowance records.
- [SAC events](https://github.com/stellar/rs-soroban-env/blob/883fd56374fc619afaa4c4c7489563555b7173f2/soroban-env-host/src/builtin_contracts/stellar_asset_contract/event.rs):
  separate transfer, mint, burn, clawback and issuer-control operations. SAC adds
  the asset identifier and handles issuer-account special cases.
- [Soroban token SDK events](https://github.com/stellar/rs-soroban-sdk/blob/dd5f06b0563d5f6d9e5d3f0f3e15004aa105ee6b/soroban-token-sdk/src/events.rs):
  scalar and structured amount payloads, including muxed-recipient information.
  The Groundhog reference accepts contract addresses and emits one amount format.

These are source comparisons. The test suite executes Groundhog contracts;
it does not run Stellar contracts or establish cross-network interoperability.

## Quickbridge integration

The bridge mints through its contract address, subject to the token admin's
authority to replace the minter. Its withdrawal method must couple owner-authorized
token destruction with a withdrawal record in the same transaction. Delivery must couple minting with consumption of a unique
source transfer. The token does not accept relay messages, check source finality,
or maintain bridge transfer IDs.

A direct holder burn is not a withdrawal request and must never release custody.
Before using this reference as a backed wrapper, choose whether such burns are
allowed and accounted as unclaimed backing, or rejected by the wrapper policy.
The same decision is needed for clawbacks. The reference contract alone does not
satisfy Quickbridge's wrapped-supply accounting rules.

The local bridge proposal (`~/docs/designdocs/bridge/devnet-quickbridge.md`)
starts with a centralized test bridge and one configured asset. Its issued-asset
option disables authorization requirements, revocation and clawback. The full
bridge design currently excludes arbitrary contract tokens and generally
auth-required assets from v1. It allows revocable or clawback-enabled assets,
accepting the risk to pooled custody. Supporting any SEP-41 asset and
synchronizing issuer controls expands that scope; record those decisions before
claiming support.

Keep the token interface independent of operator attestations and future FBA
verification. The announcement milestone should demonstrate a real lock, mint,
application transfer, burn and release, with visible pending/failed delivery.
Changing verification later may still require contract and balance migration.

See [the bridge implementation notes](sep41-bridge-handoff.md) for the review
findings, required integration tests and tracking issues.

## Work plan and acceptance

1. **Reference token (GH-414, implemented locally).** Add the interface and contract, local
   issuer controls, documentation, native runtime tests and RV32 artifact/spec
   build. Review the interface first, then the implementation. Cover negative and
   overflowing amounts, caller checks, initialization, inclusive expiry,
   replacement/revocation, delegated burns, frozen holders, role separation,
   rollback, emitted events and competing same-block allowance spends. The
   interface is included in `.github/workflows/publish.yml`, with its own tag and
   publication after its ERC20 event dependency. No release has been run.
2. **Downstream event compatibility (GH-414 follow-up).** Existing Groundhog
   movement compatibility and CLI consumption, including clawbacks, are covered
   locally. Finish registry dependency resolution and live-node ingestion. Then
   agree the Stellar-facing representation with data consumers. Add relative adjustment
   only with a clear caller and defined expiry/event semantics.
3. **Quickbridge token integration (GH-55, with GH-103).** Choose the first asset
   and wrapper burn policy, then prove the round trip with duplicate delivery,
   restart/retry, pending obligations and reconciliation. This depends on the
   reference token's usable interface, not the final federation design.
4. **Broader assets and issuer synchronization ([GH-429](https://linear.app/grndhg/issue/GH-429/define-broader-sep-41-asset-support-and-issuer-synchronization), following GH-53).** Define
   supported token policies, identity mapping, authorized sources, ordered
   freeze/unfreeze updates and behaviour while updates lag. Include collateral
   clawbacks and holder movement after deposit. Separate local controls from
   cross-chain synchronization in acceptance and announcement wording.

Keep detailed design here while it develops. Use GH-414 for the short current
scope and progress; extend existing bridge tracking when integration work starts.
Do not reopen completed bridge decisions or create a large issue tree now.

## Verification record

2026-09-24, based on sdk_rs `17cc9db4989c16139ff438b45c0f1b27a2ed8166`:

- Interface compile check passed with SDK 0.38.0 and protocol 0.39.0.
- 22 contract/runtime tests passed with native dispatch and again with the
  compiled RV32 token. Both use the local hogd test FFI 0.32.2 with storage
  charging disabled. This includes same-block commit conflicts, rollback and
  decoding movements through the existing Groundhog event types. CI discovers
  the native tests with the example workspace; an added step repeats them
  against the RV32 artifact built in its `examples-test` job.
- The RV32 build produced a 34,228-byte artifact. Spec extraction reported both
  interfaces (20 methods) and all eight events.
- Focused Clippy checks passed for the interface and example. The standalone
  RV32 interface carrier also compiled with both interface anchors.
- Workflow lint and the repository version checker passed. The interface has an
  explicit independent-version entry and follows the existing SDK patch-version
  compatibility policy.
- `cargo package` created the interface archive and compiled its extracted source
  against registry dependencies. An initial offline attempt needed a registry
  refresh; the final offline package verification passed. The release workflow's
  full feature-matrix dry run has not run, and nothing has been published.
- Independent design, implementation and whole-change reviews covered the token,
  event consumers and release integration. Review led to the shared-balance race
  test, release-workflow wiring and registration with the version checker. Those
  findings are addressed; the final version check passes.
- CI now pins the published CLI 0.19.0-pre.1. Both its registry package and
  release binary were verified. The macOS archive checksum is
  `c7cfcb034da60d73648c5981d06bc185adb4dee088dcb0c0ab7ce890bc4aabba`.
  The released binary rebuilt the reference token and passed all 22 RV32 tests.
  The SDK examples CI slice also passed, including structured spec-ID assertions,
  collision rejection and native example tests. No global CLI was replaced.
- CLI validation passed seven token decoder tests, eight token-history HTTP tests,
  sixteen raw-event HTTP tests and the test consuming all eight runtime-generated
  event types. The latter compares actual native/RV32 event bytes. All-target
  CLI Clippy passed with warnings denied. All 503 CLI library tests passed with
  four test threads. The first parallel run hit the existing process-probe
  one-second assertion; that test passed in isolation and in the rerun.
- The pre-PR compatibility pass widened symbols to 12 bytes and added native/RV32
  round-trip coverage for 11- and 12-character SAC metadata, plus rejection of a
  13-byte SDK symbol. Focused Clippy, formatting and package verification passed
  again. Admin control over minter replacement is now explicit in the interface
  and example. Bridge review findings are recorded in GH-55, GH-103 and GH-429,
  with details in [the handoff](sep41-bridge-handoff.md).

This is local contract/runtime evidence. A deployed network, charged-resource
behaviour, Stellar interoperability, live indexer ingestion and a bridge round
trip remain unverified. The reference contract is not deployed or released.

See [the example README](../example-contracts/sep41/README.md) for commands.
