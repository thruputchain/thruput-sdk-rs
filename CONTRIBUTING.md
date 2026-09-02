# Contributing

Thanks for looking. This is the Rust SDK for writing Groundhog contracts;
the developer-facing documentation lives in `crates/sdk/doc/` (rendered into
the crate's rustdoc) and the repository-level setup is in `README.md`.

Please read [SECURITY.md](SECURITY.md) before opening an issue for anything
that looks exploitable — do not file a public issue for a vulnerability.

## Before you start: what CI needs

CI for this repository **cannot pass from a fork today.** Every job needs
credentials that live in this repository's secrets, and secrets are not
exposed to pull requests from forks:

- `GROUNDHOG_REGISTRY_TOKEN` — the private `groundhog` cargo registry, where
  every `groundhog-*` dependency and the `groundhog` CLI come from
  (`.github/workflows/ci.yml:12-13`, `.cargo/config.toml`).
- `HOGD_FFI_PASSWORD` — the prebuilt hogd test-FFI library, downloaded by
  `crates/hogd-sys/build.rs`, which every native test links
  (`.github/workflows/ci.yml:54`).
- SSH deploy keys, for the private `integration_test/contracts` submodule and
  for git dependencies (`.github/workflows/ci.yml:70-80`).
- Self-hosted / Namespace runners (`namespace-profile-hogd-linux-2604`,
  `hog1`).

So an outside contributor cannot build or test this repository as it stands.
That is a known gap in the public preview, tracked in
[groundhogchain/packaging#86](https://github.com/groundhogchain/packaging/issues/86);
until it is closed, please open an issue describing the change rather than a
pull request you cannot verify. Small documentation fixes are still welcome
as pull requests — say in the PR that you could not run the checks, and a
maintainer will run them.

## Toolchain

`rustup` is the only local prerequisite. `rust-toolchain.toml` pins
`nightly-2026-07-01` together with the `riscv32im-unknown-none-elf` target
and the `rust-src` component, and rustup installs them on the first `cargo`
invocation in the checkout. Do not install the target into the floating
`nightly` — that is a different toolchain.

Contract (rv32) builds need that pinned nightly (`-Zbuild-std`). Building the
hogd FFI from source additionally needs cmake, clang and ccache; the default
path downloads a prebuilt library instead. See `README.md` for both.

`scripts/dev.sh` drives rv32 contract builds through the `groundhog` CLI,
which CI installs at the version pinned in `scripts/groundhog-cli-version.sh`.

## The checks

`scripts/dev.sh` is the entry point for everything (`alias dev=./scripts/dev.sh`):

| command | what it does |
|---|---|
| `dev ci` | everything `check_all` covers, then the quality checks — what CI runs, via `scripts/ci.sh` |
| `dev qual` | `check_quality`: clippy (`cargo clippy -F dev --lib` plus every contract), `cargo fmt --check`, and the `XXX` scan |
| `dev lint` | the clippy half on its own; `dev lint <dir>/<pkg>` for one contract |
| `dev test` | host tests for a package |
| `dev build-examples-rv32` | build every `example-contracts` package for rv32 |
| `dev expand <dir>/<pkg>` | expand the macros for one contract |
| `dev open-docs` | build and open the SDK documentation |

Every block of `check_all` can be skipped with `SKIP_<BLOCK>=1` (see
`check_all` in `scripts/dev.sh`) — `SKIP_INTEGRATION=1` is the one you want
without the private submodule.

Before pushing, at minimum:

```shell
cargo fmt --all -- --check
cargo clippy -p sdk -F dev --all-targets -- -D warnings
cargo test -p sdk
RUSTDOCFLAGS="--deny warnings --cfg docsrs" cargo doc --features=dev
```

Two things that trip people up:

- **`cargo check -p sdk` with default features cannot work.** A native build
  without `std` is a deliberate `compile_error!` (`crates/sdk/src/lib.rs`).
  Use `-F std` or `-F dev`.
- **`cargo check --workspace` cannot work either.** `groundhog-rv32-runtime`
  `compile_error!`s off its target, which is why it is excluded from
  `default-members`. Plain `cargo check` is the workspace-wide check.

An `XXX` anywhere in the tree fails `dev qual` (`xxx_check`). `TODO` does
not, but prefer an issue over a TODO in shipped source.

## Pull requests

- Keep the diff to one subject. Do not reformat unrelated code, and do not
  bump unrelated dependencies.
- Say in the PR what you ran, and be exact about what you did *not* run.
- Versions and lockfiles are moved by `cargo run -p version-check -- bump ...`
  and verified by `cargo run -p version-check`; do not hand-edit pins. A
  release also updates `CHANGELOG.md`.
- Contract-visible behaviour changes need an rv32 build, not only host tests:
  the host suite never compiles for `riscv32im-unknown-none-elf`.
