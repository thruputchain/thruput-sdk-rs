//! `cargo run -p version-check [-- <repo-root>]`
//!
//! Exits 0 when every pin agrees, 1 on any violation, 2 if the repository could
//! not be read. Intended to become a subcommand of the eventual `xtask`.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = match std::env::args().nth(1) {
        Some(arg) => PathBuf::from(arg),
        None => version_check::repo_root().to_path_buf(),
    };

    let violations = match version_check::check(&root) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("version-check: {e}");
            return ExitCode::from(2);
        }
    };

    if violations.is_empty() {
        println!("version-check: all pins agree");
        return ExitCode::SUCCESS;
    }
    print!("{}", version_check::report(&violations));
    ExitCode::FAILURE
}
