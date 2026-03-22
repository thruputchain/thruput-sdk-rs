---
name: bump-version
description: Use when bumping crate versions, releasing, or updating dependency pins in this workspace
---

# Bump Version

Patch-bump the workspace and update all pinned dependency references across the three workspaces and their lockfiles.

## Files to Update

### 1. Root `Cargo.toml`

- `[workspace.package] version` — the workspace version (e.g. `"0.5.0"` -> `"0.5.1"`)
- `[workspace.dependencies]` — every entry with `version = "=X.Y.Z"` pointing at an in-repo crate (groundhog-codegen, groundhog-contract-util, groundhog-native-exec, groundhog-macros, groundhog-sys, groundhog-storage-cost, groundhog-event, groundhog-test-vectors)

### 2. Standard interface crates

Each has its own sdk dep (not inherited from workspace):

- `standard-interfaces/bottle-test-if/Cargo.toml` — `[dependencies]` and `[target.'cfg(...)'.dependencies]` sdk version
- `standard-interfaces/groundhog-erc20/Cargo.toml` — same two sections
- `standard-interfaces/groundhog-erc1271/Cargo.toml` — same two sections

### 3. Sub-workspace Cargo.toml files

These are separate workspaces with their own sdk dep pin:

- `example-contracts/Cargo.toml` — `[workspace.dependencies] sdk` version
- `integration_test/Cargo.toml` — `[workspace.dependencies] sdk` version

### 4. groundhog-sys FFI version (if applicable)

- `groundhog-sys/build.rs` — `TEST_FFI_VERSION` constant (independent version, only bump when the test-ffi release changes)

### 5. Lockfiles

Run `cargo update --workspace` in each workspace root:

```bash
cargo update --workspace                                      # root
(cd example-contracts && cargo update --workspace)            # example-contracts
(cd integration_test && cargo update --workspace)             # integration_test
```

## Verification

After all edits, confirm no stale pins remain:

```bash
grep -r 'version = "=OLD_VERSION"' --include='Cargo.toml' .
```

This should return no matches for the old version.
