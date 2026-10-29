# gnaw TODO

Follow-up work after the first version, which runs CI by calling the existing
scripts. Roughly in priority order.

## CI coverage

- Add `gnaw/Cargo.lock` to the `dependency-policy` job (`cargo deny` with
  `--manifest-path gnaw/Cargo.toml`).

## Replace dev.sh

- Port the development commands: `test <path>`, `test-examples`, `test-itc`,
  `lint`, `build-docs`, `expand`, `bigint-bench`, `bigint-bench-test`. Add
  direct subcommands (such as `gnaw test example-contracts/counter`) next to
  `run`, which runs the task graph.
- Delete `dev.sh` and update CONTRIBUTING.md and AGENTS.md.

## Native ports

Replace the shell-outs that tasks still make, in this order:

1. RV32 builds (`dev.sh build-examples-rv32`, `build-integration-rv32`), as
   one task per package. Failure paths name the package, and builds can run
   in parallel.
2. ELF and artifact checks, using the `object` crate instead of `readelf`,
   `nm` and `rg`.
3. Negative fixtures, as one task driven by a table of fixtures and expected
   errors.
4. `check-docsrs.sh`, `cargo-example-smoke.sh`, `check-exception-stack.sh`.

Share package discovery between tasks instead of scanning per task.

## Speed

- Parallel RV32 builds, with `thruput build --target-dir` and a target
  directory per shard.
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
  `fixtures-negative/`, in a separate change once scripts no longer hard-code
  the paths.

## Metrics

- Replace `scripts/metrics.sh` with a `gnaw metrics` subcommand.
