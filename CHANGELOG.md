# Changelog

All notable changes to this repository are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
— with the `0.x` caveat spelled out in the SDK docs: **any minor release may
break contracts.** Pin the exact version you build against.

Most crates in this repository release together under the workspace version
and a single `v<version>` tag. The `standard-interfaces/` and
`standard-impls/` crates carry their own version line and their own
`<crate>-v<version>` tags; they are noted here when a release moves them.

This file starts at 0.25.0. For earlier releases, read the git history.

## [Unreleased]

### Added

- Signed operations: off-chain authorization for contracts, with the
  `groundhog-signed-op-if` interface crate and the `groundhog-signed-op`
  implementation component ([#390]).
- `TimePoint` and `BlockHeight` arithmetic from `groundhog-xdr` 0.30.1, with
  `Duration` re-exported from the SDK root, so deadlines are expressed as
  time rather than as raw integers ([#392]).
- Every generated client builder takes the same receiver, and the builders
  are documented ([#394]).

### Changed

- The pinned hogd test-FFI moves to 0.28.0, and tests drive the block
  timepoint themselves ([#391]).
- `hogexec-internal` is an optional dependency of `sdk`, gated on `dev`, so
  it no longer appears in a guest build or in a contract's public dependency
  tree ([#410]).
- `sdk`'s crate description says what the crate is, and the workspace
  `repository`/`homepage` URLs drop their `.git` suffix ([#403]).
- Docs: the storage type table is complete, and the quickstart links to the
  developer docs site ([#393]).

## [0.27.0] — 2026-09-01

### Changed

- **BREAKING**: cross-contract exceptions are matched by selector instead of
  through a generated enum. `{Client}Error` types no longer exist —
  `try_*` methods return the SDK's `CallError`, and a caught exception is
  matched with the new `match_exn!` macro against whatever exception types
  the caller cares about ([#385]).
- The pinned hogd test-FFI moves to 0.27.1, and the tests assert the real
  network hash ([#389]).
- `hogd-sys` requires groundhog-cli 0.11.0 on the build-from-source path, in
  lockstep with the test-FFI pin ([#388]).

### Added

- `I256` and the OpenZeppelin `Math.sol` / `SignedMath.sol` surface in
  `groundhog-u256`: full-precision `mul_div` and `mul_shr`, modular
  arithmetic, roots and logarithms, each with a `checked_` and a panicking
  form ([#382]).
- `ERC20Pausable` and `AccessControl` standard interfaces, a reusable
  `AccessControlState` implementation component under `standard-impls/`, and
  a pausable ERC20 example ([#373]).
- `sdk::block::get_network_hash` and `get_block_timepoint` ([#386]).
- The SDK front page documents the Cargo features and is reorganized ([#384]).

## [0.26.0] — 2026-08-30

### Changed

- **BREAKING**: diagnostics no longer change what a contract computes.
  `FaultString` has one wire form on every target (empty), the `debugging`
  feature is gone in favour of `not(target_arch = "riscv32")` gates, and
  `ABORT_MAX` drops from 544 to 28 bytes and is now config-independent
  ([#380]).
- **BREAKING**: the `mock` and `call_tracking` features are gone; `dev` is
  the single feature that pulls the native executor and the hogd FFI
  ([#381]).

### Removed

- The unused public `Panic` type ([#380]).

### Added

- CI builds `sdk` for `riscv32im-unknown-none-elf` before the host suite, so
  a guest-only break in the SDK surfaces in seconds rather than in the
  contract sweeps ([#381]).

## [0.25.0] — 2026-08-29

### Changed

- Moved to protocol-rs 0.29.0 ([#379]).

[Unreleased]: https://github.com/groundhogchain/sdk_rs/compare/v0.27.0...HEAD
[0.27.0]: https://github.com/groundhogchain/sdk_rs/compare/v0.26.0...v0.27.0
[0.26.0]: https://github.com/groundhogchain/sdk_rs/compare/v0.25.0...v0.26.0
[0.25.0]: https://github.com/groundhogchain/sdk_rs/compare/v0.24.2...v0.25.0
[#373]: https://github.com/groundhogchain/sdk_rs/pull/373
[#379]: https://github.com/groundhogchain/sdk_rs/pull/379
[#380]: https://github.com/groundhogchain/sdk_rs/pull/380
[#381]: https://github.com/groundhogchain/sdk_rs/pull/381
[#382]: https://github.com/groundhogchain/sdk_rs/pull/382
[#384]: https://github.com/groundhogchain/sdk_rs/pull/384
[#385]: https://github.com/groundhogchain/sdk_rs/pull/385
[#386]: https://github.com/groundhogchain/sdk_rs/pull/386
[#388]: https://github.com/groundhogchain/sdk_rs/pull/388
[#389]: https://github.com/groundhogchain/sdk_rs/pull/389
[#390]: https://github.com/groundhogchain/sdk_rs/pull/390
[#391]: https://github.com/groundhogchain/sdk_rs/pull/391
[#392]: https://github.com/groundhogchain/sdk_rs/pull/392
[#393]: https://github.com/groundhogchain/sdk_rs/pull/393
[#394]: https://github.com/groundhogchain/sdk_rs/pull/394
[#403]: https://github.com/groundhogchain/sdk_rs/pull/403
[#410]: https://github.com/groundhogchain/sdk_rs/pull/410
