# Drop `alloc` from the `logging` feature; replace logging-test with a fixture check

This follows the custom-allocator PR (`alloc-api`).

## `logging` no longer implies `alloc`

- `logging` is now `["thruput-macros/logging"]`.
- On the guest, `debug_log!` discards its arguments, so `logging` adds no
  code there. On the host, `dev` and `std` already imply `alloc-api`.
- Implying `alloc` installed the runtime's bump allocator whenever `logging`
  was on. With `thruput-sdk/logging` enabled, `custom_allocator` failed to
  link with conflicting global allocators; now it links.
- The SDK guide says `logging` has no effect on a guest build and can stay
  enabled for the RV32 artifact.

**Breaking:** a contract that relied on `logging` to enable `alloc` must
enable `thruput-sdk/alloc` itself. No contract in this repository relies on
it. The integration contracts that need `alloc` with `logging` already enable
it.

## Replace `logging-test` with `fixtures:logging`

`logging-test` is meant to catch a past bug in clients generated with
`logging` on: they used a by-value, non-`Copy` argument after moving it.
`sdk:logging` checked it only for rv32, where `debug_log!` discards its
arguments, so that check could not catch the bug.

- Remove the `logging-test/` workspace, its lockfile, and the `sdk:logging`
  task.
- Add `fixtures:logging` to the `fixtures` group. It checks `genesis_deployer`
  with `thruput-sdk/logging`:
  - **Host:** catches the regression, because the fixture's client takes
    `calldata: Vec<u8>` by value.
  - **Guest:** confirms `logging` still builds for RV32.

  One package at a time, so `logging` does not reach other fixtures through
  feature unification.
- Remove the references in `AGENTS.md`, `.github/dependabot.yml`, the
  bump-version skill, and `crates/version-check/README.md`, which now lists
  five workspaces.

## Testing

- With the bug reintroduced in `crates/macros/src/common/client.rs`, the old
  guest check still passed. `fixtures:logging` failed at its host step with
  `E0382: use of moved value: calldata`, and it passes without the bug.
- `custom_allocator` links for rv32 with `thruput-sdk/logging`.
- `lint:sdk`, `sdk:test`, `sdk:docs`, `fmt`, and `version-check` pass, as
  do gnaw's own build and its tests.
- `cooldown-check` lists workspaces with `git ls-files`, so it needs the
  `logging-test` deletion staged.

## Known issue, not fixed here

With `logging` on, a `no_std` interface crate whose own `dev` feature is off
fails to compile on the host. `debug_log!` expands to `std::println!`, which
does not resolve in a `no_std` crate. `thruput-erc20-if` gave 36 errors when
`velocity_limit` was checked with `dev,thruput-sdk/logging`. A contract test
hits this whenever it does not forward `dev` to an interface dependency. One
possible fix is for `debug_log!` to call `println!` through a path re-exported
from `thruput_sdk`.
