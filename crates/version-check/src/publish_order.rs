//! Checks that the release workflow's package lists are in dependency order.
//!
//! `.github/workflows/publish.yml` pre-vendors and publishes crates in
//! hand-maintained list order. Both passes run `cargo package`, which
//! resolves dev-dependencies alongside normal ones, so a crate whose
//! in-repo dependency sits later in its list dies mid-release with
//! "no matching package named `<dep>` found". `cargo` cannot warn earlier
//! here the way it eventually would for a bad pin: the order lives in YAML
//! it never reads, and the release workflow is dispatched by hand, so the
//! first run to exercise a new edge is usually a real release.

use std::collections::BTreeSet;
use std::fs;

use crate::checks::Violation;
use crate::model::{Edge, Manifest, Repo};

/// The release workflow, relative to the repository root.
pub const WORKFLOW: &str = ".github/workflows/publish.yml";

/// Holds [`WORKFLOW`]; its absence means this is not a checkout of this
/// repository, so the check has nothing to say.
const WORKFLOW_DIR: &str = ".github/workflows";

/// Env keys whose folded scalars list crates in publish order.
const PUBLISH_KEYS: [&str; 3] = [
    "PRIMARY_PACKAGES",
    "INTERFACE_PACKAGES",
    "INTERFACE_DEPENDENT_PACKAGES",
];

/// Step input listing the crates pre-vendored before an offline packaging
/// pass.
const VENDOR_KEY: &str = "vendor-workspace-deps";

/// What "in order" means for a given list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListKind {
    /// Pre-vendored into an initially empty `vendor/`, which is the only
    /// source packaging may consult, so every in-repo dependency has to
    /// appear earlier in the same list.
    Vendor,
    /// Published against the live registry, where a dependency may instead
    /// have been published by an earlier step or an earlier release; only an
    /// entry that comes later in this same list is wrong.
    Publish,
}

/// One ordered list of package names lifted out of the workflow.
#[derive(Debug)]
struct PackageList {
    key: String,
    kind: ListKind,
    /// Package name and the 1-based line it sits on, in list order.
    entries: Vec<(String, usize)>,
}

/// C9: every in-repo dependency is vendored and published before its
/// dependent.
pub fn check(repo: &Repo, out: &mut Vec<Violation>) {
    if !repo.root.join(WORKFLOW_DIR).is_dir() {
        return;
    }
    let Ok(text) = fs::read_to_string(repo.root.join(WORKFLOW)) else {
        out.push(violation(
            WORKFLOW.to_string(),
            format!("{WORKFLOW_DIR}/ exists but {WORKFLOW} is unreadable"),
            vec![
                "if the workflow moved, update publish_order::WORKFLOW"
                    .to_string(),
            ],
        ));
        return;
    };

    let lists = parse_lists(&text);
    for key in PUBLISH_KEYS.iter().copied().chain([VENDOR_KEY]) {
        if !lists.iter().any(|l| l.key == key) {
            out.push(violation(
                WORKFLOW.to_string(),
                format!("no `{key}` package list found"),
                vec![
                    "the check reads these lists by key; a rename here \
                     silently stops it checking them"
                        .to_string(),
                ],
            ));
        }
    }
    for list in &lists {
        check_list(repo, list, out);
    }
}

fn check_list(repo: &Repo, list: &PackageList, out: &mut Vec<Violation>) {
    let position =
        |name: &str| list.entries.iter().position(|(n, _)| n == name);

    for (at, (name, line)) in list.entries.iter().enumerate() {
        let where_ = format!("{WORKFLOW}:{line}");
        let Some(locals) = repo.locals.get(name) else {
            out.push(violation(
                where_,
                format!(
                    "`{name}` in `{}` is not an in-repo package",
                    list.key
                ),
                Vec::new(),
            ));
            continue;
        };
        let [local] = locals.as_slice() else {
            out.push(violation(
                where_,
                format!(
                    "`{name}` in `{}` matches multiple local packages; \
                     publish lists require a unique package name",
                    list.key
                ),
                Vec::new(),
            ));
            continue;
        };
        let Some(manifest) = repo.manifest(&local.manifest) else {
            continue;
        };

        for dep in required_deps(repo, manifest) {
            match position(&dep) {
                Some(j) if j > at => out.push(violation(
                    where_.clone(),
                    format!(
                        "`{name}` depends on `{dep}`, which `{}` lists after \
                         it",
                        list.key
                    ),
                    vec![kind_note(list.kind, &dep)],
                )),
                Some(_) => {}
                None if list.kind == ListKind::Vendor => out.push(violation(
                    where_.clone(),
                    format!(
                        "`{name}` depends on `{dep}`, which `{}` omits",
                        list.key
                    ),
                    vec![kind_note(list.kind, &dep)],
                )),
                None => {}
            }
        }
    }
}

fn kind_note(kind: ListKind, dep: &str) -> String {
    match kind {
        ListKind::Vendor => format!(
            "packaging resolves against vendor/ alone, so `{dep}` must be \
             pre-vendored first"
        ),
        ListKind::Publish => format!(
            "packaging resolves dev-dependencies too, so `{dep}` must be \
             published first"
        ),
    }
}

/// In-repo dependencies of `manifest`'s package that survive `cargo package`.
///
/// A `path` dependency carrying no version requirement is dropped from the
/// packaged manifest, so it never has to be vendored or published first.
/// Everything else does, dev- and build-dependencies included.
fn required_deps(repo: &Repo, manifest: &Manifest) -> BTreeSet<String> {
    let own = manifest.package.as_ref().map(|p| p.name.as_str());
    manifest
        .edges
        .iter()
        .filter(|e| !e.table.starts_with("workspace."))
        .filter(|e| Some(e.name.as_str()) != own)
        .filter(|e| repo.locals.contains_key(&e.name))
        .filter(|e| !is_stripped(repo, manifest, e))
        .map(|e| e.name.clone())
        .collect()
}

/// Whether `cargo package` drops this edge from the packaged manifest.
fn is_stripped(repo: &Repo, manifest: &Manifest, edge: &Edge) -> bool {
    let resolved = if edge.inherited {
        // The requirement lives in the owning workspace's table.
        repo.owning_workspace(&manifest.rel).and_then(|ws| {
            ws.edges.iter().find(|e| {
                e.table == "workspace.dependencies" && e.name == edge.name
            })
        })
    } else {
        Some(edge)
    };
    resolved.is_some_and(|e| e.req_raw.is_none() && e.path.is_some())
}

/// Extract the ordered package lists from workflow YAML.
///
/// A list is a `KEY: >-` or `key:` line followed by more-indented lines each
/// holding one bare package name. It ends at the first line that is not one,
/// so a comment or a blank line closes a list rather than being skipped —
/// which is what the workflow's own layout does.
fn parse_lists(text: &str) -> Vec<PackageList> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let Some((indent, key)) = header(lines[i]) else {
            i += 1;
            continue;
        };
        let kind = if key == VENDOR_KEY {
            ListKind::Vendor
        } else if PUBLISH_KEYS.contains(&key) {
            ListKind::Publish
        } else {
            i += 1;
            continue;
        };

        let mut entries = Vec::new();
        let mut j = i + 1;
        while j < lines.len() && indent_of(lines[j]) > indent {
            let Some(name) = bare_name(lines[j]) else {
                break;
            };
            entries.push((name.to_string(), j + 1));
            j += 1;
        }
        if !entries.is_empty() {
            out.push(PackageList {
                key: key.to_string(),
                kind,
                entries,
            });
        }
        i = j.max(i + 1);
    }
    out
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// A `key:` or `key: >-` line introducing a list, with its indent.
fn header(line: &str) -> Option<(usize, &str)> {
    let body = line.trim();
    let body = body.strip_suffix(">-").unwrap_or(body).trim_end();
    let key = body.strip_suffix(':')?;
    (!key.is_empty() && key.bytes().all(is_name_byte))
        .then_some((indent_of(line), key))
}

/// A list entry: one bare package name, alone on its line.
fn bare_name(line: &str) -> Option<&str> {
    let name = line.trim();
    (!name.is_empty() && name.bytes().all(is_name_byte)).then_some(name)
}

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn violation(
    location: String,
    message: String,
    note: Vec<String>,
) -> Violation {
    Violation {
        check: "publish-order",
        location,
        message,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKFLOW_SHAPE: &str = r#"
env:
  CARGO_NET_GIT_FETCH_WITH_CLI: "true"
  # Workspace-versioned crates, in dependency order.
  PRIMARY_PACKAGES: >-
    thruput-event
    thruput-macros
    sdk
  INTERFACE_PACKAGES: >-
    thruput-erc20-if

jobs:
  publish:
    steps:
      - name: Publish dry run
        uses: ./actions/cargo-publish-dry-run
        with:
          packages: ${{ env.PRIMARY_PACKAGES }}
          # Pre-vendored, in dependency order.
          vendor-workspace-deps:
            thruput-event
            sdk
          cargo-hack-feature-options: --feature-powerset
"#;

    fn list<'a>(lists: &'a [PackageList], key: &str) -> &'a PackageList {
        lists.iter().find(|l| l.key == key).expect("list present")
    }

    fn names(list: &PackageList) -> Vec<&str> {
        list.entries.iter().map(|(n, _)| n.as_str()).collect()
    }

    #[test]
    fn reads_every_list_in_order() {
        let lists = parse_lists(WORKFLOW_SHAPE);
        assert_eq!(lists.len(), 3);
        assert_eq!(
            names(list(&lists, "PRIMARY_PACKAGES")),
            ["thruput-event", "thruput-macros", "sdk"]
        );
        assert_eq!(
            names(list(&lists, "INTERFACE_PACKAGES")),
            ["thruput-erc20-if"]
        );
        assert_eq!(names(list(&lists, VENDOR_KEY)), ["thruput-event", "sdk"]);
    }

    #[test]
    fn classifies_lists_by_key() {
        let lists = parse_lists(WORKFLOW_SHAPE);
        assert_eq!(list(&lists, "PRIMARY_PACKAGES").kind, ListKind::Publish);
        assert_eq!(list(&lists, VENDOR_KEY).kind, ListKind::Vendor);
    }

    #[test]
    fn records_the_line_of_each_entry() {
        let lists = parse_lists(WORKFLOW_SHAPE);
        let primary = list(&lists, "PRIMARY_PACKAGES");
        let line = primary.entries[0].1;
        assert_eq!(
            WORKFLOW_SHAPE.lines().nth(line - 1).unwrap().trim(),
            "thruput-event"
        );
    }

    #[test]
    fn a_list_ends_at_the_next_key() {
        // `cargo-hack-feature-options` follows the vendor list and must not
        // be swallowed into it.
        let lists = parse_lists(WORKFLOW_SHAPE);
        assert_eq!(names(list(&lists, VENDOR_KEY)), ["thruput-event", "sdk"]);
    }

    #[test]
    fn ignores_keys_that_are_not_package_lists() {
        let lists = parse_lists(WORKFLOW_SHAPE);
        assert!(lists.iter().all(|l| {
            l.key == VENDOR_KEY || PUBLISH_KEYS.contains(&l.key.as_str())
        }));
    }

    #[test]
    fn a_renamed_key_yields_no_list() {
        let renamed = WORKFLOW_SHAPE.replace("PRIMARY_PACKAGES", "PRIMARIES");
        let lists = parse_lists(&renamed);
        assert!(!lists.iter().any(|l| l.key == "PRIMARY_PACKAGES"));
    }
}
