//! Tests for the release-workflow order check against fixture repositories.
//!
//! Each test builds a miniature repo — a root workspace, three crates, and a
//! `publish.yml` listing them — then asserts what `checks::all` says about
//! the list order. The dependency that matters is a dev-dependency: that is
//! the edge the real 0.28.0 release tripped over, and the one a reader is
//! most likely to assume does not count.

use std::fs;
use std::path::Path;

use version_check::checks;
use version_check::model::Repo;

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// A repo where `mid` dev-depends on `leaf`, plus a `top` that depends on
/// `mid` normally. `order` becomes both package lists in the workflow.
fn fixture(dir: &Path, order: &[&str]) {
    let listed = order
        .iter()
        .map(|p| format!("    {p}\n"))
        .collect::<String>();
    let vendored = order
        .iter()
        .map(|p| format!("            {p}\n"))
        .collect::<String>();

    write(
        dir,
        "Cargo.toml",
        r#"[workspace]
members = ["crates/leaf", "crates/mid", "crates/top"]
default-members = ["crates/leaf", "crates/mid", "crates/top"]

[workspace.package]
version = "0.28.0"

[workspace.dependencies]
leaf = { path = "crates/leaf", version = "=0.28.0" }
mid = { path = "crates/mid", version = "=0.28.0" }
"#,
    );
    write(
        dir,
        "crates/leaf/Cargo.toml",
        "[package]\nname = \"leaf\"\nversion.workspace = true\n",
    );
    write(
        dir,
        "crates/mid/Cargo.toml",
        "[package]\nname = \"mid\"\nversion.workspace = true\n\n\
         [dev-dependencies]\nleaf = { workspace = true }\n",
    );
    write(
        dir,
        "crates/top/Cargo.toml",
        "[package]\nname = \"top\"\nversion.workspace = true\n\n\
         [dependencies]\nmid = { workspace = true }\n",
    );
    write(
        dir,
        ".github/workflows/publish.yml",
        &format!(
            "env:
  PRIMARY_PACKAGES: >-
{listed}
  INTERFACE_PACKAGES: >-
    leaf

  INTERFACE_DEPENDENT_PACKAGES: >-
    top

jobs:
  publish:
    steps:
      - name: Publish dry run
        with:
          vendor-workspace-deps:
{vendored}"
        ),
    );
}

fn violations(dir: &Path) -> Vec<String> {
    let repo = Repo::load(dir).unwrap();
    checks::all(&repo)
        .into_iter()
        .filter(|v| v.check == "publish-order")
        .map(|v| v.message)
        .collect()
}

#[test]
fn dependency_order_is_clean() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), &["leaf", "mid", "top"]);
    assert_eq!(violations(dir.path()), Vec::<String>::new());
}

#[test]
fn a_dev_dependency_listed_later_is_a_violation() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), &["mid", "leaf", "top"]);
    let found = violations(dir.path());
    // Once for the publish list, once for the vendor list.
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(
        found.iter().all(|m| m.contains("`mid` depends on `leaf`")),
        "{found:#?}"
    );
}

#[test]
fn a_normal_dependency_listed_later_is_a_violation() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), &["leaf", "top", "mid"]);
    let found = violations(dir.path());
    assert_eq!(found.len(), 2, "{found:#?}");
    assert!(
        found.iter().all(|m| m.contains("`top` depends on `mid`")),
        "{found:#?}"
    );
}

#[test]
fn a_vendor_list_omission_is_a_violation() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), &["leaf", "mid", "top"]);
    let workflow = dir.path().join(".github/workflows/publish.yml");
    let text = fs::read_to_string(&workflow).unwrap();
    fs::write(&workflow, text.replace("            leaf\n", "")).unwrap();

    let found = violations(dir.path());
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].contains("omits"), "{found:#?}");
}

#[test]
fn a_publish_list_may_omit_a_dependency_an_earlier_step_published() {
    // INTERFACE_DEPENDENT_PACKAGES lists only `top`, whose `mid` dependency
    // ships in the earlier PRIMARY_PACKAGES step. That is how the real
    // workflow is staged, and must not be reported.
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), &["leaf", "mid", "top"]);
    assert_eq!(violations(dir.path()), Vec::<String>::new());
}

#[test]
fn a_path_dependency_without_a_version_is_not_required() {
    // `cargo package` strips these, so they never need vendoring first.
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), &["leaf", "mid", "top"]);
    write(
        dir.path(),
        "crates/mid/Cargo.toml",
        "[package]\nname = \"mid\"\nversion.workspace = true\n\n\
         [dev-dependencies]\nleaf = { path = \"../leaf\" }\n",
    );
    let workflow = dir.path().join(".github/workflows/publish.yml");
    let text = fs::read_to_string(&workflow).unwrap();
    fs::write(&workflow, text.replace("            leaf\n", "")).unwrap();

    assert_eq!(violations(dir.path()), Vec::<String>::new());
}

#[test]
fn a_repo_without_the_workflow_is_not_checked() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), &["leaf", "mid", "top"]);
    fs::remove_dir_all(dir.path().join(".github")).unwrap();
    assert_eq!(violations(dir.path()), Vec::<String>::new());
}

#[test]
fn a_missing_workflow_beside_its_directory_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), &["leaf", "mid", "top"]);
    fs::remove_file(dir.path().join(".github/workflows/publish.yml")).unwrap();
    let found = violations(dir.path());
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].contains("unreadable"), "{found:#?}");
}
