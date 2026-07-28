# version-check

Checks that version pins agree across every manifest and lockfile in this
repository.

## Why

Most crates here release in lockstep at one version, but that version is
transcribed into roughly thirty places spread over six workspaces and six
lockfiles:

| Workspace | Lockfile | In root workspace? |
| --- | --- | --- |
| `Cargo.toml` | `Cargo.lock` | — |
| `example-contracts/` | own | no |
| `example-contracts/cross_crate_collision/` | own | no |
| `example-contracts/multi_root_collision/` | own | no |
| `integration_test/` | own | no |
| `logging-test/` | own | no |

`cargo` will eventually catch a missed pin, but only one at a time, only in the
workspace it happens to be resolving, and only once you get as far as running
`cargo update` or `cargo publish` there. This crate reports every disagreement
in the repository in a single pass, before any of that.

It is a *checker*, not a bumper: it never edits anything. See
[Toward a bumper](#toward-a-bumper).

## Running it

```bash
cargo test -p version-check       # how CI runs it
cargo run -p version-check        # same checks, readable report, exit 1 on failure
cargo run -p version-check -- /path/to/other/checkout
```

The `#[test]` in `src/lib.rs` means the plain `cargo test` in `scripts/dev.sh`
already covers this crate — it is in the root `default-members`, so no extra CI
wiring exists or is needed.

## The core invariant

There is deliberately no notion of a "version stream" anywhere in this crate.
For every dependency edge naming an in-repo package, the requirement must be
satisfied by **that package's own declared version**, read from its manifest.

## Checks

Each violation is tagged with its id. All checks run on every invocation and
none short-circuits, so one run tells you everything that needs fixing.

| Id | Enforces | Rationale |
| --- | --- | --- |
| `in-repo-req` | A pin on an in-repo package is satisfied by that package's declared version. | The main check. Catches the pin you forgot to bump. |
| `pin-operator` | In-repo pins use a single `=` requirement, unless excepted. | The crates release together, so they pin each other exactly. (A caret where `=` was meant is invisible to a `grep` for the old version string.) |
| `path-target` | Every `path` dependency resolves to a manifest declaring the expected package name. | Catches moved, renamed, and typo'd paths. |
| `in-repo-source` | A dependency naming an in-repo package reaches it by `path`. | Without a `path` it silently resolves from the registry, so local changes are ignored. |
| `own-version` | Root-workspace members use `version.workspace = true`, unless excepted. | A new crate with a hardcoded version would drift out of lockstep on the next release. |
| `lockfile` | Every lockfile agrees with the manifests on in-repo versions, and locks in-repo packages from their path rather than a registry source. | The six lockfiles are the easiest thing to forget; the registry-source case is a `grep`-proof failure. |
| `default-members` | Non-target-only `crates/*` members appear in `default-members`. | Host-wide `cargo build`/`test`/`doc` build only `default-members`, so an omitted crate is silently untested. |
| `external-agreement` | Every manifest pinning the same external crate pins it identically. | Probably what we want. (Compares version requirements only, not features.) |

## Exceptions

Deliberate deviations live in `src/policy.rs`, and every entry carries a
`reason` — an exception cannot be added without writing down why it is correct.
The current set:

- `groundhog-erc20-if`, `groundhog-erc20-metadata-if`, `groundhog-erc1271-if`
  caret-depend on `sdk` rather than pinning it, because the published interface
  crates are compatible with any SDK patch release.
- Those three plus `bottle-test-if` and `version-mismatch-helper` declare their
  own versions instead of inheriting the workspace version.
- `groundhog-rv32-runtime` and `groundhog-wasm-runtime` are absent from
  `default-members`, because a top-level `compile_error!` rejects the host
  target.
- `hex-literal` and `thiserror` pin sites under `integration_test/contracts/`
  are excluded from `external-agreement`, pending a PR to the
  `integration_test_contracts` submodule. Both are temporary: the contracts
  should inherit these from the `integration_test` workspace rather than pinning
  them inline, which would remove the pin sites entirely.

**Exceptions are checked in both directions.** An exception that matches no pin
site, or whose site has since come into line with the rest of the repo, is
itself reported as stale — so the list cannot quietly rot after the work it was
waiting on lands.

## Anticipated checks

Not yet implemented, in rough order of value:

- **`publish-coverage`** — every publishable in-repo crate appears in the
  `PUBLISH_PACKAGES` list of `.github/workflows/publish.yml` or
  `publish_standard_interfaces.yml`. A new published crate missing from the
  release workflow is the same class of error as a missed pin, and is currently
  caught only by noticing it never shipped.
- **`missing-pin`** — an in-repo dependency of a *published* crate that carries
  no `version` at all. `cargo publish` rejects this, so the check would only
  move the failure earlier; the exceptions (path-only dev-dependencies, which
  cargo strips at package time, and `publish = false` crates) are what make it
  fiddly enough to defer.
- **`ffi-version`** — `crates/hogd-sys/build.rs` carries a `TEST_FFI_VERSION`
  constant on its own independent version line. Worth at least parsing and
  sanity-checking, though what it should be checked *against* is unclear.
- **`feature-agreement`** — extend `external-agreement` to flag manifests that
  pin the same external crate at the same version but with divergent
  `features`/`default-features`. Likely noisy; needs a look at how much real
  divergence exists first.

## Deliberately not checked

- **Lockfile regenerability.** Shelling out to `cargo update --locked` per
  workspace would need registry access and would make the check slow enough to
  skip. Whether a lockfile can be regenerated is the bumper's problem;
  whether it *agrees* is check `lockfile`'s.
- **Whether a new version number is semantically right.** Nothing here knows
  that a change warranted `0.18.0` rather than `0.17.5`. This crate only
  enforces internal consistency.
- **`workspace = true` naming a key absent from the workspace table.** `cargo`
  already errors on this at manifest load, well before this check would run.

## Other version-check TODOs

- **Fixture-based unit tests** — the checks are currently validated by
  perturbing the real repository and confirming each one fires. Synthetic
  fixture repositories under `tests/` would be better.

## Toward a bumper

The discovery and policy layers are the reusable half of a bump tool. A bumper
would reuse `model::Repo` unchanged, replace `checks` with `toml_edit` writes at
the same pin sites — `toml_edit` preserves the comments and ordering that the
root manifest depends on heavily — and then run `cargo update --workspace` once
per lockfile. The check list above is exactly its postcondition, so nothing
built here is throwaway.

## Toward an xtask

`src/main.rs` is a thin wrapper over `lib::check` and `lib::report`, so this can
become a subcommand of a repo-wide `xtask` without restructuring: move the
argument parsing up a level and call `version_check::check(root)`. That is the
intended direction for the `scripts/*.sh` checks generally.
