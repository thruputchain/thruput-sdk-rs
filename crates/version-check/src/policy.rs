//! Deliberate deviations from the default rules.
//!
//! Every entry carries a `reason`. Adding an exception means writing down why
//! the deviation is correct — a silent allowlist would defeat the point of the
//! checker.

use semver::Op;

/// A dependency edge whose requirement operator is deliberately not `=`.
pub struct OperatorException {
    /// Matched against the repo-relative manifest path as a prefix.
    pub manifest_prefix: &'static str,
    pub dep: &'static str,
    pub op: Op,
    pub reason: &'static str,
}

/// A pin site excluded from the external-agreement comparison.
///
/// Scoped to a manifest prefix rather than excusing the crate repo-wide: a
/// blanket exception on a crate as widely pinned as `thiserror` would hide every
/// future divergence along with the one being excused.
pub struct AgreementException {
    pub crate_name: &'static str,
    /// Matched against the repo-relative manifest path as a prefix.
    pub manifest_prefix: &'static str,
    pub reason: &'static str,
}

/// A named subject (package or crate) excused from a check.
pub struct Named {
    pub name: &'static str,
    pub reason: &'static str,
}

impl Named {
    pub fn find<'a>(list: &'a [Named], name: &str) -> Option<&'a Named> {
        list.iter().find(|n| n.name == name)
    }
}

/// Manifests under this prefix hold the standard-interface crates, which carry
/// their own version line rather than the workspace's.
pub const INTERFACE_PREFIX: &str = "standard-interfaces/";

/// Manifests under this prefix hold the standard-interface implementation
/// components, which ride the interface crates' version line and release in
/// lockstep with them.
pub const IMPL_PREFIX: &str = "standard-impls/";

/// True when `rel` belongs to the independently-versioned interface family:
/// the standard-interface crates and their implementation components.
pub fn in_interface_family(rel: &str) -> bool {
    rel.starts_with(INTERFACE_PREFIX) || rel.starts_with(IMPL_PREFIX)
}

/// Crates from the protocol-rs repository, which release together at one
/// version. A breaking change here forces a new standard-interfaces release, so
/// that clients can request a version built against it.
pub const PROTOCOL_CRATES: &[&str] = &[
    "groundhog-spec",
    "groundhog-trace",
    "groundhog-xdr",
    "groundhog-syscall",
    "hogexec-internal",
];

/// In-repo pins that are not `=`.
pub const OPERATOR_EXCEPTIONS: &[OperatorException] = &[
    OperatorException {
        manifest_prefix: "standard-interfaces/",
        dep: "sdk",
        op: Op::Caret,
        reason: "the published interface crates are compatible with any sdk \
                 patch release, so they deliberately do not pin the SDK in \
                 lockstep",
    },
    OperatorException {
        manifest_prefix: "standard-impls/",
        dep: "sdk",
        op: Op::Caret,
        reason: "the published implementation components follow the interface \
                 crates' policy: compatible with any sdk patch release, so \
                 they deliberately do not pin the SDK in lockstep",
    },
];

/// Packages allowed to write a literal `version` instead of inheriting the
/// workspace version.
pub const OWN_VERSION_EXCEPTIONS: &[Named] = &[
    Named {
        name: "groundhog-token-controls-if",
        reason: "standard interface with an independent version line, bumped \
                 on SDK semver-major rather than every release",
    },
    Named {
        name: "groundhog-erc20-if",
        reason: "standard interfaces share an independent version line, bumped \
                 on SDK semver-major rather than every release",
    },
    Named {
        name: "groundhog-erc20-metadata-if",
        reason: "standard interfaces share an independent version line, bumped \
                 on SDK semver-major rather than every release",
    },
    Named {
        name: "groundhog-erc20-pausable-if",
        reason: "standard interfaces share an independent version line, bumped \
                 on SDK semver-major rather than every release",
    },
    Named {
        name: "groundhog-access-control-if",
        reason: "standard interfaces share an independent version line, bumped \
                 on SDK semver-major rather than every release",
    },
    Named {
        name: "groundhog-signed-op-if",
        reason: "standard interfaces share an independent version line, bumped \
                 on SDK semver-major rather than every release",
    },
    Named {
        name: "groundhog-erc20-permit-if",
        reason: "standard interfaces share an independent version line, bumped \
                 on SDK semver-major rather than every release",
    },
    Named {
        name: "groundhog-erc165-if",
        reason: "standard interfaces share an independent version line, bumped \
                 on SDK semver-major rather than every release",
    },
    Named {
        name: "groundhog-erc721-if",
        reason: "standard interfaces share an independent version line, bumped \
                 on SDK semver-major rather than every release",
    },
    Named {
        name: "groundhog-access-control",
        reason: "implementation component on the standard interfaces' \
                 independent version line, released in lockstep with its \
                 interface crate",
    },
    Named {
        name: "groundhog-signed-op",
        reason: "implementation component on the standard interfaces' \
                 independent version line, released in lockstep with its \
                 interface crate",
    },
    Named {
        name: "version-mismatch-helper",
        reason: "fixture that deliberately carries a fixed version to exercise \
                 the SDK's version-mismatch diagnostics",
    },
];

/// `crates/*` members deliberately absent from `default-members`.
pub const DEFAULT_MEMBER_EXCEPTIONS: &[Named] = &[Named {
    name: "groundhog-rv32-runtime",
    reason: "target-only runtime: a top-level compile_error! rejects every \
                 target but rv32, so host-wide builds must skip it",
}];

/// Pin sites excluded from the external-agreement comparison.
///
/// They only bite when the `integration_test/contracts` submodule is checked
/// out. When it is not, the files were never scanned, so
/// `Repo::is_unscanned` suppresses the stale-exception report rather than
/// treating the absence as evidence the exception is obsolete.
pub const EXTERNAL_AGREEMENT_EXCEPTIONS: &[AgreementException] = &[
    AgreementException {
        crate_name: "hex-literal",
        manifest_prefix: "integration_test/contracts/",
        reason: "pending a PR to the integration_test_contracts submodule: \
                 hash_bug pins hex-literal 1.1.0 inline where it should inherit \
                 `hex-literal.workspace = true` from integration_test",
    },
    AgreementException {
        crate_name: "thiserror",
        manifest_prefix: "integration_test/contracts/",
        reason: "pending a PR to the integration_test_contracts submodule: \
                 manual_revert_test pins thiserror 2.0.10 inline where it should \
                 inherit `thiserror = { workspace = true }` from integration_test",
    },
];
