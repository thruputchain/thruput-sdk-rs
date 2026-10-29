# gnaw TODO

Follow-up work, roughly in priority order.

## CI coverage

- Add `gnaw/Cargo.lock` to the `dependency-policy` job (`cargo deny` with
  `--manifest-path gnaw/Cargo.toml`).

## Native ports

Replace the scripts that tasks still run:

- `check-storage-registry-stripped.sh` and `check-domain-collision-detected.sh`
  (the first uses `strings`; `elf.rs` can do it).
- `check-exception-stack.sh`, `check-docsrs.sh`, `cargo-example-smoke.sh`.

Share package discovery between tasks instead of scanning per task.

## Commands

- Make `cargo gnaw` work from subdirectories. The alias's manifest path is
  relative to the current directory.
- `gnaw test --rv32`: set each package's `rv32-artifact-vars`, as
  `fixtures:test` does.

## Speed

- Parallel RV32 builds, with `thruput build --target-dir` and a target
  directory per shard. The builds are already one step per package.
- Caching experiments. A Namespace-cached `target/` is I/O-bound (see
  `.github/actions/gnaw-setup`). Try `sccache`, or caching only the build-std
  dependencies under `target/riscv32im-*/release/deps`.
- Make the build-std flags the same everywhere, so builds can share
  artifacts.

## Fixtures

- Fix or delete the packages of the `disabled` kind. None says why it was
  set aside.
- Rename `example-contracts/` to `fixtures/` and `negative-contracts/` to
  `fixtures-negative/`, in a separate change. gnaw keeps the paths in
  `src/repo.rs`; several scripts and many comments still hard-code them.

## Metrics

- Replace `scripts/metrics.sh` with a `gnaw metrics` subcommand.
