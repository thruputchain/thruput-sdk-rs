//! Repository discovery and manifest parsing.
//!
//! Everything here is deliberately *syntactic*: we record whether a pin was
//! written inline or inherited with `workspace = true`, because that
//! distinction is exactly what the checks audit. (A crate like `cargo_toml`
//! would resolve inheritance for us and erase it.)

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use semver::{Version, VersionReq};
use toml_edit::{Document, Item, TableLike};

/// Every dependency table a manifest can carry a pin in.
pub const DEP_KINDS: [&str; 3] =
    ["dependencies", "dev-dependencies", "build-dependencies"];

/// Directory names never worth descending into.
const SKIP_DIRS: [&str; 4] = ["target", ".git", ".github", "tmp"];

#[derive(Debug)]
pub enum Error {
    Io(PathBuf, std::io::Error),
    Toml(PathBuf, String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(p, e) => write!(f, "{}: {e}", p.display()),
            Error::Toml(p, e) => write!(f, "{}: {e}", p.display()),
        }
    }
}

/// How a package's own `version` field is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionSpec {
    Literal(Version),
    /// `version.workspace = true`
    Inherited,
    Missing,
}

#[derive(Debug)]
pub struct Package {
    pub name: String,
    pub spec: VersionSpec,
    /// Resolved version: the literal, or the owning workspace's.
    pub version: Option<Version>,
    /// True unless the manifest says `publish = false`.
    pub publish: bool,
    /// Line of the `[package]` header, for messages.
    pub line: usize,
}

/// One dependency declaration site.
#[derive(Debug, Clone)]
pub struct Edge {
    pub manifest: String,
    pub line: usize,
    /// Human-readable table path, e.g. `target.cfg(unix).dependencies`.
    pub table: String,
    /// The same table, as segments — what a writer needs to navigate back to
    /// this entry. `table` is for humans; this is for [`crate::bump`].
    pub table_path: Vec<String>,
    pub kind: &'static str,
    /// The table key as written.
    pub key: String,
    /// The package actually depended on (`package = "..."` if renamed).
    pub name: String,
    pub req_raw: Option<String>,
    /// `None` while `req_raw` is `Some` means the requirement is malformed.
    pub req: Option<VersionReq>,
    pub path: Option<String>,
    /// `workspace = true` — not a pin site; the pin lives in the workspace table.
    pub inherited: bool,
}

impl Edge {
    /// A site that names its own version, and so can be checked or bumped.
    pub fn is_pin_site(&self) -> bool {
        !self.inherited
    }

    pub fn where_(&self) -> String {
        format!("{}:{}", self.manifest, self.line)
    }
}

#[derive(Debug)]
pub struct Manifest {
    /// Repo-relative path, always with `/` separators.
    pub rel: String,
    pub dir: PathBuf,
    pub is_workspace_root: bool,
    /// `[workspace.package] version`, if this is a workspace root.
    pub workspace_version: Option<Version>,
    pub members: Vec<String>,
    pub default_members: Vec<String>,
    /// Line of the `default-members` key, for messages.
    pub default_members_line: usize,
    pub package: Option<Package>,
    pub edges: Vec<Edge>,
}

/// An in-repo package, keyed by its declared name.
#[derive(Debug)]
pub struct Local {
    pub name: String,
    pub version: Version,
    pub manifest: String,
    pub dir: PathBuf,
}

#[derive(Debug)]
pub struct Lockfile {
    pub rel: String,
    /// `(name, version, has_source, line)` for every `[[package]]`.
    pub packages: Vec<(String, String, bool, usize)>,
}

#[derive(Debug)]
pub struct Repo {
    pub root: PathBuf,
    pub manifests: Vec<Manifest>,
    pub lockfiles: Vec<Lockfile>,
    /// In-repo packages by name. A package with an unresolvable version is
    /// omitted here and reported by [`crate::checks`] instead.
    pub locals: BTreeMap<String, Local>,
    /// True when lockfiles were deliberately not read, so an empty `lockfiles`
    /// is a choice rather than a finding.
    pub lockfiles_ignored: bool,
    /// Submodule paths that are checked out, and so were scanned.
    pub included_submodules: Vec<String>,
    /// Submodule paths that are not checked out, and so could not be scanned.
    pub absent_submodules: Vec<String>,
}

/// Knobs for [`Repo::load_with`].
#[derive(Debug, Clone, Copy, Default)]
pub struct LoadOptions {
    /// Skip reading lockfiles, so the `lockfile` check has nothing to inspect.
    ///
    /// Useful mid-bump, when the manifests have been edited but the six
    /// lockfiles have not been regenerated yet — regenerating them needs
    /// registry access, which the manifest checks do not.
    pub ignore_lockfiles: bool,
}

impl Repo {
    /// Load with the default options: everything is read.
    pub fn load(root: &Path) -> Result<Repo, Error> {
        Repo::load_with(root, LoadOptions::default())
    }

    pub fn load_with(root: &Path, opts: LoadOptions) -> Result<Repo, Error> {
        let (included_submodules, absent_submodules) =
            partition_submodules(root);
        let mut manifest_paths = Vec::new();
        let mut lock_paths = Vec::new();
        collect(
            root,
            root,
            &absent_submodules,
            &mut manifest_paths,
            &mut lock_paths,
        )?;
        manifest_paths.sort();
        lock_paths.sort();

        let mut manifests = Vec::new();
        for path in &manifest_paths {
            manifests.push(parse_manifest(root, path)?);
        }
        resolve_inherited_versions(&mut manifests);

        let mut locals = BTreeMap::new();
        for m in &manifests {
            if let Some(pkg) = &m.package
                && let Some(v) = &pkg.version
            {
                locals.insert(
                    pkg.name.clone(),
                    Local {
                        name: pkg.name.clone(),
                        version: v.clone(),
                        manifest: m.rel.clone(),
                        dir: m.dir.clone(),
                    },
                );
            }
        }

        let mut lockfiles = Vec::new();
        if !opts.ignore_lockfiles {
            for path in &lock_paths {
                lockfiles.push(parse_lockfile(root, path)?);
            }
        }

        Ok(Repo {
            lockfiles_ignored: opts.ignore_lockfiles,
            included_submodules,
            absent_submodules,
            root: root.to_path_buf(),
            manifests,
            lockfiles,
            locals,
        })
    }

    pub fn manifest(&self, rel: &str) -> Option<&Manifest> {
        self.manifests.iter().find(|m| m.rel == rel)
    }

    /// Nearest ancestor workspace root for `rel` (excluding `rel` itself).
    pub fn owning_workspace(&self, rel: &str) -> Option<&Manifest> {
        let dir = parent_dir(rel);
        self.manifests
            .iter()
            .filter(|m| m.is_workspace_root && m.rel != rel)
            .filter(|m| {
                let wdir = parent_dir(&m.rel);
                wdir.is_empty()
                    || dir == wdir
                    || dir.starts_with(&format!("{wdir}/"))
            })
            .max_by_key(|m| parent_dir(&m.rel).len())
    }

    pub fn edges(&self) -> impl Iterator<Item = &Edge> {
        self.manifests.iter().flat_map(|m| m.edges.iter())
    }

    /// True when `prefix` names a location inside a submodule that is not
    /// checked out.
    ///
    /// Lets the checks distinguish "this policy entry is obsolete" from "the
    /// files it refers to were never scanned" — a distinction that decides
    /// whether a stale-exception report is a real finding or an artifact of the
    /// checkout.
    pub fn is_unscanned(&self, prefix: &str) -> bool {
        let prefix = prefix.trim_end_matches('/');
        self.absent_submodules
            .iter()
            .any(|s| prefix == s || prefix.starts_with(&format!("{s}/")))
    }
}

fn parent_dir(rel: &str) -> String {
    match rel.rfind('/') {
        Some(i) => rel[..i].to_string(),
        None => String::new(),
    }
}

/// Submodule paths declared in `.gitmodules`, repo-relative.
///
/// `.gitmodules` is git-config format, not TOML — `path = a/b` is a bare value
/// that a TOML parser rejects — so scan it by line.
fn submodule_paths(root: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(root.join(".gitmodules")) else {
        return Vec::new();
    };
    let mut out: Vec<String> = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("path"))
        .filter_map(|rest| rest.trim_start().strip_prefix('='))
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Split declared submodules into (checked out, not checked out).
///
/// An uninitialized submodule is an empty directory, so a populated one is one
/// holding something other than its own `.git` gitlink.
fn partition_submodules(root: &Path) -> (Vec<String>, Vec<String>) {
    let mut included = Vec::new();
    let mut absent = Vec::new();
    for rel in submodule_paths(root) {
        let populated = fs::read_dir(root.join(&rel))
            .map(|entries| entries.flatten().any(|e| e.file_name() != ".git"))
            .unwrap_or(false);
        if populated {
            included.push(rel);
        } else {
            absent.push(rel);
        }
    }
    (included, absent)
}

fn collect(
    root: &Path,
    dir: &Path,
    skip: &[String],
    manifests: &mut Vec<PathBuf>,
    locks: &mut Vec<PathBuf>,
) -> Result<(), Error> {
    let entries =
        fs::read_dir(dir).map_err(|e| Error::Io(dir.to_path_buf(), e))?;
    let mut subdirs = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| Error::Io(dir.to_path_buf(), e))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let ty = entry.file_type().map_err(|e| Error::Io(path.clone(), e))?;
        if ty.is_dir() {
            let rel = rel_path(root, &path);
            if !SKIP_DIRS.contains(&name.as_str()) && !skip.contains(&rel) {
                subdirs.push(path);
            }
        } else if name == "Cargo.toml" {
            manifests.push(path);
        } else if name == "Cargo.lock" {
            locks.push(path);
        }
    }
    subdirs.sort();
    for sub in subdirs {
        collect(root, &sub, skip, manifests, locks)?;
    }
    Ok(())
}

fn read_doc(path: &Path) -> Result<(String, Document<String>), Error> {
    let text = fs::read_to_string(path)
        .map_err(|e| Error::Io(path.to_path_buf(), e))?;
    let doc = Document::parse(text.clone())
        .map_err(|e| Error::Toml(path.to_path_buf(), e.to_string()))?;
    Ok((text, doc))
}

fn parse_manifest(root: &Path, path: &Path) -> Result<Manifest, Error> {
    let (text, doc) = read_doc(path)?;
    let rel = rel_path(root, path);
    let dir = path.parent().unwrap_or(root).to_path_buf();

    let workspace = doc.get("workspace").and_then(|i| i.as_table_like());
    let is_workspace_root = workspace.is_some();
    let workspace_version = workspace
        .and_then(|w| w.get("package"))
        .and_then(|i| i.as_table_like())
        .and_then(|p| p.get("version"))
        .and_then(|i| i.as_str())
        .and_then(|s| Version::parse(s).ok());

    let string_list = |w: &dyn TableLike, key: &str| -> Vec<String> {
        w.get(key)
            .and_then(|i| i.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    let members = workspace
        .map(|w| string_list(w, "members"))
        .unwrap_or_default();
    let default_members = workspace
        .map(|w| string_list(w, "default-members"))
        .unwrap_or_default();
    let default_members_line =
        key_line(&text, &doc, &["workspace"], "default-members");

    let package =
        doc.get("package").and_then(|i| i.as_table_like()).map(|p| {
            let name = p
                .get("name")
                .and_then(|i| i.as_str())
                .unwrap_or_default()
                .to_string();
            let spec = match p.get("version") {
                None => VersionSpec::Missing,
                Some(item) => {
                    if let Some(s) = item.as_str() {
                        match Version::parse(s) {
                            Ok(v) => VersionSpec::Literal(v),
                            Err(_) => VersionSpec::Missing,
                        }
                    } else if inherits_workspace(item) {
                        VersionSpec::Inherited
                    } else {
                        VersionSpec::Missing
                    }
                }
            };
            let publish = !matches!(p.get("publish"), Some(item) if item.as_bool() == Some(false));
            Package {
                name,
                version: match &spec {
                    VersionSpec::Literal(v) => Some(v.clone()),
                    _ => None,
                },
                spec,
                publish,
                line: key_line(&text, &doc, &[], "package"),
            }
        });

    let mut edges = Vec::new();
    for kind in DEP_KINDS {
        push_edges(&text, &doc, &rel, &[kind], kind, kind, &mut edges);
    }
    if let Some(target) = doc.get("target").and_then(|i| i.as_table_like()) {
        for (cfg, item) in target.iter() {
            if item.as_table_like().is_none() {
                continue;
            }
            for kind in DEP_KINDS {
                let label = format!("target.{cfg}.{kind}");
                push_edges(
                    &text,
                    &doc,
                    &rel,
                    &["target", cfg, kind],
                    kind,
                    &label,
                    &mut edges,
                );
            }
        }
    }
    if workspace.is_some() {
        push_edges(
            &text,
            &doc,
            &rel,
            &["workspace", "dependencies"],
            "dependencies",
            "workspace.dependencies",
            &mut edges,
        );
    }

    Ok(Manifest {
        rel,
        dir,
        is_workspace_root,
        workspace_version,
        members,
        default_members,
        default_members_line,
        package,
        edges,
    })
}

/// True for `key.workspace = true` / `key = { workspace = true, .. }`.
fn inherits_workspace(item: &Item) -> bool {
    item.as_table_like()
        .and_then(|t| t.get("workspace"))
        .and_then(|i| i.as_bool())
        .unwrap_or(false)
}

/// Walk down a table path, returning the table at the end.
fn table_at<'a>(
    doc: &'a Document<String>,
    path: &[&str],
) -> Option<&'a dyn TableLike> {
    let mut cur = doc.as_item().as_table_like()?;
    for seg in path {
        cur = cur.get(seg)?.as_table_like()?;
    }
    Some(cur)
}

fn push_edges(
    text: &str,
    doc: &Document<String>,
    rel: &str,
    path: &[&str],
    kind: &'static str,
    label: &str,
    out: &mut Vec<Edge>,
) {
    let Some(table) = table_at(doc, path) else {
        return;
    };
    for (key, item) in table.iter() {
        // `foo = "1.0"` or `foo = { .. }`
        let (req_raw, path_field, renamed, inherited) = match item.as_str() {
            Some(s) => (Some(s.to_string()), None, None, false),
            None => match item.as_table_like() {
                Some(t) => (
                    t.get("version")
                        .and_then(|i| i.as_str())
                        .map(str::to_string),
                    t.get("path").and_then(|i| i.as_str()).map(str::to_string),
                    t.get("package")
                        .and_then(|i| i.as_str())
                        .map(str::to_string),
                    t.get("workspace")
                        .and_then(|i| i.as_bool())
                        .unwrap_or(false),
                ),
                None => continue,
            },
        };
        let req = req_raw.as_deref().and_then(|s| VersionReq::parse(s).ok());
        out.push(Edge {
            manifest: rel.to_string(),
            line: key_line(text, doc, path, key),
            table: label.to_string(),
            table_path: path.iter().map(|s| s.to_string()).collect(),
            kind,
            key: key.to_string(),
            name: renamed.unwrap_or_else(|| key.to_string()),
            req_raw,
            req,
            path: path_field,
            inherited,
        });
    }
}

/// 1-indexed line of `key` inside the table at `path`.
///
/// Prefers the parsed key's span; falls back to a textual scan for the rare
/// case where a span is unavailable.
fn key_line(
    text: &str,
    doc: &Document<String>,
    path: &[&str],
    key: &str,
) -> usize {
    let span = if path.is_empty() {
        doc.as_item()
            .as_table_like()
            .and_then(|t| t.get_key_value(key))
            .and_then(|(k, _)| k.span())
    } else {
        table_at(doc, path)
            .and_then(|t| t.get_key_value(key))
            .and_then(|(k, _)| k.span())
    };
    if let Some(span) = span {
        return line_of(text, span.start);
    }
    for (i, line) in text.lines().enumerate() {
        let t = line.trim_start();
        let t = t.strip_prefix('"').unwrap_or(t);
        if let Some(rest) = t.strip_prefix(key) {
            let rest = rest.trim_start_matches('"').trim_start();
            if rest.starts_with('=') || rest.starts_with('.') {
                return i + 1;
            }
        }
    }
    1
}

/// 1-indexed line containing byte `offset`.
fn line_of(text: &str, offset: usize) -> usize {
    let offset = offset.min(text.len());
    text[..offset].bytes().filter(|b| *b == b'\n').count() + 1
}

/// Fill in `version` for packages that wrote `version.workspace = true`.
fn resolve_inherited_versions(manifests: &mut [Manifest]) {
    let roots: Vec<(String, Option<Version>)> = manifests
        .iter()
        .filter(|m| m.is_workspace_root)
        .map(|m| (parent_dir(&m.rel), m.workspace_version.clone()))
        .collect();

    for m in manifests.iter_mut() {
        let Some(pkg) = &mut m.package else { continue };
        if pkg.spec != VersionSpec::Inherited {
            continue;
        }
        let dir = parent_dir(&m.rel);
        let best = roots
            .iter()
            .filter(|(wdir, _)| {
                wdir.is_empty()
                    || &dir == wdir
                    || dir.starts_with(&format!("{wdir}/"))
            })
            .max_by_key(|(wdir, _)| wdir.len());
        pkg.version = best.and_then(|(_, v)| v.clone());
    }
}

fn parse_lockfile(root: &Path, path: &Path) -> Result<Lockfile, Error> {
    let (text, doc) = read_doc(path)?;
    let mut packages = Vec::new();
    if let Some(arr) = doc.get("package").and_then(|i| i.as_array_of_tables())
    {
        for t in arr.iter() {
            let name = t
                .get("name")
                .and_then(|i| i.as_str())
                .unwrap_or_default()
                .to_string();
            let version = t
                .get("version")
                .and_then(|i| i.as_str())
                .unwrap_or_default()
                .to_string();
            let has_source = t.contains_key("source");
            let line = t
                .get_key_value("name")
                .and_then(|(k, _)| k.span())
                .map(|s| line_of(&text, s.start))
                .unwrap_or(1);
            packages.push((name, version, has_source, line));
        }
    }
    Ok(Lockfile {
        rel: rel_path(root, path),
        packages,
    })
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}
