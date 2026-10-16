//! `cargo run -p version-check [-- [OPTIONS] [<repo-root>]]`
//!
//! Exits 0 when every pin agrees, 1 on any violation, 2 on a usage error or if
//! the repository could not be read. Intended to become a subcommand of the
//! eventual `xtask`.

use semver::Version;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use version_check::model::{LoadOptions, Repo};

const USAGE: &str = "\
usage: version-check [OPTIONS] [<repo-root>]
       version-check bump <patch|breaking|X.Y.Z> [OPTIONS] [<repo-root>]
       version-check sync [OPTIONS] [<repo-root>]

Checks that version pins agree across every manifest and lockfile, applies
coordinated version bumps, and repairs pins that have fallen behind.

check options:
  --ignore-lockfiles   Do not read lockfiles, so the `lockfile` check is
                       skipped. Useful mid-bump, when the manifests have been
                       edited but the lockfiles have not been regenerated yet.

bump <spec>:
  patch                Patch-bump the SDK. Leaves the standard interfaces
                       alone: their caret `thruput-sdk` pin still contains the new
                       version.
  breaking             Semver-incompatible SDK bump (minor while the major is
                       0). Also patch-bumps the standard interfaces and
                       refreshes their `thruput-sdk` pins.
  X.Y.Z                Bump the SDK to exactly this version.

sync:
  Repoint every in-repo pin at the version its package already declares and
  regenerate the lockfiles, moving no version line. This is the repair for a
  workspace that a release left behind -- typically one added on a branch, which
  no release run on the default branch could reach. Unlike `bump`, it does not
  require the repository to be consistent to begin with, because that is the
  state it exists to resolve.

bump options:
  --protocol X.Y.Z     Move the protocol-rs crates to this version. Implies an
                       SDK bump (patch unless <spec> says otherwise) and a
                       standard-interfaces patch bump.
  --allow-dirty        Bump even with uncommitted changes to tracked files, or
                       with edits landing in untracked manifests. Untracked
                       files alone never block.
  --skip-lockfiles     Edit manifests but do not run `cargo update`. The
                       lockfile check is skipped to match.

`--allow-dirty` and `--skip-lockfiles` apply to `sync` too.

  -h, --help           Show this message.

Defaults to the repository this binary was built from.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("bump") {
        return bump_main(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("sync") {
        return sync_main(&args[1..]);
    }

    let mut root = None;
    let mut opts = LoadOptions::default();

    for arg in args {
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

/// Load, run every check, and report. Used as both the bump's precondition and
/// its postcondition — a bump is only correct if the repo was consistent before
/// and is consistent after.
fn verify(
    root: &Path,
    opts: LoadOptions,
    stage: &str,
) -> Result<(), ExitCode> {
    let repo = match Repo::load_with(root, opts) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("version-check: {e}");
            return Err(ExitCode::from(2));
        }
    };
    let violations = version_check::checks::all(&repo);
    if violations.is_empty() {
        println!("{stage}: all pins agree");
        return Ok(());
    }
    eprintln!("\n{}", version_check::report(&violations));
    eprintln!("{stage}: refusing to continue");
    Err(ExitCode::FAILURE)
}

fn bump_main(args: &[String]) -> ExitCode {
    use version_check::bump::{self, Kind, Request};

    let mut req = Request::default();
    let mut root = None;
    let mut spec = None;
    let mut allow_dirty = false;
    let mut skip_lockfiles = false;

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--allow-dirty" => allow_dirty = true,
            "--skip-lockfiles" => skip_lockfiles = true,
            "--protocol" => match it.next().map(|v| v.parse::<Version>()) {
                Some(Ok(v)) => req.protocol = Some(v),
                _ => {
                    eprintln!("version-check: --protocol needs a version");
                    return ExitCode::from(2);
                }
            },
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            flag if flag.starts_with('-') => {
                eprintln!("version-check: unknown option `{flag}`\n\n{USAGE}");
                return ExitCode::from(2);
            }
            other if spec.is_none() => spec = Some(other.to_string()),
            other if root.is_none() => root = Some(PathBuf::from(other)),
            extra => {
                eprintln!("version-check: unexpected argument `{extra}`");
                return ExitCode::from(2);
            }
        }
    }

    match spec.as_deref() {
        Some("patch") => req.sdk = Some(Kind::Patch),
        Some("breaking") => req.sdk = Some(Kind::Breaking),
        Some(v) => match v.parse::<Version>() {
            Ok(v) => req.sdk_target = Some(v),
            Err(_) => {
                eprintln!(
                    "version-check: `{v}` is not `patch`, `breaking`, or a version"
                );
                return ExitCode::from(2);
            }
        },
        // `bump --protocol X` alone is meaningful: it implies a patch SDK bump.
        None if req.protocol.is_some() => {}
        None => {
            eprintln!("version-check: bump needs a spec\n\n{USAGE}");
            return ExitCode::from(2);
        }
    }

    let root =
        root.unwrap_or_else(|| version_check::repo_root().to_path_buf());
    let repo = match Repo::load(&root) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("version-check: {e}");
            return ExitCode::from(2);
        }
    };
    println!("version-check: {}", version_check::scope_summary(&repo));

    let worktree = match bump::worktree(&repo) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("version-check: {e}");
            return ExitCode::from(2);
        }
    };
    if !allow_dirty && let Err(e) = bump::require_clean_worktree(&worktree) {
        eprintln!("version-check: {e}");
        return ExitCode::from(2);
    }

    // Bumping a repository that already disagrees with itself would bury the
    // original problem in ~25 unrelated edits.
    if let Err(code) = verify(&root, LoadOptions::default(), "before") {
        return code;
    }

    let plan = match bump::plan(&repo, &req) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("version-check: {e}");
            return ExitCode::from(2);
        }
    };
    if plan.is_empty() {
        println!("nothing to bump");
        return ExitCode::SUCCESS;
    }
    print!("{}", bump::describe(&plan));

    if !allow_dirty
        && let Err(e) = bump::require_targets_tracked(&worktree, &plan)
    {
        eprintln!("version-check: {e}");
        return ExitCode::from(2);
    }

    if let Err(e) = bump::apply(&repo, &plan) {
        eprintln!("version-check: {e}");
        return ExitCode::from(2);
    }

    // Manifests are edited but the lockfiles have not caught up yet.
    let staged = LoadOptions {
        ignore_lockfiles: true,
    };
    if let Err(code) = verify(&root, staged, "manifests") {
        return code;
    }

    if skip_lockfiles {
        println!(
            "lockfiles skipped; run `cargo update --workspace` per workspace"
        );
        return ExitCode::SUCCESS;
    }
    if let Err(e) = bump::update_lockfiles(&repo, &plan) {
        eprintln!("version-check: {e}");
        return ExitCode::FAILURE;
    }
    match verify(&root, LoadOptions::default(), "after") {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

/// `sync` repairs a repository whose pins have fallen behind the versions its
/// packages declare, which is what a release leaves in any workspace it could
/// not reach — a workspace added on a branch, most often, since a release run
/// on the default branch never saw that directory.
///
/// It deliberately omits the bump's `before` verify: a repository that already
/// disagrees with itself is the input, not a reason to refuse. The `after`
/// verify still runs, so a sync that cannot fully repair the repo fails loudly
/// with the remaining violations.
fn sync_main(args: &[String]) -> ExitCode {
    use version_check::bump::{self, Request};

    let mut root = None;
    let mut allow_dirty = false;
    let mut skip_lockfiles = false;

    for arg in args {
        match arg.as_str() {
            "--allow-dirty" => allow_dirty = true,
            "--skip-lockfiles" => skip_lockfiles = true,
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
                eprintln!("version-check: unexpected argument `{extra}`");
                return ExitCode::from(2);
            }
        }
    }

    let root =
        root.unwrap_or_else(|| version_check::repo_root().to_path_buf());
    let repo = match Repo::load(&root) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("version-check: {e}");
            return ExitCode::from(2);
        }
    };
    println!("version-check: {}", version_check::scope_summary(&repo));

    let worktree = match bump::worktree(&repo) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("version-check: {e}");
            return ExitCode::from(2);
        }
    };
    if !allow_dirty && let Err(e) = bump::require_clean_worktree(&worktree) {
        eprintln!("version-check: {e}");
        return ExitCode::from(2);
    }

    let plan = match bump::plan(
        &repo,
        &Request {
            sync: true,
            ..Request::default()
        },
    ) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("version-check: {e}");
            return ExitCode::from(2);
        }
    };

    // An empty plan is not the end of it: a lockfile can be stale on its own,
    // with every manifest pin already correct.
    if plan.is_empty() {
        println!("no stale pins");
    } else {
        print!("{}", bump::describe(&plan));
        if !allow_dirty
            && let Err(e) = bump::require_targets_tracked(&worktree, &plan)
        {
            eprintln!("version-check: {e}");
            return ExitCode::from(2);
        }
        if let Err(e) = bump::apply(&repo, &plan) {
            eprintln!("version-check: {e}");
            return ExitCode::from(2);
        }
        let staged = LoadOptions {
            ignore_lockfiles: true,
        };
        if let Err(code) = verify(&root, staged, "manifests") {
            return code;
        }
    }

    if skip_lockfiles {
        println!(
            "lockfiles skipped; run `cargo update --workspace` per workspace"
        );
        return ExitCode::SUCCESS;
    }
    if let Err(e) = bump::update_lockfiles(&repo, &plan) {
        eprintln!("version-check: {e}");
        return ExitCode::FAILURE;
    }
    match verify(&root, LoadOptions::default(), "after") {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}
