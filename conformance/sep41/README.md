# SEP-41 comparison tests

Compare `SEP41View` over the Groundhog issuer RV32 token with pinned Stellar
SAC. Each step checks balances, allowance, authorization, events and rollback.
This separate workspace keeps Soroban dependencies out of SDK/example builds.

Build the issuer example with the repository's pinned CLI. From the repo root:

```sh
ISSUER_TOKEN_RV32_ARTIFACT="$PWD/target/groundhog/erc20_issuer.rv32a" \
  cargo test --manifest-path conformance/sep41/Cargo.toml --locked
```

The runtime test uses testkit 0.20.0, native signing wallets and an RV32 actor
calling `SEP41View`. It checks funded initialization, approval/expiry,
authorization, collateral rollback and indexed events against receipts.
It owns a disposable node and produces blocks manually. Use CLI 0.20.0 to
build the actor, then run:

```sh
(cd conformance/sep41/actor && groundhog build -p sep41-runtime-actor -- --locked -Zbuild-std=core,alloc)
GROUNDHOG_DEVNODE_BINARY=/path/to/hog-devnode-0.1.29-rc.4 \
ISSUER_TOKEN_RV32_ARTIFACT="$PWD/target/groundhog/erc20_issuer.rv32a" \
SEP41_ACTOR_RV32_ARTIFACT="$PWD/conformance/sep41/actor/target/groundhog/sep41_runtime_actor.rv32a" \
  cargo test --manifest-path conformance/sep41/Cargo.toml --features runtime --test runtime --locked
```

Artifact paths and the node binary are required; there is no fallback.
CI builds both artifacts and runs both suites.

Intentional differences:

- Native reset/change events represent one finite approval. Zero approval
  deletes the grant without a deadline, so compare revocation only.
- Native clawback emits burn plus context; check both, compare one destruction.
- Native supply is capped at `i128::MAX` in aggregate; SAC caps each contract
  holder. Metadata and admin/minter roles differ between the implementations.

Host tests mock authorization and disable Groundhog resource charges. Runtime
calls use real signatures; native wallets wrap child aborts, so diagnostic
views check the child reason while receipts and state checks establish rollback.
Classic trustlines, same-block concurrency and bridge backing/recovery are
outside this suite.
