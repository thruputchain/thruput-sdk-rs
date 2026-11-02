//! Assertions on the spec sidecars the rv32 fixture builds write to
//! `target/thruput/<contract>.spec.json`.
//!
//! The JSON is minified, so a needle matches exactly: `"name":"Orphan"`
//! cannot match `"name":"OrphanEvent"`.

use regex::Regex;

use crate::elf;
use crate::rv32::elf_path;
use crate::task::{Ctx, Failure, Result};

pub enum Check {
    /// The spec contains this text.
    Has(&'static str),
    Lacks(&'static str),
    /// The spec matches this regex.
    Matches(&'static str),
    /// The spec names a type, exception, or event with this declared name.
    ///
    /// Ids are structured (crate, major, module path, declared name), and
    /// the crate and version move with every release, so these match the
    /// `"declared":"Name"` field: `Payload` cannot match `AdoptedPayload`.
    /// Interface, method, and argument names are not ids; use [`Check::Has`].
    HasId(&'static str),
    LacksId(&'static str),
}

use Check::*;

pub struct Spec {
    pub contract: &'static str,
    pub checks: &'static [Check],
}

pub static SPECS: &[Spec] = &[
    Spec {
        contract: "free_fn_unnamed",
        checks: &[
            Has(r#""format":"thruput_contract_spec_v1""#),
            Has(r#""name":"_unnamed_interface""#),
            Has(r#""version":"0.4.0""#),
        ],
    },
    Spec {
        contract: "imported_error_callee",
        checks: &[
            Has(r#""name":"ImportedErrorCallee""#),
            HasId("ImportedError"),
        ],
    },
    Spec {
        contract: "multi_imported_traits",
        checks: &[
            Has(r#""name":"ExplicitNamedInterface""#),
            Has(r#""name":"VersionedCallee""#),
            Has(r#""version":"1.2.3""#),
        ],
    },
    Spec {
        contract: "spec_named_types",
        checks: &[
            Has(r#""type_defs":["#),
            Has(r#""name":"SpecNamedTypesContract""#),
            HasId("Payload"),
            HasId("Choice"),
            HasId("Envelope"),
            Has(r#""name":"echo_blob""#),
            Has(r#""kind":"vec","element":{"kind":"u8"}"#),
            Has(r#""name":"echo_hash""#),
            Has(r#""kind":"array","len":32,"element":{"kind":"u8"}"#),
            Has(r#""name":"echo_blob_envelope""#),
            // A named element in a bounded heapless::Vec: the vector keeps
            // its bound, the element is spelled by name, and its struct
            // definition is published.
            Has(r#""name":"echo_tallies""#),
            Has(r#""declared":"Tally"}},"max_len":4"#),
            Matches(r#""kind":"struct","name":\{[^}]*"declared":"Tally"\}"#),
            HasId("PayloadEvent"),
            HasId("AuditEvent"),
            LacksId("GhostEvent"),
            Lacks(r#""name":"UnimplementedInterface""#),
            // The scheme-tagged unions are declared by thruput-contract-spec,
            // which every contract links: they must still be shaken where
            // unused.
            LacksId("PublicKey"),
            LacksId("Signature"),
        ],
    },
    // Scheme-tagged keys and signatures as contract arguments: both unions
    // are published with one byte-array case per scheme.
    Spec {
        contract: "scheme_keys",
        checks: &[
            Has(r#""name":"SchemeKeys""#),
            Matches(
                r#""kind":"union","name":\{[^}]*"declared":"PublicKey"\}"#,
            ),
            Matches(
                r#""kind":"union","name":\{[^}]*"declared":"Signature"\}"#,
            ),
            Has(
                r#""name":"ed25519","type":{"kind":"array","len":32,"element":{"kind":"u8"}}"#,
            ),
            Has(
                r#""name":"mldsa65","type":{"kind":"array","len":1952,"element":{"kind":"u8"}}"#,
            ),
            Has(
                r#""name":"mldsa65","type":{"kind":"array","len":3309,"element":{"kind":"u8"}}"#,
            ),
            HasId("KeyStored"),
        ],
    },
    // Phantom-method shaking: phantom_method_impl implements A's Ledger
    // (method ping) while linking B's colliding Ledger (method steal, payload
    // LootConfig) and carrying a same-crate collider (method phantom_local).
    // The contract must build, and its spec must advertise exactly the
    // methods with real entrypoints: ping only, with the phantom method's
    // payload typedef shaken too.
    Spec {
        contract: "phantom_method_impl",
        checks: &[
            Has(r#""name":"Ledger""#),
            Has(r#""name":"ping""#),
            Lacks(r#""name":"steal""#),
            Lacks(r#""name":"phantom_local""#),
            LacksId("LootConfig"),
        ],
    },
    // Method names, arguments, and effects come from the trait, not its
    // impl. Covers the ConstWriter encoding path.
    Spec {
        contract: "trait_sourced_method_spec",
        checks: &[
            Has(
                r#"{"name":"initialize","args":[{"name":"factory","type":{"kind":"address"}}],"return_type":{"kind":"void"}}"#,
            ),
            Has(
                r#"{"name":"set_rate","args":[{"name":"numerator","type":{"kind":"u64"}},{"name":"denominator","type":{"kind":"u64"}}],"return_type":{"kind":"u64"},"effect":"stateChanging"}"#,
            ),
            Has(
                r#"{"name":"get_rate","args":[],"return_type":{"kind":"u64"},"effect":"readOnly"}"#,
            ),
            Has(
                r#"{"name":"calculate","args":[{"name":"value","type":{"kind":"u64"}}],"return_type":{"kind":"u64"},"effect":"pure"}"#,
            ),
            Lacks(r#""_factory""#),
            Lacks(r#""_numerator""#),
            Lacks(r#""_denominator""#),
            Lacks(r#""_value""#),
        ],
    },
    // Pre-release interface versions: the macro accepts them and the spec
    // pipeline must publish them verbatim (ingestion once rejected anything
    // but MAJOR.MINOR.PATCH, so a pre-release contract compiled and then
    // failed at spec finalization).
    Spec {
        contract: "prerelease_version",
        checks: &[Has(r#""version":"0.9.0-rc.1""#)],
    },
    Spec {
        contract: "erc20_raw_storage",
        checks: &[
            Has(r#""events":["#),
            HasId("TransferEvent"),
            HasId("AllowanceChangeEvent"),
            Has(r#""name":"from","type":{"kind":"address"}"#),
        ],
    },
    Spec {
        contract: "genesis_deployer",
        checks: &[
            Has(r#""name":"GenesisDeployer""#),
            Has(r#""name":"deploy""#),
            Has(r#""name":"deploy_and_invoke""#),
        ],
    },
    // `Signed<T>` arguments and returns are signed_token nodes carrying the
    // preimage type (they consume no calldata bytes), never u32 handles.
    Spec {
        contract: "signer_callee",
        checks: &[
            Matches(
                r#""name":"auth","type":\{"kind":"signed_token","preimage":\{"kind":"named","name":\{[^}]*"declared":"TransferAuth"\}\}\}"#,
            ),
            HasId("TransferAuth"),
        ],
    },
    // `Signed<Opaque>` has an erased (null) preimage.
    Spec {
        contract: "signed_op_wallet",
        checks: &[Has(r#""kind":"signed_token","preimage":null"#)],
    },
    // Cross-crate event shaking: event_shaking_subset implements only the
    // `Adopted` trait from event_shaking_if; `Orphan` is never implemented.
    // AdoptedPayload checks that typedefs are kept, OrphanPayload that they
    // are shaken.
    Spec {
        contract: "event_shaking_subset",
        checks: &[
            Has(r#""name":"Adopted""#),
            HasId("AdoptedEvent"),
            HasId("AdoptedPayload"),
            Lacks(r#""name":"Orphan""#),
            LacksId("OrphanEvent"),
            LacksId("OrphanPayload"),
        ],
    },
    // Typedef-collision shaking: the caller links two "versions" of a market
    // interface crate (typedef_shaking_if_v1/v2) only to call them, and
    // implements neither trait. Each defines a differently shaped
    // `SharedConfig`; both definitions reach the raw section but are
    // unreachable, so the conflict must not fail the build (it once failed
    // with ConflictingTypeDef) and nothing from the market interfaces may
    // reach the final spec. This pipeline canonicalizes with the pinned CLI's
    // embedded spec-tool, not the workspace one, so it also covers version
    // skew between them.
    Spec {
        contract: "typedef_shaking_caller",
        checks: &[
            Has(r#""name":"Caller""#),
            LacksId("SharedConfig"),
            Lacks(r#""name":"MarketV1""#),
            Lacks(r#""name":"MarketV2""#),
        ],
    },
    // Every collision that qualified ids resolve, in one contract that
    // builds. Each pair shares a bare ident, which is all the spec used to
    // record, so each was merged into one entry or rejected as a
    // ConflictingTypeDef naming a type the author never wrote. Two of the
    // three pairs are the contract's dependencies' names, so it could not
    // have worked around them.
    //
    // Checked per crate, so both halves of a pair must be present: matching
    // `"declared":"Error"` alone would pass with only one of them.
    Spec {
        contract: "spec_name_reuse",
        checks: &[
            Matches(r#""crate":"dup_name_if_a",[^}]*"declared":"Error""#),
            Matches(r#""crate":"dup_name_if_b",[^}]*"declared":"Error""#),
            Matches(r#""crate":"dup_name_if_a",[^}]*"declared":"Updated""#),
            Matches(r#""crate":"dup_name_if_b",[^}]*"declared":"Updated""#),
            // Two modules of the contract's own crate reusing one ident.
            Has(r#"::ledger","declared":"Config""#),
            Has(r#"::vault","declared":"Config""#),
        ],
    },
    // Two crates, one simple name, both definitions in the pool: v1's is
    // reachable (the contract implements MarketV1) and v2's is not. Ids keep
    // them apart; before them, this contract could not be built at all, and
    // the reference would have been ambiguous between the two shapes.
    Spec {
        contract: "typedef_ids_coexist",
        checks: &[
            HasId("SharedConfig"),
            Has(r#""crate":"typedef_shaking_if_v1""#),
            Lacks(r#""crate":"typedef_shaking_if_v2""#),
        ],
    },
    // Interface-metadata conflict shaking: iface_conflict_caller links two
    // interface crates whose `Token` traits collide on the same interface key
    // (default name = trait ident, default version = workspace crate version)
    // with different emits metadata, and implements neither. Both candidates
    // are shaken, so the disagreement must not fail the build (it once failed
    // with ConflictingInterfaceEmits), and neither Token nor PingEvent may
    // reach the final spec.
    Spec {
        contract: "iface_conflict_caller",
        checks: &[
            Has(r#""name":"ConflictCaller""#),
            Lacks(r#""name":"Token""#),
            LacksId("PingEvent"),
        ],
    },
    // The anchored half of the same collision: iface_conflict_impl implements
    // A's Token and only links B's (to call a B token). Each candidate names
    // its declarer, so B's is shaken like any unimplemented interface and A's
    // metadata stands alone: A's PingEvent is kept. The declarer is build-time
    // only and must not reach the final spec.
    Spec {
        contract: "iface_conflict_impl",
        checks: &[
            Has(r#""name":"Token""#),
            HasId("PingEvent"),
            LacksId("Token"),
        ],
    },
];

/// Raw contracts (hand-written entrypoint records, no `#[contract]`) that
/// link an interface crate. The shaken candidates must leave no spec at all,
/// rather than one claiming `interfaces: []`, so there is no sidecar.
pub static NO_SPEC: &[&str] = &["raw_iface_dep"];

/// Spec-only names that must not reach the lowered `.rv32a`.
///
/// Raw spec fragments live in the ELF's `thruputspecraw*` sections, where
/// the CLI reads them; the lowerer must not copy them into the image, and
/// the final spec is a sidecar. The runtime never embeds these names
/// (dispatch uses hashed ids, events hashed signatures). Each must be in the
/// ELF, so a vanished raw section or renamed type cannot pass vacuously.
pub static NOT_IN_IMAGE: &[(&str, &[&str])] = &[
    ("event_shaking_subset", &["AdoptedEvent", "OrphanPayload"]),
    ("typedef_shaking_caller", &["SharedConfig", "MarketV1"]),
    ("iface_conflict_caller", &["PingEvent"]),
];

impl Check {
    /// Checks `spec`; on failure, says what was expected.
    fn check(&self, spec: &str) -> std::result::Result<(), String> {
        let (expect, ok) = match *self {
            Has(text) => ("contain", spec.contains(text)),
            Lacks(text) => ("lack", !spec.contains(text)),
            HasId(name) => ("contain", spec.contains(&id(name))),
            LacksId(name) => ("lack", !spec.contains(&id(name))),
            Matches(pattern) => {
                let regex = Regex::new(pattern)
                    .map_err(|e| format!("bad pattern {pattern}: {e}"))?;
                ("match", regex.is_match(spec))
            }
        };
        if ok {
            return Ok(());
        }
        let needle = match *self {
            HasId(name) | LacksId(name) => id(name),
            Has(text) | Lacks(text) | Matches(text) => text.to_string(),
        };
        Err(format!("expected to {expect} {needle}"))
    }
}

fn id(name: &str) -> String {
    format!(r#""declared":"{name}""#)
}

/// Runs every check and reports all failures together.
pub fn check(ctx: &mut Ctx) -> Result {
    let lowered = ctx.repo.lowered();
    let mut failed = Vec::new();
    for spec in SPECS {
        let path = lowered.join(format!("{}.spec.json", spec.contract));
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => {
                failed.push(format!("{}: reading spec: {e}", spec.contract));
                continue;
            }
        };
        for check in spec.checks {
            if let Err(e) = check.check(&text) {
                failed.push(format!("{}: {e}", spec.contract));
            }
        }
    }
    for contract in NO_SPEC {
        if lowered.join(format!("{contract}.spec.json")).exists() {
            failed.push(format!("{contract}: expected no spec sidecar"));
        }
    }
    for (contract, needles) in NOT_IN_IMAGE {
        let image = lowered.join(format!("{contract}.rv32a"));
        let elf = elf_path(ctx.repo, contract);
        let (image, elf) = match (std::fs::read(&image), std::fs::read(&elf)) {
            (Ok(image), Ok(elf)) => (image, elf),
            (Err(e), _) | (_, Err(e)) => {
                failed.push(format!("{contract}: reading artifacts: {e}"));
                continue;
            }
        };
        for needle in *needles {
            if !elf::contains(&elf, needle.as_bytes()) {
                failed.push(format!(
                    "{contract}: expected the ELF to contain {needle} \
                     (raw spec section missing?)"
                ));
            } else if elf::contains(&image, needle.as_bytes()) {
                failed.push(format!(
                    "{contract}: spec content {needle} leaked into the \
                     lowered image"
                ));
            }
        }
    }
    let checks = SPECS.iter().map(|s| s.checks.len()).sum::<usize>()
        + NO_SPEC.len()
        + NOT_IN_IMAGE.iter().map(|(_, n)| n.len()).sum::<usize>();
    ctx.log
        .line(&format!("{checks} spec checks, {} failed", failed.len()));
    if failed.is_empty() {
        return Ok(());
    }
    for line in &failed {
        ctx.log.line(line);
    }
    let mut failure =
        Failure::new(format!("{} spec checks failed", failed.len()));
    failure.details = failed;
    Err(failure.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_compile() {
        for spec in SPECS {
            for check in spec.checks {
                if let Matches(pattern) = check {
                    Regex::new(pattern).unwrap();
                }
            }
        }
    }

    #[test]
    fn ids_match_exactly() {
        let spec = r#"{"declared":"AdoptedPayload"}"#;
        assert!(HasId("AdoptedPayload").check(spec).is_ok());
        assert!(LacksId("Payload").check(spec).is_ok());
        assert!(HasId("Payload").check(spec).is_err());
    }
}
