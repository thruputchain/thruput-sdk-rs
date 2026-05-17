---
name: bump-version
description: Use when bumping crate versions, releasing, or updating dependency pins in this workspace
---

# Bump Version

Patch-bump the workspace and update all pinned dependency references across the three workspaces and their lockfiles.

## Files to Update

### 1. Root `Cargo.toml`

- `[workspace.package] version` — the workspace version (e.g. `"0.5.0"` -> `"0.5.1"`)
- `[workspace.dependencies]` — every entry with `version = "=X.Y.Z"` pointing at an in-repo crate. Currently: groundhog-contract-util, groundhog-contract-spec, groundhog-native-exec, groundhog-macros, groundhog-sys, groundhog-storage-cost, groundhog-event, groundhog-spec-tool, groundhog-test-vectors. **Authoritative check:** `grep '=' Cargo.toml | grep 'path = ' | grep 'version = "='` — every match starting with `groundhog-` (or `bottle-test-if`) must be bumped. The standard-interface pins (`groundhog-erc20-if`, `groundhog-erc20-metadata-if`, `groundhog-erc1271-if`, `bottle-test-if`) have their own version line (currently `=0.1.0`) and are bumped independently, not with the workspace version.

### 2. Standard interface crates

Each has its own sdk dep (not inherited from workspace). Update both `[dependencies]` and `[target.'cfg(not(target_family = "wasm"))'.dependencies]` sdk version pins:

- `standard-interfaces/bottle-test-if/Cargo.toml`
- `standard-interfaces/groundhog-erc20/Cargo.toml`
- `standard-interfaces/groundhog-erc20-metadata/Cargo.toml`
- `standard-interfaces/groundhog-erc1271/Cargo.toml`

**Authoritative check:** `ls standard-interfaces/*/Cargo.toml` — every entry needs the sdk pin bumped. If new crates have been added, include them.

### 3. Sub-workspace Cargo.toml files

These are separate workspaces with their own pinned deps in `[workspace.dependencies]`. Update **every** `=X.Y.Z` pin pointing at this repo, not just `sdk`:

- `example-contracts/Cargo.toml` — currently pins both `sdk` and `groundhog-contract-util` at the workspace version
- `integration_test/Cargo.toml` — currently pins `sdk` at the workspace version

(After bumping the workspace, run `grep 'version = "=OLD"' example-contracts/Cargo.toml integration_test/Cargo.toml` to catch any pin that was missed.)

### 4. groundhog-sys FFI version (if applicable)

- `groundhog-sys/build.rs` — `TEST_FFI_VERSION` constant (independent version, only bump when the test-ffi release changes)

### 5. Lockfiles

Run `cargo update --workspace` against each workspace's manifest:

```bash
cargo update --workspace                                                  # root
cargo update --manifest-path example-contracts/Cargo.toml --workspace
cargo update --manifest-path integration_test/Cargo.toml --workspace
cargo update --manifest-path logging-test/Cargo.toml --workspace
```

If `cargo update` fails with `failed to select a version for the requirement` against an in-repo crate, you missed a pin — fix it before re-running. The error names the offending crate.

## Verification

After all edits, confirm no stale pins remain:

```bash
grep -r 'version = "=OLD_VERSION"' --include='Cargo.toml' .
```

This should return no matches for the old version.
