use std::fs;
use std::path::Path;
use version_check::model::{LoadOptions, Repo};
use version_check::{Violation, checks};

const CRATES: &[&str] = &[
    "groundhog-contract-spec",
    "groundhog-contract-util",
    "groundhog-domain-separator",
];
const LOCKFILE: &str = "conformance/sep41/Cargo.lock";

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn fixture(
    lockfile: &str,
    version: &str,
    registry: bool,
) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Cargo.toml", "[workspace]\n");
    let mut lock = "version = 4\n".to_owned();
    for name in CRATES {
        write(
            dir.path(),
            &format!("crates/{name}/Cargo.toml"),
            &format!("[package]\nname = \"{name}\"\nversion = \"0.42.1\"\n"),
        );
        lock.push_str(&format!(
            "[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n"
        ));
        if registry {
            lock.push_str("source = \"registry+https://packages.groundhogchain.com/index\"\n");
        }
    }
    write(dir.path(), lockfile, &lock);
    dir
}

fn violations(root: &Path) -> Vec<Violation> {
    checks::all(&Repo::load(root).unwrap())
        .into_iter()
        .filter(|v| v.check == "lockfile")
        .collect()
}

#[test]
fn released_testkit_transitives_are_allowed_only_in_the_conformance_lockfile()
{
    let dir = fixture(LOCKFILE, "0.41.1", true);
    assert!(violations(dir.path()).is_empty());
    let dir = fixture("other/Cargo.lock", "0.41.1", true);
    let errors = violations(dir.path());
    assert_eq!(errors.len(), 3);
    assert!(errors.iter().all(|v| v.message.contains("registry")));
}

#[test]
fn other_registry_versions_and_current_local_versions_remain_rejected() {
    for version in ["0.41.2", "0.42.1"] {
        let dir = fixture(LOCKFILE, version, true);
        let errors = violations(dir.path());
        assert_eq!(errors.len(), 6);
        assert_eq!(
            errors
                .iter()
                .filter(|v| v.message.contains("registry"))
                .count(),
            3
        );
        assert_eq!(errors.iter().filter(|v| !v.note.is_empty()).count(), 3);
    }
}

#[test]
fn other_local_crates_are_not_excepted() {
    let dir = fixture(LOCKFILE, "0.41.1", true);
    write(
        dir.path(),
        "crates/other/Cargo.toml",
        "[package]\nname = \"other\"\nversion = \"0.42.1\"\n",
    );
    let path = dir.path().join(LOCKFILE);
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("[[package]]\nname = \"other\"\nversion = \"0.41.1\"\nsource = \"registry+https://packages.groundhogchain.com/index\"\n");
    fs::write(path, text).unwrap();
    let errors = violations(dir.path());
    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("`other`"));
    assert!(errors[0].message.contains("registry"));
}

#[test]
fn exceptions_do_not_allow_wrong_path_versions() {
    let dir = fixture(LOCKFILE, "0.41.1", false);
    let errors = violations(dir.path());
    assert_eq!(errors.len(), 6);
    assert_eq!(
        errors
            .iter()
            .filter(|v| v.message.contains("local manifests"))
            .count(),
        3
    );
}

#[test]
fn a_testkit_upgrade_that_removes_old_versions_reports_stale_exceptions() {
    let dir = fixture(LOCKFILE, "0.42.1", false);
    let errors = violations(dir.path());
    assert_eq!(errors.len(), 3);
    assert!(errors.iter().all(|v| v.note[0].contains("stale exception")));
}

#[test]
fn removing_an_excepted_crate_reports_its_stale_entry() {
    let dir = fixture(LOCKFILE, "0.41.1", true);
    write(dir.path(), LOCKFILE, "version = 4\n");
    assert_eq!(violations(dir.path()).len(), 3);
}

#[test]
fn ignoring_lockfiles_does_not_report_stale_exceptions() {
    let dir = fixture(LOCKFILE, "0.42.1", false);
    let repo = Repo::load_with(
        dir.path(),
        LoadOptions {
            ignore_lockfiles: true,
        },
    )
    .unwrap();
    assert!(checks::all(&repo).iter().all(|v| v.check != "lockfile"));
}
