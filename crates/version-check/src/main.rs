//! `cargo run -p version-check [-- <repo-root>]`
//!
//! Exits 0 when every pin agrees, 1 on any violation, 2 if the repository could
//! not be read. Intended to become a subcommand of the eventual `xtask`.

use std::path::PathBuf;
use std::process::ExitCode;

use version_check::model::Repo;

fn main() -> ExitCode {
    let root = match std::env::args().nth(1) {
        Some(arg) => PathBuf::from(arg),
        None => version_check::repo_root().to_path_buf(),
    };

    let repo = match Repo::load(&root) {
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
