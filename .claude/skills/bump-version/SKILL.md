---
name: bump-version
description: Use when bumping crate versions, releasing, or updating dependency pins in this workspace
---

# Bump Version

Bump the workspace and update all pinned dependency references across every
workspace in the repo and its lockfile. The tool discovers those by walking the
tree, so a workspace added later is picked up without touching any list.

## This is automated — use the tool

`crates/version-check` both applies the bump and verifies it. Prefer it over the
manual procedure below, which is now reference material for when something
unusual comes up.

```bash
cargo run -p version-check -- bump patch              # patch-bump the SDK
cargo run -p version-check -- bump breaking           # semver-incompatible SDK bump
cargo run -p version-check -- bump --protocol 0.21.0  # move protocol-rs
```

## Repairing a workspace a release passed by

`bump` refuses to run on a repository that already disagrees with itself, so it
is no help when the disagreement is what you are trying to fix. That is what
`sync` is for:

```bash
cargo run -p version-check -- sync
```

It repoints every in-repo pin at the version its package already declares and
regenerates the lockfiles, moving no version line. A requirement that is still
satisfied is left alone, so the deliberately-caret `thruput-sdk` pins under
`standard-interfaces/` keep naming the start of their series.

Reach for it after merging the default branch into a long-lived branch that
**adds a workspace**. A release sweeps every workspace it can see, but a
directory that exists only on your branch was never in the release's checkout,
so its pin and lockfile stay on the old version. Git reports no conflict --
the file is new on one side only -- and the first sign of trouble is
`version-check` failing in CI.

It requires no uncommitted changes to tracked files (untracked files are fine),
verifies the repo is self-consistent *before* touching anything, rewrites every
pin, regenerates all six lockfiles, and re-verifies. Review with `git diff`;
`git checkout` is the undo.

Decide which of the three you want, then let it do the arithmetic:

- **patch** — a normal SDK release. The standard interfaces are correctly left
  alone; their caret `thruput-sdk` pin already contains the new version.
- **breaking** — the SDK's own API broke. Also patch-bumps the standard
  interfaces and refreshes their `thruput-sdk` pins.
- **--protocol X.Y.Z** — a breaking protocol-rs change. Implies an SDK patch
  bump and an interfaces patch bump. Combine with `breaking` if the SDK's own
  API broke too.

Things it does **not** cover, so they stay on you:

- Whether the change is breaking, and what the new version number should be.
- The `TEST_FFI_VERSION` constant (step 4 below).
- Checking whether every publishable in-repo crate appears in the
  `PRIMARY_PACKAGES`, `INTERFACE_PACKAGES` or `INTERFACE_DEPENDENT_PACKAGES`
  list of `.github/workflows/publish.yml` — whichever matches where it sits
  relative to `standard-interfaces/`.

To check without bumping — before starting, or at any point:

```bash
cargo run -p version-check                       # full check
cargo run -p version-check -- --ignore-lockfiles # manifests only
```

## Files to Update (reference)

What the tool edits, and what to do by hand if you are not using it.

### 1. Root `Cargo.toml`

- `[workspace.package] version` — the workspace version (e.g. `"0.17.4"` ->
  `"0.17.5"`)
- `[workspace.dependencies]` — every entry with `version = "=X.Y.Z"` and a
  `path` pointing at an in-repo crate.

The standard-interface pins (`thruput-erc20-if`, `thruput-erc20-metadata-if`,
`thruput-erc20-pausable-if`,
`thruput-access-control-if`, `thruput-signed-op-if`,
`thruput-erc20-permit-if`) and the
implementation-component pins
(`thruput-access-control`, `thruput-signed-op`) sit on their own shared version line — `=0.1.6` at
the time of writing — and are bumped independently of the workspace version,
when the SDK undergoes a semver-major change.

### 2. Standard interface crates and implementation components

Each carries its own `thruput-sdk` dep rather than inheriting from the workspace. Update
the pin in **both** `[dependencies]` and
`[target.'cfg(not(target_arch = "riscv32"))'.dependencies]`:

- `standard-interfaces/thruput-erc20/Cargo.toml`
- `standard-interfaces/thruput-erc20-metadata/Cargo.toml`
- `standard-interfaces/thruput-erc20-pausable/Cargo.toml`
- `standard-interfaces/thruput-access-control/Cargo.toml`
- `standard-impls/thruput-access-control/Cargo.toml` — this one also carries
  an exact `=` pin on `thruput-access-control-if` (the two release in
  lockstep on the interface version line), which the bump must update too.

These pins are deliberately **caret** (`version = "0.17.4"`), not exact: the
published interface crates are compatible with any SDK patch release. Do not
"fix" them to `=`. `version-check` enforces the caret via an exception in
`crates/version-check/src/policy.rs`, so a stray `=` here fails the check.

### 3. Sub-workspace `Cargo.toml` files

Separate workspaces with their own `[workspace.dependencies]`. Update **every**
`=X.Y.Z` pin pointing at this repo, not just `thruput-sdk` — both currently pin several
in-repo crates plus the standard interfaces:

- `example-contracts/Cargo.toml`
- `integration_test/Cargo.toml`
- `logging-test/Cargo.toml`

### Workspace-excluded fixtures (must be updated manually)

Not members of any workspace, so nothing resolves them until you build them
directly. Both need their pin and their own `Cargo.lock` updated:

- `example-contracts/multi_root_collision` — one manifest + its own `Cargo.lock`
- `example-contracts/cross_crate_collision` — one pin in that workspace root's
  `[workspace.dependencies]` (members inherit it) + its own `Cargo.lock`

`version-check` scans both, including their lockfiles, so a stale pin here is
reported like any other rather than waiting to surface at build time.

### 4. `hogd-sys` FFI version (if applicable)

- `crates/hogd-sys/build.rs` — the `TEST_FFI_VERSION` constant (`"0.20.1"` at
  the time of writing). An independent version line: bump it only when the
  test-ffi release changes, not with the workspace version. Not covered by
  `version-check`.
- `crates/hogd-sys/docs-rs/bindings.rs` — on a `TEST_FFI_VERSION` bump,
  replace it with the `bindings.rs` from the new linux-x86_64 archive (on an
  x86_64 Linux host, a build leaves that copy at
  `target/debug/build/thruput-sys-*/out/bindings.rs`). The
  `docs_rs_bindings` test fails until it matches.

### 5. Lockfiles

One lockfile per workspace. The tool finds them by walking the tree, so trust
the count `version-check` prints over the list below, which has drifted before.
`cargo update --workspace` against each:

```bash
cargo update --workspace                                                          # root
cargo update --manifest-path example-contracts/Cargo.toml --workspace
cargo update --manifest-path example-contracts/cross_crate_collision/Cargo.toml --workspace
cargo update --manifest-path example-contracts/multi_root_collision/Cargo.toml --workspace
cargo update --manifest-path integration_test/Cargo.toml --workspace
cargo update --manifest-path logging-test/Cargo.toml --workspace
```

If `cargo update` fails with `failed to select a version for the requirement`
against an in-repo crate, you missed a pin. Prefer running `version-check`
first — it reports every missed pin at once, whereas `cargo update` stops at the
first one and only for the workspace it is resolving.

At this point the manifests are edited but the lockfiles are not regenerated, so
a plain run would report six lockfiles' worth of expected disagreement. Use the
flag to check just the manifests:

```bash
cargo run -p version-check -- --ignore-lockfiles
```

Get that clean *before* running the `cargo update` commands above, then run
without the flag afterwards to confirm the lockfiles caught up.

## Finishing

```bash
cargo run -p version-check
```

A clean run means every pin, in every manifest and lockfile, agrees. If it
reports a **stale exception**, the deviation it was recording has been resolved
upstream — delete that entry from `crates/version-check/src/policy.rs` rather
than leaving it in place.
