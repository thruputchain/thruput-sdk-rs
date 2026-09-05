//! Round-trip tests for the bump planner against fixture repositories.
//!
//! Each test builds a miniature repo in a temp dir — a root workspace, two
//! workspace-family crates, one standard-interface crate, and one
//! standard-impls component riding the interface version line — then plans a
//! bump, applies it, reloads, and asserts both the rewritten values and that
//! `checks::all` is clean afterwards (the checks are the bump's declared
//! postcondition).

use std::fs;
use std::path::Path;

use semver::Version;
use version_check::bump::{self, Kind, Request};
use version_check::model::Repo;
use version_check::{checks, policy};

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// A minimal repository the checks pass on: a root workspace at 0.17.4, a
/// `tool` crate pinning `sdk` exactly, a standard-interface crate with its
/// own version line and a caret-family `sdk` pin, and a standard-impls
/// component on the same interface version line that exact-pins the
/// interface crate.
///
/// The `.gitmodules` entry mirrors the real repo so the agreement exceptions
/// scoped to `integration_test/contracts/` count as unscanned rather than
/// stale.
fn fixture(dir: &Path) {
    write(
        dir,
        ".gitmodules",
        "[submodule \"integration_test_contracts\"]\n\
         \tpath = integration_test/contracts\n\
         \turl = ssh://git@example.com/contracts.git\n",
    );
    write(
        dir,
        "Cargo.toml",
        r#"[workspace]
members = ["crates/sdk", "crates/tool"]
default-members = ["crates/sdk", "crates/tool"]

[workspace.package]
version = "0.17.4"

[workspace.dependencies]
sdk = { path = "crates/sdk", version = "=0.17.4" }
groundhog-xdr = { version = "=0.21.0" }
"#,
    );
    write(
        dir,
        "crates/sdk/Cargo.toml",
        r#"[package]
name = "sdk"
version.workspace = true

[dependencies]
groundhog-xdr = { workspace = true }
"#,
    );
    write(
        dir,
        "crates/tool/Cargo.toml",
        r#"[package]
name = "tool"
version.workspace = true

[dependencies]
sdk = { path = "../sdk", version = "=0.17.4" }
"#,
    );
    write(
        dir,
        "standard-interfaces/erc20-if/Cargo.toml",
        r#"[package]
name = "groundhog-erc20-if"
version = "0.4.2"

[dependencies]
sdk = { path = "../../crates/sdk", version = "0.17.0" }
"#,
    );
    write(
        dir,
        "standard-impls/access-control/Cargo.toml",
        r#"[package]
name = "groundhog-access-control"
version = "0.4.2"

[dependencies]
sdk = { path = "../../crates/sdk", version = "0.17.0" }
groundhog-erc20-if = { path = "../../standard-interfaces/erc20-if", version = "=0.4.2" }
"#,
    );
}

fn load(dir: &Path) -> Repo {
    Repo::load(dir).unwrap()
}

fn req_of<'a>(repo: &'a Repo, manifest: &str, key: &str) -> &'a str {
    repo.edges()
        .find(|e| e.manifest == manifest && e.key == key)
        .unwrap_or_else(|| panic!("no `{key}` edge in {manifest}"))
        .req_raw
        .as_deref()
        .unwrap()
}

fn interface_version(repo: &Repo) -> Version {
    repo.manifest("standard-interfaces/erc20-if/Cargo.toml")
        .unwrap()
        .package
        .as_ref()
        .unwrap()
        .version
        .clone()
        .unwrap()
}

fn assert_clean(repo: &Repo) {
    let violations = checks::all(repo);
    assert!(
        violations.is_empty(),
        "checks not clean after bump: {violations:#?}"
    );
}

#[test]
fn bumped_computes_patch_and_breaking() {
    let v = Version::new(0, 17, 4);
    assert_eq!(bump::bumped(&v, Kind::Patch), Version::new(0, 17, 5));
    // Breaking while major is 0 moves the minor.
    assert_eq!(bump::bumped(&v, Kind::Breaking), Version::new(0, 18, 0));
    // Breaking at major >= 1 moves the major.
    let v1 = Version::new(1, 2, 3);
    assert_eq!(bump::bumped(&v1, Kind::Patch), Version::new(1, 2, 4));
    assert_eq!(bump::bumped(&v1, Kind::Breaking), Version::new(2, 0, 0));
}

#[test]
fn fixture_is_clean_before_any_bump() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    assert_clean(&load(tmp.path()));
}

#[test]
fn patch_bump_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    let repo = load(tmp.path());

    let plan = bump::plan(
        &repo,
        &Request {
            sdk: Some(Kind::Patch),
            ..Request::default()
        },
    )
    .unwrap();

    let ws = plan.workspace.as_ref().unwrap();
    assert_eq!(ws.old, Version::new(0, 17, 4));
    assert_eq!(ws.new, Version::new(0, 17, 5));
    // A patch bump must leave the standard interfaces alone.
    assert!(plan.interfaces.is_none());
    assert!(plan.protocol.is_none());
    // Workspace version + root workspace.dependencies pin + tool pin.
    assert_eq!(plan.edits.len(), 3);
    assert_eq!(plan.workspace_roots, vec!["Cargo.toml".to_string()]);

    bump::apply(&repo, &plan).unwrap();

    let after = load(tmp.path());
    let root = after.manifest("Cargo.toml").unwrap();
    assert_eq!(root.workspace_version, Some(Version::new(0, 17, 5)));
    assert_eq!(req_of(&after, "Cargo.toml", "sdk"), "=0.17.5");
    assert_eq!(req_of(&after, "crates/tool/Cargo.toml", "sdk"), "=0.17.5");
    // Interface line and its caret-family sdk pin untouched — the
    // standard-impls component included.
    assert_eq!(interface_version(&after), Version::new(0, 4, 2));
    assert_eq!(
        req_of(&after, "standard-interfaces/erc20-if/Cargo.toml", "sdk"),
        "0.17.0"
    );
    assert_eq!(
        req_of(&after, "standard-impls/access-control/Cargo.toml", "sdk"),
        "0.17.0"
    );
    assert_eq!(
        req_of(
            &after,
            "standard-impls/access-control/Cargo.toml",
            "groundhog-erc20-if"
        ),
        "=0.4.2"
    );
    assert_clean(&after);
}

#[test]
fn breaking_bump_moves_interfaces_and_refreshes_sdk_pins() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    let repo = load(tmp.path());

    let plan = bump::plan(
        &repo,
        &Request {
            sdk: Some(Kind::Breaking),
            ..Request::default()
        },
    )
    .unwrap();

    assert_eq!(plan.workspace.as_ref().unwrap().new, Version::new(0, 18, 0));
    let ifc = plan.interfaces.as_ref().unwrap();
    assert_eq!(ifc.old, Version::new(0, 4, 2));
    assert_eq!(ifc.new, Version::new(0, 4, 3));

    bump::apply(&repo, &plan).unwrap();

    let after = load(tmp.path());
    let root = after.manifest("Cargo.toml").unwrap();
    assert_eq!(root.workspace_version, Some(Version::new(0, 18, 0)));
    assert_eq!(req_of(&after, "crates/tool/Cargo.toml", "sdk"), "=0.18.0");
    assert_eq!(interface_version(&after), Version::new(0, 4, 3));
    // The caret-family pin is refreshed to the new workspace version, and the
    // requirement keeps its operator (a bare caret stays bare).
    assert_eq!(
        req_of(&after, "standard-interfaces/erc20-if/Cargo.toml", "sdk"),
        "0.18.0"
    );
    // The standard-impls component rides the interface line: its version and
    // caret sdk pin move like an interface crate's, and its exact pin on the
    // interface crate tracks the new interface version.
    assert_eq!(
        after
            .manifest("standard-impls/access-control/Cargo.toml")
            .unwrap()
            .package
            .as_ref()
            .unwrap()
            .version
            .clone()
            .unwrap(),
        Version::new(0, 4, 3)
    );
    assert_eq!(
        req_of(&after, "standard-impls/access-control/Cargo.toml", "sdk"),
        "0.18.0"
    );
    assert_eq!(
        req_of(
            &after,
            "standard-impls/access-control/Cargo.toml",
            "groundhog-erc20-if"
        ),
        "=0.4.3"
    );
    assert_clean(&after);
}

#[test]
fn explicit_target_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    let repo = load(tmp.path());

    let plan = bump::plan(
        &repo,
        &Request {
            sdk_target: Some(Version::new(1, 0, 0)),
            ..Request::default()
        },
    )
    .unwrap();

    // 0.17.4 -> 1.0.0 is semver-incompatible, so the interfaces move too.
    assert_eq!(plan.workspace.as_ref().unwrap().new, Version::new(1, 0, 0));
    assert_eq!(plan.interfaces.as_ref().unwrap().new, Version::new(0, 4, 3));

    bump::apply(&repo, &plan).unwrap();

    let after = load(tmp.path());
    let root = after.manifest("Cargo.toml").unwrap();
    assert_eq!(root.workspace_version, Some(Version::new(1, 0, 0)));
    assert_eq!(req_of(&after, "Cargo.toml", "sdk"), "=1.0.0");
    assert_eq!(req_of(&after, "crates/tool/Cargo.toml", "sdk"), "=1.0.0");
    assert_eq!(
        req_of(&after, "standard-interfaces/erc20-if/Cargo.toml", "sdk"),
        "1.0.0"
    );
    assert_clean(&after);
}

#[test]
fn protocol_bump_implies_sdk_patch_and_interface_bump() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    let repo = load(tmp.path());

    assert!(policy::PROTOCOL_CRATES.contains(&"groundhog-xdr"));
    let plan = bump::plan(
        &repo,
        &Request {
            protocol: Some(Version::new(0, 22, 0)),
            ..Request::default()
        },
    )
    .unwrap();

    assert_eq!(plan.workspace.as_ref().unwrap().new, Version::new(0, 17, 5));
    let proto = plan.protocol.as_ref().unwrap();
    assert_eq!(proto.old, Version::new(0, 21, 0));
    assert_eq!(proto.new, Version::new(0, 22, 0));
    assert!(plan.interfaces.is_some());

    bump::apply(&repo, &plan).unwrap();

    let after = load(tmp.path());
    assert_eq!(req_of(&after, "Cargo.toml", "groundhog-xdr"), "=0.22.0");
    assert_eq!(req_of(&after, "Cargo.toml", "sdk"), "=0.17.5");
    assert_eq!(interface_version(&after), Version::new(0, 4, 3));
    assert_clean(&after);
}

#[test]
fn no_request_plans_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    let repo = load(tmp.path());

    let plan = bump::plan(&repo, &Request::default()).unwrap();
    assert!(plan.is_empty());
    assert!(plan.workspace.is_none());
    assert!(plan.interfaces.is_none());
    assert!(plan.protocol.is_none());
}

#[test]
fn multi_part_requirement_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    // A ranged pin cannot be retargeted by operator preservation; the plan
    // must refuse rather than guess.
    write(
        tmp.path(),
        "crates/tool/Cargo.toml",
        r#"[package]
name = "tool"
version.workspace = true

[dependencies]
sdk = { path = "../sdk", version = ">=0.17, <0.18" }
"#,
    );
    let repo = load(tmp.path());

    let err = bump::plan(
        &repo,
        &Request {
            sdk: Some(Kind::Patch),
            ..Request::default()
        },
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("multi-part requirement"),
        "unexpected error: {err}"
    );
}

#[test]
fn diverged_interface_versions_are_refused() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    // A second interface crate at a different version: the interfaces move
    // together, so they must start together.
    write(
        tmp.path(),
        "standard-interfaces/erc20-pausable-if/Cargo.toml",
        r#"[package]
name = "groundhog-erc20-pausable-if"
version = "0.4.1"

[dependencies]
sdk = { path = "../../crates/sdk", version = "0.17.0" }
"#,
    );
    let repo = load(tmp.path());

    let err = bump::plan(
        &repo,
        &Request {
            sdk: Some(Kind::Breaking),
            ..Request::default()
        },
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("not on one version"),
        "unexpected error: {err}"
    );
}
