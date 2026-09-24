# ERC20 issuer reference

A standard Groundhog ERC20 token with optional authorization, freeze, clawback
and holder burns. It uses the existing signed-grant interface for delegated
transactions. See the [spec](../../docs/sep41-reference-token.md) and
[bridge notes](../../docs/sep41-bridge-handoff.md).

Use `groundhog-erc20-if` for token operations, `groundhog-erc20-metadata-if` for
metadata and `groundhog-token-controls-if` for issuer operations. Deploy and
initialize together:

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

The caller becomes admin; the bridge becomes minter. The admin can replace the
minter. An issuer relay therefore needs restricted authority, rather than full
admin access. The wrapper must also decide how direct holder burns affect its
custody accounting. A token burn does not request a bridge withdrawal.

From `example-contracts/`:

```sh
cargo test -p erc20_issuer --locked
groundhog build -p erc20_issuer -- --locked -Zbuild-std=core,alloc
ISSUER_TOKEN_RV32_ARTIFACT="$PWD/../target/groundhog/erc20_issuer.rv32a" \
ERC20_PERMIT_RV32_ARTIFACT="$PWD/../target/groundhog/erc20_issuer.rv32a" \
  cargo test -p erc20_issuer --locked
```

The second environment variable selects the guest for the shared signed-wallet
suite. Explicit missing artifact paths fail. These tests use the hogd test
runtime with resource charges disabled, not a deployed network.

To export runtime events for downstream tests:

```sh
ISSUER_TOKEN_CLI_EVENTS_OUT=/tmp/issuer-events.json \
  cargo test -p erc20_issuer --locked export_cli_event_fixture -- --ignored
```

Set `ISSUER_TOKEN_RV32_ARTIFACT` too to export guest events. Forced burns produce
a standard burn plus clawback context; consumers must count the burn only.
