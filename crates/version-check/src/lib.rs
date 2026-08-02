//! Checks that in-repo version pins agree across every manifest and lockfile.
//!
//! This repository releases most crates in lockstep at one version, spread
//! over six workspaces and six lockfiles, and a release involves editing many
//! cargo files by hand. `cargo` reports at most one inconsistency at a time,
//! and only for the workspace it happens to be resolving. This crate reports
//! all of them at once.
//!
//! See `README.md` for the checklist and the reasoning behind each one.
//!
//! The entry point is [`check`]. `main.rs` is a thin wrapper.

pub mod checks;
pub mod model;
pub mod policy;

use std::path::Path;

pub use checks::Violation;
pub use model::Error;

/// The repository root, as known at compile time.
///
/// (Correct for both the test and the binary when built from this workspace.)
pub fn repo_root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// Load every manifest and lockfile under `root` and run all checks.
pub fn check(root: &Path) -> Result<Vec<Violation>, Error> {
    let repo = model::Repo::load(root)?;
    Ok(checks::all(&repo))
}

/// What the run actually covered.
///
/// Always reported, so that a submodule which is not checked out shows up as
/// reduced coverage rather than reading as "everything is fine".
pub fn scope_summary(repo: &model::Repo) -> String {
    use std::fmt::Write as _;

    let mut out = format!(
        "{} manifests, {} lockfiles",
        repo.manifests.len(),
        repo.lockfiles.len()
    );
    if !repo.included_submodules.is_empty() {
        let _ = write!(
            out,
            "\n  submodules scanned: {}",
            repo.included_submodules.join(", ")
        );
    }
    if !repo.absent_submodules.is_empty() {
        let _ = write!(
            out,
            "\n  submodules not checked out, so not scanned: {}",
            repo.absent_submodules.join(", ")
        );
    }
    out
}

/// Render violations as a report, grouped by file in repository order.
pub fn report(violations: &[Violation]) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    let mut current = String::new();
    for v in violations {
        // A location is `file:line`, or a bare `file` for a violation that
        // belongs to no particular line (a stale policy entry, say).
        let (file, line) = match v.location.rsplit_once(':') {
            Some((f, l))
                if !l.is_empty() && l.bytes().all(|b| b.is_ascii_digit()) =>
            {
                (f, Some(l))
            }
            _ => (v.location.as_str(), None),
        };
        if file != current {
            if !current.is_empty() {
                out.push('\n');
            }
            let _ = writeln!(out, "{file}");
            current = file.to_string();
        }
        match line {
            Some(line) => {
                let _ = writeln!(out, "  {line}: [{}] {}", v.check, v.message);
            }
            None => {
                let _ = writeln!(out, "  [{}] {}", v.check, v.message);
            }
        }
        for note in &v.note {
            let _ = writeln!(out, "      {note}");
        }
    }
    let _ = writeln!(
        out,
        "\n{} violation{}",
        violations.len(),
        if violations.len() == 1 { "" } else { "s" }
    );
    out
}
