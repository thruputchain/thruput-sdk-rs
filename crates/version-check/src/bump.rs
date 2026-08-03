//! Applies a coordinated version bump across every manifest and lockfile.
//!
//! This supports two kinds of releases:
//!
//! 1. **Patch-bump the SDK**. The workspace version and every `=` pin on a
//!    workspace-versioned crate move. The standard interfaces are left alone:
//!    their `sdk` pin is a caret, so `^0.17.4` still contains `0.17.5` and
//!    nothing needs to change.
//! 2. **A breaking change** — to the SDK itself, or to protocol-rs Everything
//!    in (1), plus a patch bump of the standard interfaces, plus refreshing
//!    their caret `sdk` pins to the workspace version, which absorbs however
//!    much drift accumulated since the last breaking release.
//!
//! There is no hardcoded file list here. The two crate families are told apart
//! by how they declare their own version — [`VersionSpec::Inherited`] for the
//! workspace family, a literal under `standard-interfaces/` for the interface
//! line.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use semver::Version;
use toml_edit::{DocumentMut, TableLike, Value};

use crate::model::{Repo, VersionSpec};
use crate::policy::{INTERFACE_PREFIX, PROTOCOL_CRATES};

#[derive(Debug)]
pub enum Error {
    Model(crate::model::Error),
    Io(PathBuf, std::io::Error),
    Toml(String, String),
    /// A precondition that must hold before anything is written.
    Precondition(String),
    Cargo(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Model(e) => write!(f, "{e}"),
            Error::Io(p, e) => write!(f, "{}: {e}", p.display()),
            Error::Toml(p, e) => write!(f, "{p}: {e}"),
            Error::Precondition(m) => write!(f, "{m}"),
            Error::Cargo(m) => write!(f, "{m}"),
        }
    }
}

impl From<crate::model::Error> for Error {
    fn from(e: crate::model::Error) -> Error {
        Error::Model(e)
    }
}

/// How far to move a version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Patch,
    /// Semver-incompatible: minor while the major is 0, else major.
    Breaking,
}

pub fn bumped(v: &Version, kind: Kind) -> Version {
    match kind {
        Kind::Patch => Version::new(v.major, v.minor, v.patch + 1),
        Kind::Breaking if v.major == 0 => Version::new(0, v.minor + 1, 0),
        Kind::Breaking => Version::new(v.major + 1, 0, 0),
    }
}

/// What the caller asked for.
#[derive(Debug, Default)]
pub struct Request {
    /// How to move the workspace version, or an explicit target.
    pub sdk: Option<Kind>,
    pub sdk_target: Option<Version>,
    /// New protocol-rs version. Implies an SDK bump and an interfaces bump.
    pub protocol: Option<Version>,
}

#[derive(Debug, Clone)]
pub struct Change {
    pub old: Version,
    pub new: Version,
}

impl fmt::Display for Change {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.old, self.new)
    }
}

/// Where a single edit lands.
#[derive(Debug, Clone)]
pub enum Target {
    /// The `version` key of a `[package]` / `[workspace.package]` table.
    PackageVersion { table_path: Vec<String> },
    /// The `version` requirement of one dependency entry.
    DepRequirement {
        table_path: Vec<String>,
        key: String,
    },
}

#[derive(Debug, Clone)]
pub struct Edit {
    pub manifest: String,
    pub line: usize,
    pub target: Target,
    pub old: String,
    pub new: String,
    pub what: &'static str,
}

#[derive(Debug, Default)]
pub struct Plan {
    pub workspace: Option<Change>,
    pub interfaces: Option<Change>,
    pub protocol: Option<Change>,
    pub edits: Vec<Edit>,
    /// Workspace-root manifests whose lockfiles need regenerating.
    pub workspace_roots: Vec<String>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }
}

/// Keep the requirement's operator, replacing only the version.
///
/// `=0.17.5` -> `=0.17.6`, `0.17.4` -> `0.17.5`. Multi-comparator requirements
/// are refused rather than guessed at.
fn retarget(raw: &str, new: &Version) -> Result<String, Error> {
    if raw.contains(',') {
        return Err(Error::Precondition(format!(
            "cannot retarget the multi-part requirement `{raw}`; fix it by hand"
        )));
    }
    let op: String = raw.chars().take_while(|c| !c.is_ascii_digit()).collect();
    Ok(format!("{op}{new}"))
}

/// Resolve a request against the repository into a concrete set of edits.
pub fn plan(repo: &Repo, req: &Request) -> Result<Plan, Error> {
    let root =
        repo.manifest(crate::checks::ROOT_MANIFEST).ok_or_else(|| {
            Error::Precondition("no root Cargo.toml".to_string())
        })?;
    let ws_old = root.workspace_version.clone().ok_or_else(|| {
        Error::Precondition(
            "the root manifest has no [workspace.package] version".to_string(),
        )
    })?;

    // The interface crates move together, so they must start together.
    let interface_pkgs: Vec<(&str, &Version, &str)> = repo
        .manifests
        .iter()
        .filter(|m| m.rel.starts_with(INTERFACE_PREFIX))
        .filter_map(|m| {
            let p = m.package.as_ref()?;
            Some((p.name.as_str(), p.version.as_ref()?, m.rel.as_str()))
        })
        .collect();
    let if_old = match interface_pkgs.first() {
        None => None,
        Some((_, v, _)) => {
            if let Some((n, bad, rel)) =
                interface_pkgs.iter().find(|(_, v2, _)| v2 != v)
            {
                return Err(Error::Precondition(format!(
                    "the standard interfaces are not on one version: {n} is \
                     {bad} in {rel}, but {} is {v}",
                    interface_pkgs[0].0
                )));
            }
            Some((*v).clone())
        }
    };

    // protocol-rs releases as one train, so every pin on it agrees already —
    // `external-agreement` enforces that, so take the first.
    let proto_old = repo
        .edges()
        .filter(|e| e.is_pin_site())
        .filter(|e| PROTOCOL_CRATES.contains(&e.name.as_str()))
        .find_map(|e| e.req_raw.as_deref())
        .and_then(|raw| {
            Version::parse(raw.trim_start_matches(['=', '^', '~'])).ok()
        });

    // A protocol change implies an SDK bump; patch unless asked for more.
    let sdk_kind = match (req.sdk, req.protocol.is_some()) {
        (Some(k), _) => Some(k),
        (None, true) => Some(Kind::Patch),
        (None, false) => None,
    };
    let ws_new = match (&req.sdk_target, sdk_kind) {
        (Some(v), _) => Some(v.clone()),
        (None, Some(k)) => Some(bumped(&ws_old, k)),
        (None, None) => None,
    };

    // Interfaces move on any breaking change, to either side.
    let sdk_breaking = ws_new.as_ref().is_some_and(|new| {
        new.major != ws_old.major || new.minor != ws_old.minor
    });
    let if_new = if sdk_breaking || req.protocol.is_some() {
        if_old.as_ref().map(|v| bumped(v, Kind::Patch))
    } else {
        None
    };

    let mut plan = Plan {
        workspace: ws_new.as_ref().map(|new| Change {
            old: ws_old.clone(),
            new: new.clone(),
        }),
        interfaces: if_new.as_ref().zip(if_old.as_ref()).map(|(new, old)| {
            Change {
                old: old.clone(),
                new: new.clone(),
            }
        }),
        protocol: req.protocol.as_ref().zip(proto_old.as_ref()).map(
            |(new, old)| Change {
                old: old.clone(),
                new: new.clone(),
            },
        ),
        ..Plan::default()
    };

    // The version every standard-interface `sdk` pin should end up naming.
    let ws_final = ws_new.clone().unwrap_or_else(|| ws_old.clone());
    let interface_names: Vec<&str> =
        interface_pkgs.iter().map(|(n, _, _)| *n).collect();

    if let Some(new) = &ws_new {
        plan.edits.push(Edit {
            manifest: root.rel.clone(),
            line: 0,
            target: Target::PackageVersion {
                table_path: vec!["workspace".into(), "package".into()],
            },
            old: ws_old.to_string(),
            new: new.to_string(),
            what: "workspace version",
        });
    }

    if let Some(new) = &if_new {
        for (_, _, rel) in &interface_pkgs {
            plan.edits.push(Edit {
                manifest: (*rel).to_string(),
                line: 0,
                target: Target::PackageVersion {
                    table_path: vec!["package".into()],
                },
                old: if_old
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                new: new.to_string(),
                what: "interface version",
            });
        }
    }

    for edge in repo.edges() {
        if !edge.is_pin_site() {
            continue;
        }
        let Some(raw) = edge.req_raw.as_deref() else {
            continue;
        };
        let in_interfaces = edge.manifest.starts_with(INTERFACE_PREFIX);
        let family = repo
            .locals
            .get(&edge.name)
            .and_then(|l| repo.manifest(&l.manifest))
            .and_then(|m| m.package.as_ref())
            .map(|p| p.spec.clone());

        let (target_version, what) =
            if interface_names.contains(&edge.name.as_str()) {
                match &if_new {
                    Some(v) => (v, "interface pin"),
                    None => continue,
                }
            } else if family == Some(VersionSpec::Inherited) {
                // A workspace-family crate. Inside standard-interfaces/ this is the
                // caret `sdk` pin, which only moves on a breaking release.
                if in_interfaces {
                    if if_new.is_none() {
                        continue;
                    }
                    (&ws_final, "interface sdk pin")
                } else {
                    match &ws_new {
                        Some(v) => (v, "workspace pin"),
                        None => continue,
                    }
                }
            } else if PROTOCOL_CRATES.contains(&edge.name.as_str()) {
                match &req.protocol {
                    Some(v) => (v, "protocol pin"),
                    None => continue,
                }
            } else {
                continue;
            };

        let new = retarget(raw, target_version)?;
        if new == raw {
            continue;
        }
        plan.edits.push(Edit {
            manifest: edge.manifest.clone(),
            line: edge.line,
            target: Target::DepRequirement {
                table_path: edge.table_path.clone(),
                key: edge.key.clone(),
            },
            old: raw.to_string(),
            new,
            what,
        });
    }

    plan.workspace_roots = repo
        .manifests
        .iter()
        .filter(|m| m.is_workspace_root)
        .map(|m| m.rel.clone())
        .collect();

    Ok(plan)
}

/// Write every edit, grouped so each manifest is parsed and rewritten once.
pub fn apply(repo: &Repo, plan: &Plan) -> Result<(), Error> {
    let mut by_manifest: BTreeMap<&str, Vec<&Edit>> = BTreeMap::new();
    for edit in &plan.edits {
        by_manifest.entry(&edit.manifest).or_default().push(edit);
    }

    for (rel, edits) in by_manifest {
        let path = repo.root.join(rel);
        let text = fs::read_to_string(&path)
            .map_err(|e| Error::Io(path.clone(), e))?;
        let mut doc: DocumentMut =
            text.parse().map_err(|e: toml_edit::TomlError| {
                Error::Toml(rel.to_string(), e.to_string())
            })?;

        for edit in edits {
            let (table_path, key) = match &edit.target {
                Target::PackageVersion { table_path } => {
                    (table_path.as_slice(), "version")
                }
                Target::DepRequirement { table_path, key } => {
                    (table_path.as_slice(), key.as_str())
                }
            };
            let table = table_mut(&mut doc, table_path).ok_or_else(|| {
                Error::Toml(
                    rel.to_string(),
                    format!("no table `{}`", table_path.join(".")),
                )
            })?;
            let ok = match &edit.target {
                Target::PackageVersion { .. } => {
                    set_string(table, key, &edit.new)
                }
                Target::DepRequirement { .. } => {
                    set_requirement(table, key, &edit.new)
                }
            };
            if !ok {
                return Err(Error::Toml(
                    rel.to_string(),
                    format!("could not rewrite `{key}` at line {}", edit.line),
                ));
            }
        }

        fs::write(&path, doc.to_string())
            .map_err(|e| Error::Io(path.clone(), e))?;
    }
    Ok(())
}

fn table_mut<'a>(
    doc: &'a mut DocumentMut,
    path: &[String],
) -> Option<&'a mut dyn TableLike> {
    let mut cur: &mut dyn TableLike = doc.as_table_mut();
    for seg in path {
        cur = cur.get_mut(seg)?.as_table_like_mut()?;
    }
    Some(cur)
}

/// Replace a string value in place, preserving its surrounding formatting.
fn set_string(table: &mut dyn TableLike, key: &str, new: &str) -> bool {
    let Some(item) = table.get_mut(key) else {
        return false;
    };
    let Some(v) = item.as_value_mut() else {
        return false;
    };
    let decor = v.decor().clone();
    *v = Value::from(new);
    *v.decor_mut() = decor;
    true
}

/// Set a dependency's version, whether it is written `foo = "1"` or
/// `foo = { version = "1", .. }`.
fn set_requirement(table: &mut dyn TableLike, key: &str, new: &str) -> bool {
    let Some(item) = table.get_mut(key) else {
        return false;
    };
    if item.as_str().is_some() {
        let Some(v) = item.as_value_mut() else {
            return false;
        };
        let decor = v.decor().clone();
        *v = Value::from(new);
        *v.decor_mut() = decor;
        return true;
    }
    match item.as_table_like_mut() {
        Some(t) => set_string(t, "version", new),
        None => false,
    }
}

/// `cargo update --workspace` against every workspace root.
pub fn update_lockfiles(repo: &Repo, plan: &Plan) -> Result<(), Error> {
    for rel in &plan.workspace_roots {
        let status = Command::new("cargo")
            .arg("update")
            .arg("--workspace")
            .arg("--manifest-path")
            .arg(repo.root.join(rel))
            .status()
            .map_err(|e| {
                Error::Cargo(format!("could not run cargo update: {e}"))
            })?;
        if !status.success() {
            return Err(Error::Cargo(format!(
                "`cargo update --manifest-path {rel}` failed; the manifests \
                 are already edited, so inspect `git diff` before retrying"
            )));
        }
    }
    Ok(())
}

/// Refuse to write over uncommitted work: `git diff` is the review surface.
/// What `git status --porcelain` says about the worktree.
#[derive(Debug, Default)]
pub struct Worktree {
    /// Tracked paths with staged or unstaged changes.
    pub modified: Vec<String>,
    /// Untracked paths. Git collapses whole new directories, so an entry may be
    /// a directory and end in `/`.
    pub untracked: Vec<String>,
}

impl Worktree {
    /// True when `rel` is untracked, directly or via a collapsed parent.
    pub fn is_untracked(&self, rel: &str) -> bool {
        self.untracked
            .iter()
            .any(|u| rel == u || (u.ends_with('/') && rel.starts_with(u)))
    }
}

pub fn worktree(repo: &Repo) -> Result<Worktree, Error> {
    let out = Command::new("git")
        .arg("-C")
        .arg(&repo.root)
        .arg("status")
        .arg("--porcelain")
        .output()
        .map_err(|e| Error::Cargo(format!("could not run git status: {e}")))?;
    if !out.status.success() {
        return Err(Error::Precondition(
            "git status failed; is this a git checkout?".to_string(),
        ));
    }

    let mut state = Worktree::default();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if line.len() < 4 {
            continue;
        }
        let (status, path) = line.split_at(2);
        let path = path.trim_start().to_string();
        if status == "??" {
            state.untracked.push(path);
        } else {
            state.modified.push(path);
        }
    }
    Ok(state)
}

/// Refuse to write over uncommitted work: `git diff` is the review surface.
///
/// Untracked files are fine. Nothing here can clobber a file git does not know
/// about, and refusing to run because of a stray scratch file is just noise.
/// Untracked files the bump would *edit* are a different matter — see
/// [`require_targets_tracked`].
pub fn require_clean_worktree(state: &Worktree) -> Result<(), Error> {
    if state.modified.is_empty() {
        return Ok(());
    }
    Err(Error::Precondition(format!(
        "the working tree has uncommitted changes to tracked files, and this \
         rewrites ~25 pins across six workspaces:\n  {}\ncommit or stash \
         first, or pass --allow-dirty",
        state.modified.join("\n  ")
    )))
}

/// Refuse to edit a manifest git is not tracking.
///
/// Tolerating untracked files rests on `git checkout` being able to undo the
/// bump. That is not true of a manifest git has never seen — a brand-new
/// `standard-interfaces/` crate, say, whose version and `sdk` pin the bump would
/// happily rewrite with no way back.
pub fn require_targets_tracked(
    state: &Worktree,
    plan: &Plan,
) -> Result<(), Error> {
    let mut untracked: Vec<&str> = plan
        .edits
        .iter()
        .map(|e| e.manifest.as_str())
        .filter(|m| state.is_untracked(m))
        .collect();
    untracked.sort();
    untracked.dedup();
    if untracked.is_empty() {
        return Ok(());
    }
    Err(Error::Precondition(format!(
        "the bump would edit files git is not tracking, so `git checkout` \
         could not undo it:\n  {}\n`git add` them first, or pass --allow-dirty",
        untracked.join("\n  ")
    )))
}

/// Human-readable summary of what a plan will do.
pub fn describe(plan: &Plan) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    for (label, change) in [
        ("sdk", &plan.workspace),
        ("standard-interfaces", &plan.interfaces),
        ("protocol-rs", &plan.protocol),
    ] {
        if let Some(c) = change {
            let _ = writeln!(out, "  {label}: {c}");
        }
    }
    let mut by_kind: BTreeMap<&str, usize> = BTreeMap::new();
    for edit in &plan.edits {
        *by_kind.entry(edit.what).or_default() += 1;
    }
    let counts: Vec<String> =
        by_kind.iter().map(|(k, n)| format!("{n} {k}")).collect();
    let _ =
        writeln!(out, "  {} edits: {}", plan.edits.len(), counts.join(", "));
    out
}
