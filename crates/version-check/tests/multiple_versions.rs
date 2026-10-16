//! Multiple local releases must resolve by dependency path, not name alone.

use std::fs;
use std::path::Path;
use version_check::bump::{self, Request};
use version_check::model::Repo;
use version_check::{Violation, checks};

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "Cargo.toml",
        "[workspace]\n[workspace.package]\nversion = \"0.2.0\"\n",
    );
    for (folder, version) in [("v1", "0.1.0"), ("v2", "0.2.0")] {
        write(
            dir.path(),
            &format!("{folder}/Cargo.toml"),
            &format!(
                "[package]\nname = \"interface\"\nversion = \"{version}\"\n"
            ),
        );
    }
    write(
        dir.path(),
        "consumer/Cargo.toml",
        r#"[package]
name = "consumer"
version.workspace = true
[dependencies]
old = { package = "interface", path = "../v1", version = "=0.1.0" }
new = { package = "interface", path = "../v2", version = "=0.2.0" }
"#,
    );
    write(
        dir.path(),
        "Cargo.lock",
        r#"version = 4
[[package]]
name = "interface"
version = "0.1.0"
[[package]]
name = "interface"
version = "0.2.0"
"#,
    );
    dir
}

fn version_violations(repo: &Repo) -> Vec<Violation> {
    checks::all(repo)
        .into_iter()
        .filter(|v| matches!(v.check, "in-repo-req" | "lockfile"))
        .collect()
}

#[test]
fn renamed_dependencies_and_lockfile_retain_both_versions() {
    let dir = fixture();
    let repo = Repo::load(dir.path()).unwrap();
    assert_eq!(repo.locals["interface"].len(), 2);
    assert!(version_violations(&repo).is_empty());
    let plan = bump::plan(
        &repo,
        &Request {
            sync: true,
            ..Request::default()
        },
    )
    .unwrap();
    assert!(plan.is_empty());
}

#[test]
fn a_pin_matching_the_other_release_is_rejected_and_synced_to_its_path() {
    let dir = fixture();
    let manifest = dir.path().join("consumer/Cargo.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    fs::write(&manifest, text.replace("=0.1.0", "=0.2.0")).unwrap();
    let repo = Repo::load(dir.path()).unwrap();
    let violations = version_violations(&repo);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].check, "in-repo-req");
    assert!(violations[0].message.contains("v1/Cargo.toml"));

    let plan = bump::plan(
        &repo,
        &Request {
            sync: true,
            ..Request::default()
        },
    )
    .unwrap();
    assert_eq!(plan.edits.len(), 1);
    assert_eq!(plan.edits[0].old, "=0.2.0");
    assert_eq!(plan.edits[0].new, "=0.1.0");
}

#[test]
fn lockfiles_still_reject_unknown_versions_and_registry_sources() {
    let dir = fixture();
    write(
        dir.path(),
        "Cargo.lock",
        r#"version = 4
[[package]]
name = "interface"
version = "0.3.0"
[[package]]
name = "interface"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#,
    );
    let repo = Repo::load(dir.path()).unwrap();
    let violations = version_violations(&repo);
    assert_eq!(violations.len(), 2);
    assert!(violations.iter().all(|v| v.check == "lockfile"));
    assert!(violations.iter().any(|v| v.message.contains("0.3.0")));
    assert!(violations.iter().any(|v| v.message.contains("registry")));
}
