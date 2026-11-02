# ERC20 issuer reference

Thruput ERC20 with optional issuer controls in `thruput-erc20-controls-if`.
Deploy and initialize atomically with `ERC20AdminClient::initialize`: its caller
becomes admin, and the supplied address becomes minter. See the
[Stellar integration notes](stellar-integration.md) for policy and
bridge boundaries. SEP-41 callers use `SEP41View` over this token.

Run from `fixtures/`:

```sh
cargo test -p erc20_issuer --locked
thruput build -p erc20_issuer -- --locked -Zbuild-std=core,alloc
ISSUER_TOKEN_RV32_ARTIFACT="$PWD/../target/thruput/erc20_issuer.rv32a" \
ERC20_PERMIT_RV32_ARTIFACT="$PWD/../target/thruput/erc20_issuer.rv32a" \
  cargo test -p erc20_issuer --locked
```

Both artifact variables are needed: the policy and shared permit suites select
their guests separately. Tests disable resource charges; wallets run natively.

To export events for CLI tests, set `ISSUER_TOKEN_CLI_EVENTS_OUT=/tmp/events.json`
and run `cargo test -p erc20_issuer --locked export_cli_event_fixture -- --ignored`.
Add `ISSUER_TOKEN_RV32_ARTIFACT` to export guest events.
