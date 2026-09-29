# SEP-41 issuer reference

A separate token profile with absolute, expiring allowances. Balance accounting
and issuer policy come from the same source as `erc20_issuer`; each deployment
has its own state. Native ERC20 and permit spending methods are not exposed.

Deploy and initialize atomically through `ERC20AdminClient::initialize`.
`IssuerClient` supplies the example's mint and total-supply methods. See the
[Stellar integration notes](../../docs/sep41-reference-token.md) for differences
from Soroban.

Run from `example-contracts/`:

```sh
cargo test -p sep41_issuer --locked
groundhog build -p sep41_issuer -- --locked -Zbuild-std=core,alloc
SEP41_TOKEN_RV32_ARTIFACT="$PWD/../target/groundhog/sep41_issuer.rv32a" \
  cargo test -p sep41_issuer --locked
```

Tests disable resource charges. Stellar comparisons and funded wallet/node
validation are separate follow-up work.
