# Security Policy

## Reporting a vulnerability

**Do not open a public issue for a security problem**, and do not disclose it
on a public channel before it is fixed.

Report privately via GitHub Security Advisories: go to the **Security** tab of
this repository and choose **Report a vulnerability**. That opens a private
advisory visible only to you and the maintainers.

Please include:

- what an attacker can do, and what they need to do it;
- the affected version or commit, and the affected crate;
- a reproduction — a contract, a test, or a transaction — if you have one;
- any deployment you believe is exposed.

You should get an acknowledgement from a maintainer. If you get no response,
or the Security tab offers no reporting option, please say so in a public
issue **without describing the vulnerability** and a maintainer will follow
up privately.

## Scope

This repository is the Rust SDK: the `sdk` crate and its supporting crates,
the contract macros, the standard interfaces and implementation components,
and the native test harness.

Things that especially warrant a private report:

- a way for a contract to be authorized when it should not be — the
  `Signed<T>` auth-token machinery, `AuthClaim` handling, or the standard
  `AccessControl` component;
- a mismatch between what the native test harness computes and what a
  deployed rv32 artifact computes, since a test passing then proves nothing;
- storage-key collisions or aliasing between distinct storage items;
- arithmetic that silently wraps where a contract author would reasonably
  expect it to abort.

The chain node, the CLI, and the protocol live in other repositories; a
report about those should go to them. If you are not sure, report it here and
we will route it.

## Supported versions

The SDK is `0.x` and only the latest released version is supported. There is
no backporting to older `0.x` lines: fixes land in a new release.
