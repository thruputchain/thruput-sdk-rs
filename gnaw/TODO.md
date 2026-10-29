# gnaw TODO

Follow-up work, roughly in priority order.

## CI coverage

- Add `gnaw/Cargo.lock` to the `dependency-policy` job (`cargo deny` with
  `--manifest-path gnaw/Cargo.toml`).

- Update the `integration_test/contracts` submodule's comments and README,
  which still mention `scripts/dev.sh`.

## Native ports

Replace the scripts that tasks still run:

- `check-storage-registry-stripped.sh` and `check-domain-collision-detected.sh`
  (the first uses `strings`; `elf.rs` can do it).
- `check-exception-stack.sh`, `check-docsrs.sh`, `cargo-example-smoke.sh`.
- The discovery in `scripts/metrics/sdk_codesizes_rv32*.sh`, which copies
  `repo::rv32_packages`.

Share package discovery between tasks instead of scanning per task.

## Commands

- Make `cargo gnaw` work from subdirectories. The alias's manifest path is
  relative to the current directory.
- `gnaw test --rv32`: set the artifact variables from `RV32_ARTIFACT_TESTS`,
  as `fixtures:test` does.

## Speed

- Parallel RV32 builds, with `thruput build --target-dir` and a target
  directory per shard. The builds are already one step per package.
- Caching experiments. A Namespace-cached `target/` is I/O-bound (see
  `.github/actions/gnaw-setup`). Try `sccache`, or caching only the build-std
  dependencies under `target/riscv32im-*/release/deps`.
- Make the build-std flags the same everywhere, so builds can share
  artifacts.

## Fixtures

- Replace `disabled` marker files with package metadata
  (`[package.metadata.<name>]`) that says what kind of fixture a package is:
  native only, rv32, negative, or tested against its artifact. Generate
  `RV32_ARTIFACT_TESTS` in `src/tasks.rs` from it.
- Rename `example-contracts/` to `fixtures/` and `negative-contracts/` to
  `fixtures-negative/`, in a separate change. gnaw keeps the paths in
  `src/repo.rs`; several scripts and many comments still hard-code them.

## Metrics

- Replace `scripts/metrics.sh` with a `gnaw metrics` subcommand.
