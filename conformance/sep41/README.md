# SEP-41 comparison tests

[GH-441](https://linear.app/grndhg/issue/GH-441) runs matching traces through
`SEP41View` over the real Groundhog issuer RV32 token and Stellar's Asset
Contract in Soroban SDK 28.0.0 / host 28.0.2. This separate workspace keeps
Soroban dependencies out of SDK and example builds.

Each step checks balances, allowance, holder authorization and event fields,
including rollback after rejected calls. Traces cover approval replacement,
partial spending, inclusive expiry, revocation, signed amount bounds,
transfers, burns, freeze/unfreeze and clawback. SAC auth mocks match the exact
actor, function and arguments; Groundhog uses a caller-checked host wallet.
Commit and advance both hosts between steps. Same-block concurrency stays in
the native token tests.

The comparison preserves intentional differences:

- Native reset/change events become one finite approval effect. Deletion has
  no deadline; zero approval compares revocation only.
- Native clawback emits burn plus context. Check both; compare one destruction.
- Native aggregate supply is capped at `i128::MAX`; SAC caps each contract
  holder separately. Metadata is configured on Groundhog and asset-derived
  on SAC. Admin/minter handover is not shared behavior.

Build the issuer example with the repository's pinned CLI, then run:

```sh
ISSUER_TOKEN_RV32_ARTIFACT="$PWD/target/groundhog/erc20_issuer.rv32a" \
  cargo test --manifest-path conformance/sep41/Cargo.toml --locked
```

The artifact is required; CI uses its existing example build. Host comparisons
disable Groundhog resource charges. Classic trustlines, bridge backing and
recovery are outside this suite. Boundary cases also draw on
[Leigh's example](https://github.com/groundhogchain/groundhog-examples/pull/116).

The funded runtime test uses published testkit 0.20.0 with separate native
signing wallets and a small RV32 actor that calls `SEP41View`. It checks
atomic funded initialization, approval replacement/expiry/revocation,
authorization and parent/child rollback, storage collateral, and receipts
against indexed event bytes and identities. It owns a disposable node and
manually produces blocks so the expiry boundary is exact. The actor has its
own workspace; build it without host/Soroban features:

```sh
(cd conformance/sep41/actor && groundhog build -p sep41-runtime-actor -- --locked -Zbuild-std=core,alloc)
GROUNDHOG_DEVNODE_BINARY=/path/to/hog-devnode-0.1.29-rc.4 \
ISSUER_TOKEN_RV32_ARTIFACT="$PWD/target/groundhog/erc20_issuer.rv32a" \
SEP41_ACTOR_RV32_ARTIFACT="$PWD/conformance/sep41/actor/target/groundhog/sep41_runtime_actor.rv32a" \
  cargo test --manifest-path conformance/sep41/Cargo.toml --features runtime --test runtime --locked
```

All three inputs are required. The runtime feature leaves ordinary host tests
independent of a node. This proves signed Groundhog execution; the SAC oracle
still runs in a Soroban host with exact authorization mocks.
The native wallet wraps child aborts; matching diagnostic views decode the
child reason while signed receipts and state checks prove rejection/rollback.
