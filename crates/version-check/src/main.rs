//! `cargo run -p version-check [-- [OPTIONS] [<repo-root>]]`
//!
//! Exits 0 when every pin agrees, 1 on any violation, 2 on a usage error or if
//! the repository could not be read. Intended to become a subcommand of the
//! eventual `xtask`.

use std::path::PathBuf;
use std::process::ExitCode;

use version_check::model::{LoadOptions, Repo};

const USAGE: &str = "\
usage: version-check [OPTIONS] [<repo-root>]

Checks that version pins agree across every manifest and lockfile.

options:
  --ignore-lockfiles   Do not read lockfiles, so the `lockfile` check is
                       skipped. Useful mid-bump, when the manifests have been
                       edited but the lockfiles have not been regenerated yet.
  -h, --help           Show this message.

Defaults to the repository this binary was built from.";

fn main() -> ExitCode {
    let mut root = None;
    let mut opts = LoadOptions::default();

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--ignore-lockfiles" => opts.ignore_lockfiles = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            flag if flag.starts_with('-') => {
                eprintln!("version-check: unknown option `{flag}`\n\n{USAGE}");
                return ExitCode::from(2);
            }
            path if root.is_none() => root = Some(PathBuf::from(path)),
            extra => {
                eprintln!(
                    "version-check: unexpected argument `{extra}`\n\n{USAGE}"
                );
                return ExitCode::from(2);
            }
        }
    }

    let root =
        root.unwrap_or_else(|| version_check::repo_root().to_path_buf());
    let repo = match Repo::load_with(&root, opts) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("version-check: {e}");
            return ExitCode::from(2);
        }
    };

    println!("version-check: {}", version_check::scope_summary(&repo));

    let violations = version_check::checks::all(&repo);
    if violations.is_empty() {
        println!("all pins agree");
        return ExitCode::SUCCESS;
    }
    print!("\n{}", version_check::report(&violations));
    ExitCode::FAILURE
}
