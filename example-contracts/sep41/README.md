# Reference token

A Groundhog adaptation of SEP-41 with expiring allowances, holder burns,
issuer authorization and clawback. Admin and minter are separate roles.
The [spec and work plan](../../docs/sep41-reference-token.md) describe behaviour,
Groundhog differences and remaining integration work.

This example uses Groundhog addresses, calls and events. It does not implement
Soroban's ABI or cross-chain synchronization. `live_until_ledger` refers to the
Groundhog execution height, not Stellar's ledger number.

Transfers, mints and burns use the existing Groundhog token events. Approval
carries an absolute grant and expiry; clawback has its own event. The current
released CLI token history recognizes transfers, mints and burns. The companion
CLI change adds clawbacks; see the spec for validation and release order.

## Build and test

From `example-contracts/`:

```sh
cargo test -p sep41 --locked
cargo clippy -p sep41 --all-targets --features dev --locked

groundhog build -p sep41 -- --locked -Zbuild-std=core,alloc
SEP41_RV32_ARTIFACT="$PWD/../target/groundhog/sep41.rv32a" \
  cargo test -p sep41 --locked
```

Use a CLI that supports the SDK's raw contract spec format (v3). If the installed
CLI reports a format mismatch, use a matching CLI build by its full path. On
macOS, a pending Xcode license can be avoided with
`DEVELOPER_DIR=/Library/Developer/CommandLineTools` when those tools are installed.

The normal test run uses native contract dispatch backed by the hogd test runtime.
Setting `SEP41_RV32_ARTIFACT` runs the same tests against the compiled guest;
a missing or invalid explicit artifact fails rather than selecting native dispatch.
The tests disable resource charges and do not connect to a live network.
CI's example job runs the native suite and repeats it against the built RV32
artifact.

## Use from another contract

Depend on `groundhog-sep41-if` and use `TokenClient` for token operations and
`TokenAdminClient` for issuer operations. Deploy and initialize in one transaction:

```rust,ignore
let address = sdk::deploy_contract(&nonce, &code_hash);
TokenAdminClient::new(&address).initialize(
    &bridge_address,
    "Example Token".try_into().unwrap(),
    "EXAMPLE".try_into().unwrap(),
    7,
    Policy {
        auth_required: false,
        auth_revocable: false,
        clawback_enabled: false,
    },
);
```

The deploying caller becomes admin. The named bridge becomes minter, with no
permission to freeze holders, claw back balances or change either role. The admin
can replace the minter, including with itself. A relay that only synchronizes
issuer controls needs a restricted administrative contract, or it also gains
control over minting. For issuer-controlled assets, choose the policy at
initialization and authorize holders before minting or transferring when required.

An owner calls `approve` or `transfer` from its wallet contract; its `from`
argument must equal that caller. A spender calls `transfer_from` or `burn_from`
with its own address as `spender`. Grants are absolute and expire after the named
Groundhog block. Same-pair allowance changes in separate transactions contend;
retry a commit conflict in a later block. Freeze and role changes are visible to
other transactions in the next block.

Quickbridge must pair its withdrawal burn with a withdrawal record atomically.
A direct holder burn creates no withdrawal claim. Wrapper policy and accounting
for direct burns and clawbacks must be decided before this reference is used for
backed assets. See the spec's Quickbridge section.

## Export events for CLI tests

The ignored exporter writes actual runtime events for the CLI's HTTP tests.
From `example-contracts/`, after building the token:

```sh
SEP41_RV32_ARTIFACT="$PWD/../target/groundhog/sep41.rv32a" \
SEP41_CLI_EVENTS_OUT=/tmp/sep41-events.json \
  cargo test -p sep41 --locked export_cli_event_fixture -- --ignored
```

Omit `SEP41_RV32_ARTIFACT` to export through native dispatch. Both modes produce
the same event bytes. The file contains contract events only; it does not claim
real transaction hashes, ledger heights or a deployed network.
