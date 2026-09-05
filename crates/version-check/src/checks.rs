//! The individual invariants.
//!
//! Each `check_*` function appends to `out` and never short-circuits: one run
//! reports every violation in the repository, so a release bump can be fixed in
//! a single pass instead of one `cargo update` failure at a time.

use std::collections::BTreeMap;
use std::fs;

use semver::Op;

use crate::model::{Repo, VersionSpec};
use crate::policy::{
    DEFAULT_MEMBER_EXCEPTIONS, EXTERNAL_AGREEMENT_EXCEPTIONS, Named,
    OPERATOR_EXCEPTIONS, OWN_VERSION_EXCEPTIONS,
};

pub const ROOT_MANIFEST: &str = "Cargo.toml";

#[derive(Debug, Clone)]
pub struct Violation {
    /// Stable check id, e.g. `in-repo-req`.
    pub check: &'static str,
    /// `path:line`.
    pub location: String,
    pub message: String,
    /// Extra lines of context, printed indented under the message.
    pub note: Vec<String>,
}

impl Violation {
    pub(crate) fn new(
        check: &'static str,
        location: String,
        message: String,
    ) -> Violation {
        Violation {
            check,
            location,
            message,
            note: Vec::new(),
        }
    }
}

pub fn all(repo: &Repo) -> Vec<Violation> {
    let mut out = Vec::new();
    in_repo_req(repo, &mut out);
    pin_operator(repo, &mut out);
    path_target(repo, &mut out);
    in_repo_source(repo, &mut out);
    own_version(repo, &mut out);
    lockfiles(repo, &mut out);
    default_members(repo, &mut out);
    external_agreement(repo, &mut out);
    crate::publish_order::check(repo, &mut out);
    out
}

/// C1: a pin on an in-repo package must be satisfied by that package's own
/// declared version.
fn in_repo_req(repo: &Repo, out: &mut Vec<Violation>) {
    for edge in repo.edges() {
        if !edge.is_pin_site() {
            continue;
        }
        let Some(local) = repo.locals.get(&edge.name) else {
            continue;
        };
        let Some(raw) = &edge.req_raw else { continue };
        match &edge.req {
            None => out.push(Violation::new(
                "in-repo-req",
                edge.where_(),
                format!(
                    "`{}` has an unparseable version requirement `{raw}`",
                    edge.name
                ),
            )),
            Some(req) if !req.matches(&local.version) => {
                out.push(Violation::new(
                    "in-repo-req",
                    edge.where_(),
                    format!(
                        "`{}` is pinned `{raw}` but {} declares version {}",
                        edge.name, local.manifest, local.version
                    ),
                ))
            }
            Some(_) => {}
        }
    }
}

/// C2: in-repo pins are `=` unless [`policy::OPERATOR_EXCEPTIONS`] says otherwise.
fn pin_operator(repo: &Repo, out: &mut Vec<Violation>) {
    for edge in repo.edges() {
        if !edge.is_pin_site() || !repo.locals.contains_key(&edge.name) {
            continue;
        }
        let (Some(req), Some(raw)) = (&edge.req, &edge.req_raw) else {
            continue;
        };

        let exception = OPERATOR_EXCEPTIONS.iter().find(|e| {
            edge.manifest.starts_with(e.manifest_prefix) && e.dep == edge.name
        });
        let expected = exception.map(|e| e.op).unwrap_or(Op::Exact);

        let actual: Vec<Op> = req.comparators.iter().map(|c| c.op).collect();
        if actual.len() != 1 || actual[0] != expected {
            let mut v = Violation::new(
                "pin-operator",
                edge.where_(),
                format!(
                    "`{}` is pinned `{raw}`; expected a single {} requirement",
                    edge.name,
                    describe(expected)
                ),
            );
            match exception {
                Some(e) => v.note.push(format!("exception applies: {}", e.reason)),
                None => v.note.push(
                    "in-repo crates release in lockstep, so they pin each other exactly".into(),
                ),
            }
            out.push(v);
        }
    }
}

fn describe(op: Op) -> &'static str {
    match op {
        Op::Exact => "exact (`=X.Y.Z`)",
        Op::Caret => "caret (`X.Y.Z`)",
        Op::Tilde => "tilde (`~X.Y.Z`)",
        _ => "comparison",
    }
}

/// C3: every `path` dependency resolves to a manifest declaring the expected
/// package name.
fn path_target(repo: &Repo, out: &mut Vec<Violation>) {
    for m in &repo.manifests {
        for edge in &m.edges {
            let Some(rel) = &edge.path else { continue };
            let joined = m.dir.join(rel);
            let Ok(abs) = fs::canonicalize(&joined) else {
                out.push(Violation::new(
                    "path-target",
                    edge.where_(),
                    format!(
                        "`{}` has path `{rel}`, which does not exist",
                        edge.key
                    ),
                ));
                continue;
            };
            let target = repo.manifests.iter().find(|mm| {
                fs::canonicalize(&mm.dir).map(|c| c == abs).unwrap_or(false)
            });
            match target.and_then(|mm| mm.package.as_ref()) {
                None => out.push(Violation::new(
                    "path-target",
                    edge.where_(),
                    format!("`{}` has path `{rel}`, which holds no package", edge.key),
                )),
                Some(pkg) if pkg.name != edge.name => out.push(Violation::new(
                    "path-target",
                    edge.where_(),
                    format!(
                        "`{}` resolves to path `{rel}`, which declares package `{}`",
                        edge.name, pkg.name
                    ),
                )),
                Some(_) => {}
            }
        }
    }
}

/// C4: a dependency naming an in-repo package must reach it by `path`, not by
/// falling back to the registry.
fn in_repo_source(repo: &Repo, out: &mut Vec<Violation>) {
    for edge in repo.edges() {
        if !edge.is_pin_site() || edge.path.is_some() {
            continue;
        }
        if repo.locals.contains_key(&edge.name) {
            out.push(Violation::new(
                "in-repo-source",
                edge.where_(),
                format!(
                    "`{}` is an in-repo package but is declared without a `path`, \
                     so it resolves from the registry",
                    edge.name
                ),
            ));
        }
    }
}

/// C5: root-workspace members inherit the workspace version.
fn own_version(repo: &Repo, out: &mut Vec<Violation>) {
    for m in &repo.manifests {
        let Some(pkg) = &m.package else { continue };
        if repo.owning_workspace(&m.rel).map(|w| w.rel.as_str())
            != Some(ROOT_MANIFEST)
        {
            continue;
        }
        let excepted = Named::find(OWN_VERSION_EXCEPTIONS, &pkg.name);
        match (&pkg.spec, excepted) {
            (VersionSpec::Inherited, Some(e)) => out.push({
                let mut v = Violation::new(
                    "own-version",
                    format!("{}:{}", m.rel, pkg.line),
                    format!(
                        "`{}` inherits the workspace version but is listed as an \
                         own-version exception",
                        pkg.name
                    ),
                );
                v.note.push(format!("stale exception: {}", e.reason));
                v.note.push("remove it from policy::OWN_VERSION_EXCEPTIONS".into());
                v
            }),
            (VersionSpec::Inherited, None) => {}
            (_, Some(_)) => {}
            (VersionSpec::Literal(v), None) => out.push(Violation::new(
                "own-version",
                format!("{}:{}", m.rel, pkg.line),
                format!(
                    "`{}` declares version {v} instead of `version.workspace = true`",
                    pkg.name
                ),
            )),
            (VersionSpec::Missing, None) => out.push(Violation::new(
                "own-version",
                format!("{}:{}", m.rel, pkg.line),
                format!("`{}` has no resolvable version", pkg.name),
            )),
        }
    }
}

/// C6: every lockfile agrees with the manifests about in-repo versions, and
/// resolves in-repo packages from their path rather than the registry.
fn lockfiles(repo: &Repo, out: &mut Vec<Violation>) {
    for lock in &repo.lockfiles {
        for (name, version, has_source, line) in &lock.packages {
            let Some(local) = repo.locals.get(name) else {
                continue;
            };
            let location = format!("{}:{}", lock.rel, line);
            if *has_source {
                out.push(Violation::new(
                    "lockfile",
                    location,
                    format!(
                        "`{name}` is an in-repo package but is locked to a registry \
                         source; re-run `cargo update` for this workspace"
                    ),
                ));
            } else if version != &local.version.to_string() {
                out.push(Violation::new(
                    "lockfile",
                    location,
                    format!(
                        "`{name}` is locked at {version} but {} declares {}",
                        local.manifest, local.version
                    ),
                ));
            }
        }
    }
}

/// C7: `crates/*` members appear in `default-members`, so host-wide
/// `cargo build`/`test`/`doc` cover them.
fn default_members(repo: &Repo, out: &mut Vec<Violation>) {
    let Some(root) = repo.manifest(ROOT_MANIFEST) else {
        return;
    };
    for m in &repo.manifests {
        let Some(pkg) = &m.package else { continue };
        if repo.owning_workspace(&m.rel).map(|w| w.rel.as_str())
            != Some(ROOT_MANIFEST)
        {
            continue;
        }
        let dir = m.rel.strip_suffix("/Cargo.toml").unwrap_or(&m.rel);
        if !dir.starts_with("crates/") {
            continue;
        }
        let listed = root
            .default_members
            .iter()
            .any(|entry| matches_member(entry, dir));
        let anchor =
            format!("{}:{}", ROOT_MANIFEST, root.default_members_line);
        match (listed, Named::find(DEFAULT_MEMBER_EXCEPTIONS, &pkg.name)) {
            (false, None) => out.push({
                let mut v = Violation::new(
                    "default-members",
                    anchor,
                    format!("`{}` ({dir}) is missing from default-members", pkg.name),
                );
                v.note.push(
                    "host-wide `cargo build`/`test`/`doc` build only default-members, \
                     so an omitted crate is silently untested"
                        .into(),
                );
                v
            }),
            (true, Some(e)) => out.push({
                let mut v = Violation::new(
                    "default-members",
                    anchor,
                    format!(
                        "`{}` is listed in default-members but is recorded as an \
                         exception",
                        pkg.name
                    ),
                );
                v.note.push(format!("stale exception: {}", e.reason));
                v
            }),
            _ => {}
        }
    }
}

/// `crates/*` matches `crates/sdk`; exact entries match themselves.
fn matches_member(entry: &str, dir: &str) -> bool {
    match entry.strip_suffix("/*") {
        Some(prefix) => {
            dir.strip_prefix(prefix)
                .map(|r| r.starts_with('/') && !r[1..].contains('/'))
                == Some(true)
        }
        None => entry == dir,
    }
}

/// C8: every manifest that pins the same external crate pins it identically.
///
/// Excused sites are excluded from the comparison rather than disabling the
/// crate's check, and an exception whose site has since come into line is itself
/// reported so the list cannot quietly rot.
fn external_agreement(repo: &Repo, out: &mut Vec<Violation>) {
    // name -> normalized requirement -> sites
    let mut seen: BTreeMap<&str, BTreeMap<String, Vec<String>>> =
        BTreeMap::new();
    // exception index -> (site, requirement) it excused
    let mut excused: BTreeMap<usize, Vec<(String, String)>> = BTreeMap::new();

    for edge in repo.edges() {
        if !edge.is_pin_site() || repo.locals.contains_key(&edge.name) {
            continue;
        }
        let Some(req) = &edge.req else { continue };
        let exception = EXTERNAL_AGREEMENT_EXCEPTIONS.iter().position(|e| {
            e.crate_name == edge.name
                && edge.manifest.starts_with(e.manifest_prefix)
        });
        match exception {
            Some(i) => excused
                .entry(i)
                .or_default()
                .push((edge.where_(), req.to_string())),
            None => seen
                .entry(&edge.name)
                .or_default()
                .entry(req.to_string())
                .or_default()
                .push(edge.where_()),
        }
    }

    for (name, reqs) in &seen {
        if reqs.len() < 2 {
            continue;
        }
        // Anchor the report at the first site of the least-common requirement so
        // the location points at something worth editing.
        let mut ordered: Vec<(&String, &Vec<String>)> = reqs.iter().collect();
        ordered.sort_by_key(|(_, sites)| sites.len());
        let location = ordered[0].1[0].clone();
        let mut v = Violation::new(
            "external-agreement",
            location,
            format!(
                "`{name}` is pinned {} different ways across the repo",
                reqs.len()
            ),
        );
        for (req, sites) in reqs.iter().rev() {
            v.note.push(format!("`{req}` at {}", sites.join(", ")));
        }
        out.push(v);
    }

    for (i, e) in EXTERNAL_AGREEMENT_EXCEPTIONS.iter().enumerate() {
        let policy = "crates/version-check/src/policy.rs".to_string();
        let Some(sites) = excused.get(&i) else {
            // Nothing matched — but if the exception points into a submodule
            // that is not checked out, those files were never scanned, so this
            // says nothing about whether the exception is still needed.
            if repo.is_unscanned(e.manifest_prefix) {
                continue;
            }
            let mut v = Violation::new(
                "external-agreement",
                policy,
                format!(
                    "the `{}` exception under {} matches no pin site",
                    e.crate_name, e.manifest_prefix
                ),
            );
            v.note.push(format!("stale exception: {}", e.reason));
            out.push(v);
            continue;
        };
        // If the rest of the repo agrees on one requirement and every excused
        // site now matches it, the exception has served its purpose.
        let consensus: Vec<&String> = seen
            .get(e.crate_name)
            .map(|m| m.keys().collect())
            .unwrap_or_default();
        if consensus.len() == 1
            && sites.iter().all(|(_, req)| req == consensus[0])
        {
            let mut v = Violation::new(
                "external-agreement",
                sites[0].0.clone(),
                format!(
                    "`{}` now agrees with the rest of the repo at `{}`",
                    e.crate_name, consensus[0]
                ),
            );
            v.note.push(format!("stale exception: {}", e.reason));
            v.note.push(
                "remove it from policy::EXTERNAL_AGREEMENT_EXCEPTIONS".into(),
            );
            out.push(v);
        }
    }
}
