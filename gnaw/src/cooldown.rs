//! `--cooldown`: cool each workspace's lockfile under the repository's
//! cooldown policy (`cooldown.toml`) before anything else runs.
//!
//! One pass per workspace is enough: a `Cargo.lock` covers the whole
//! workspace, for every feature and target. Afterwards the lockfiles agree
//! with the manifests, so the cargo calls that follow run with `--locked` and
//! fail rather than change a lockfile. Scripts that run cargo without
//! `--locked` leave a lockfile that already agrees unchanged.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::repo::Repo;
use crate::task::Cmd;
use crate::tasks::on_path;

/// The workspace roots: the directories of the lockfiles Git tracks.
///
/// The integration workspace's members live in a submodule, so it is left
/// out, with a note, when the submodule is not checked out.
pub fn workspaces(repo: &Repo) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("git")
        .args(["ls-files", "-z", "--", "*Cargo.lock"])
        .current_dir(&repo.root)
        .output()
        .map_err(|e| format!("could not run git ls-files: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut workspaces = Vec::new();
    for path in output.stdout.split(|&b| b == 0).filter(|p| !p.is_empty()) {
        let path = String::from_utf8_lossy(path);
        let dir = repo.root.join(&*path);
        let dir = dir.parent().expect("a lockfile path has a parent");
        if dir == repo.integration() && !repo.integration_present() {
            eprintln!(
                "gnaw: not cooling {}: {} is not checked out",
                repo.rel(dir).display(),
                repo.rel(&repo.integration_contracts()).display()
            );
            continue;
        }
        workspaces.push(dir.to_path_buf());
    }
    Ok(workspaces)
}

/// Cools the lockfile of each of `workspaces`, one at a time. Prints each
/// command; returns `false` after the first failure, saying why.
pub fn cool(repo: &Repo, workspaces: &[impl AsRef<Path>]) -> bool {
    if !on_path("cargo-cooldown") {
        eprintln!(
            "gnaw: --cooldown needs cargo-cooldown: run \
             `cargo install cargo-cooldown --version 0.3.5 --locked`"
        );
        return false;
    }
    for dir in workspaces {
        // `cargo cooldown` cools the lockfile before any cargo subcommand;
        // `locate-project` is one that does nothing else.
        let cmd = Cmd::new("cargo", dir.as_ref()).args([
            "cooldown",
            "locate-project",
            "--workspace",
        ]);
        let line = cmd.display(repo);
        eprintln!("$ {line}");
        // stdout is locate-project's JSON; cooldown reports on stderr.
        match cmd.command().stdout(Stdio::null()).status() {
            Ok(status) if status.success() => {}
            Ok(status) => {
                eprintln!("gnaw: failed ({status}): {line}");
                return false;
            }
            Err(e) => {
                eprintln!("gnaw: could not run {line}: {e}");
                return false;
            }
        }
    }
    true
}
