# version-check

Checks that version pins agree across every manifest and lockfile in this
repository.  Also automates version-bumping.

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

It also applies coordinated version bumps — see [Bumping](#bumping). The check
and the bump share one discovery layer, so the check list is exactly the bump's
postcondition.

## Running it

```bash
cargo run -p version-check        # same checks, readable report, exit 1 on failure
cargo run -p version-check -- /path/to/other/checkout
cargo run -p version-check -- --ignore-lockfiles
cargo run -p version-check -- --help
```

This check is run as part of `scripts/ci.sh`.

`--ignore-lockfiles` skips reading the six lockfiles, so the `lockfile` check has
nothing to inspect and every other check runs unchanged. It is for the middle of
a bump: once the manifests are edited but before `cargo update` has been run
against each workspace, the lockfiles are *expected* to disagree, and
regenerating them needs registry access that the manifest checks do not. Use it
to confirm the pins are right first, then regenerate, then run without the flag.

Like the submodule handling, the narrowed scope is stated rather than implied:

```
version-check: 118 manifests, lockfiles ignored (--ignore-lockfiles)
```

`scripts/dev.sh` runs the checker without the flag, and does so *after* the
lockfiles have been updated, so CI always runs the full check.

## Scope: submodules are scanned when checked out

Paths listed in `.gitmodules` — `integration_test/contracts` today — are scanned
if they are populated and skipped if they are not. A submodule is treated as
absent when its directory is missing, empty, or holds nothing but its own `.git`
gitlink.

This matters because **`ci.yml` initializes submodules and `sdk_tests.yml` does
not**, so coverage genuinely differs between workflows:

```
version-check: 118 manifests, 6 lockfiles      # local, ci.yml
  submodules scanned: integration_test/contracts
all pins agree

version-check: 83 manifests, 6 lockfiles       # sdk_tests
  submodules not checked out, so not scanned: integration_test/contracts
all pins agree
```

Varying coverage is safe only because it is *stated*, never silent — the counts
and the submodule disposition are printed on every run, pass or fail, so reduced
coverage cannot read as "everything is fine".

### Absent is not the same as resolved

One subtlety makes this work. An exception scoped inside a submodule matches no
pin site when that submodule is absent — which looks exactly like an exception
whose underlying problem has been fixed. Reporting it as stale is wrong, and it
is precisely how the first version of this crate failed: green locally, red in
`sdk_tests`, for a reason that had nothing to do with any pin.

So `Repo::is_unscanned` distinguishes the two, and the stale-exception check
stays silent about scopes that were never scanned. The check still fires
normally when the submodule *is* present, so an exception that has genuinely
outlived its purpose is still caught — just only in the runs that can see it.

Nothing in the checks is specific to this repository, so a submodule's own
repository can also run the checker on itself.

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
| `publish-order` | Every in-repo dependency is vendored and published before its dependent in `.github/workflows/publish.yml`. | `cargo package` resolves dev-dependencies too, so a list out of order fails the release itself. The workflow is dispatched by hand, so the first run to exercise a new edge is usually a real release. |

## Exceptions

Deliberate deviations live in `src/policy.rs`, and every entry carries a
`reason` — an exception cannot be added without writing down why it is correct.
The current set:

- Manifests under `standard-interfaces/` and `standard-impls/` caret-depend on
  `sdk` rather than pinning it, because the published interface and
  implementation-component crates are compatible with any SDK patch release.
- The standard-interface crates (`groundhog-sep41-if`, `groundhog-erc20-if`,
  `groundhog-erc20-metadata-if`,
  `groundhog-erc20-pausable-if`, `groundhog-access-control-if`, `groundhog-erc165-if`,
  `groundhog-erc721-if`,
  `groundhog-signed-op-if`, `groundhog-erc20-permit-if`) and the
  `groundhog-access-control` and `groundhog-signed-op` components, plus
  `bottle-test-if` and
  `version-mismatch-helper`, declare their own versions instead of inheriting
  the workspace version.
- `groundhog-rv32-runtime` is absent from `default-members`, because a
  top-level `compile_error!` rejects the host target.
- `hex-literal` and `thiserror` pin sites under `integration_test/contracts/`
  are excluded from `external-agreement`, pending a PR to the
  `integration_test_contracts` submodule. Both are temporary: the contracts
  should inherit these from the `integration_test` workspace rather than pinning
  them inline, which would remove the pin sites entirely. They only apply when
  that submodule is checked out — see
  [Absent is not the same as resolved](#absent-is-not-the-same-as-resolved).

Agreement exceptions are scoped to a manifest prefix rather than excusing a crate
repo-wide, so the excused *site* drops out of the comparison while the crate
stays checked everywhere else — a blanket exception on something as widely pinned
as `thiserror` would hide every future divergence along with the one being
excused.

**Exceptions are checked in both directions.** An exception that matches no pin
site, or whose site has since come into line with the rest of the repo, is
itself reported as stale — so the list cannot quietly rot after the work it was
waiting on lands.

## Possible additional checks

Not yet implemented, in rough order of value:

- **`publish-coverage`** — every publishable in-repo crate appears in the
  `PRIMARY_PACKAGES` or `INTERFACE_PACKAGES` list of
  `.github/workflows/publish.yml`. A new published crate missing from the
  release workflow is the same class of error as a missed pin, and is currently
  caught only by noticing it never shipped. `publish-order` already reads those
  lists, so this needs the coverage rule rather than any new parsing.
- **`ffi-version`** — `crates/hogd-sys/build.rs` carries a `TEST_FFI_VERSION`
  constant on its own independent version line. Worth at least parsing and
  sanity-checking, though what it should be checked *against* is unclear.
- **`missing-pin`** — an in-repo dependency of a *published* crate that carries
  no `version` at all. `cargo publish` rejects this, so the check would only
  move the failure earlier; the exceptions (path-only dev-dependencies, which
  cargo strips at package time, and `publish = false` crates) are what make it
  fiddly enough to defer.
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

## Bumping

```bash
cargo run -p version-check -- bump patch                # patch-bump the SDK
cargo run -p version-check -- bump breaking             # semver-incompatible SDK bump
cargo run -p version-check -- bump 1.0.0                # an explicit target
cargo run -p version-check -- bump --protocol 0.21.0    # move protocol-rs
```

The `standard-interfaces` are deliberately untouched by a patch bump. Their
`sdk` pin is a caret, so `^0.17.4` still contains `0.17.5` and nothing needs to
change — the pins are *supposed* to drift between breaking releases, and a
breaking bump refreshes them to the current workspace version, absorbing
however much drift accumulated.

`--protocol` implies an SDK bump, patch unless the spec asks for more, because a
breaking protocol-rs change forces a new interfaces release so clients can
request a version built against it. The `xdrs` / `xdr-common` crates are a
separate train and are left alone.

### How it decides what to edit

There is no hardcoded file list. The two crate families are told apart by how
each package declares its own version: `VersionSpec::Inherited` is the
workspace family, a literal version under `standard-interfaces/` is the
independent interface line. That predicate must be path-scoped rather than "has
a literal version" — `bottle-test-if` (0.1.0) and `version-mismatch-helper`
(1.0.0) also carry literal versions and must never move.

Requirements keep their operator: only the version part is rewritten, so `=`
stays `=` and a caret stays a caret. Multi-part requirements (`>=1, <2`) are
refused rather than guessed at.

Writes go through `toml_edit`, preserving comments and formatting.

### Safety

1. No uncommitted changes to **tracked** files, unless `--allow-dirty`. `git
   diff` is the review surface and git is the undo.

   Untracked files are fine — nothing here can clobber a file git has never
   seen, and refusing to run over a stray scratch file is just noise. The one
   exception is an untracked file the bump would *edit* (a brand-new
   `standard-interfaces/` crate, say): tolerating untracked files rests on `git
   checkout` being able to undo the bump, which is not true of a manifest git
   does not know about. Those are named and refused separately, after the plan
   exists and so with the offending paths in hand.
2. Every check runs **before** anything is written. Bumping a repository that
   already disagrees with itself would bury the original problem under ~25
   unrelated edits.
3. After the manifests are rewritten, the checks run again with
   `--ignore-lockfiles`, since the lockfiles have not caught up yet.
4. `cargo update --workspace` runs against each of the six workspace roots —
   themselves discovered, not listed. `--skip-lockfiles` stops before this.
5. Every check runs once more as the postcondition.

Deliberately not automated:

- choosing whether a change is breaking
- `TEST_FFI_VERSION`
- the publish workflows
- anything inside a submodule

## Toward an xtask

`src/main.rs` is a thin wrapper over the library, so both subcommands can move
under a repo-wide `xtask` without restructuring: lift the argument parsing and
call `version_check::checks::all` / `version_check::bump::plan` directly. That is
the intended direction for the `scripts/*.sh` checks generally — at which point
the crate's name, which now covers a bumper too, is worth revisiting.
