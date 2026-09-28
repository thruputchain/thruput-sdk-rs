# ERC20 issuer reference

Groundhog ERC20 with optional issuer controls in `groundhog-erc20-controls-if`.
Deploy and initialize atomically with `ERC20AdminClient::initialize`: its caller
becomes admin, and the supplied address becomes minter. See the
[Stellar integration notes](../../docs/sep41-reference-token.md) for policy and
bridge boundaries.

Run from `example-contracts/`:

```sh
cargo test -p erc20_issuer --locked
groundhog build -p erc20_issuer -- --locked -Zbuild-std=core,alloc
ISSUER_TOKEN_RV32_ARTIFACT="$PWD/../target/groundhog/erc20_issuer.rv32a" \
ERC20_PERMIT_RV32_ARTIFACT="$PWD/../target/groundhog/erc20_issuer.rv32a" \
  cargo test -p erc20_issuer --locked
```

Both artifact variables are needed: the policy and shared permit suites select
their guests separately. Tests disable resource charges; wallets run natively.

To export events for CLI tests, set `ISSUER_TOKEN_CLI_EVENTS_OUT=/tmp/events.json`
and run `cargo test -p erc20_issuer --locked export_cli_event_fixture -- --ignored`.
Add `ISSUER_TOKEN_RV32_ARTIFACT` to export guest events.
