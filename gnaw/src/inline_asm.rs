//! The `inline-asm` check: inline assembly appears only where [`ALLOWED`]
//! says it may.
//!
//! The check lexes every Rust file in the repository and counts references
//! to the inline assembly macros in [`MACROS`]: each identifier token with
//! one of those names, wherever it appears. Comments do not count, and
//! string literals are not searched. Invocations, `use` imports (renamed or
//! not), and macro bodies such as `quote!` input all count. A file with an
//! assembly source extension counts as one reference.
//!
//! The files are those Git tracks, plus untracked files it does not ignore,
//! less untracked files matching `.local_src_ignore`. A clippy lint would
//! see only the code compiled for one target and feature set, and most
//! inline assembly here is guarded by `cfg(target_arch = "riscv32")`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::process::Command;

use proc_macro2::{Spacing, TokenStream, TokenTree};

use crate::repo::Repo;
use crate::task::{Ctx, Failure, Result};

/// The files that may use inline assembly, and exactly how many references
/// each holds. See the module documentation for what counts.
///
/// Do not add an entry or raise a count without maintainer approval. Prefer
/// plain Rust, a `core::arch` intrinsic, or an `extern "C"` import. When an
/// entry's file loses references, lower its count, or remove the entry.
pub const ALLOWED: &[(&str, usize)] = &[
    // `ebreak` in the rv32 panic handler, and its import.
    ("crates/rv32-runtime/src/panic.rs", 2),
    // The naked rv32 `_start` shim, which sets gp and sp, and its import.
    ("crates/rv32-runtime/src/start.rs", 2),
];

/// Inline assembly macro names. `llvm_asm` has been removed from Rust; it
/// is listed so that it cannot return unnoticed.
const MACROS: &[&str] = &["asm", "global_asm", "naked_asm", "llvm_asm"];

/// Git pathspecs for the files to search.
const PATHSPECS: &[&str] = &["*.rs", "*.s", "*.S", "*.asm"];

/// File extensions that mark assembly source.
const ASSEMBLY_EXTENSIONS: &[&str] = &["s", "S", "asm"];

/// One reference to inline assembly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub line: usize,
    /// 1-based, in characters.
    pub column: usize,
    pub what: String,
}

/// A file whose references disagree with [`ALLOWED`].
#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    /// More references than the allowlist allows (none, if it does not name
    /// the file).
    Excess {
        path: String,
        allowed: usize,
        sites: Vec<Site>,
    },
    /// Fewer references than the allowlist allows.
    Stale {
        path: String,
        allowed: usize,
        found: usize,
    },
    /// The file could not be read or lexed, so it could not be checked.
    Unchecked { path: String, error: String },
}

impl Problem {
    fn is_excess(&self) -> bool {
        matches!(self, Problem::Excess { .. })
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::Excess {
                path,
                allowed: 0,
                sites,
            } => {
                let lines: Vec<String> =
                    sites.iter().map(|s| format!("{path}:{s}")).collect();
                write!(f, "{}", lines.join("\n"))
            }
            Problem::Excess {
                path,
                allowed,
                sites,
            } => {
                write!(
                    f,
                    "{path}: {} references, {allowed} allowed:",
                    sites.len()
                )?;
                for site in sites {
                    write!(f, "\n  {path}:{site}")?;
                }
                Ok(())
            }
            Problem::Stale {
                path,
                allowed: _,
                found: 0,
            } => write!(
                f,
                "{path}: no references left; remove its ALLOWED entry"
            ),
            Problem::Stale {
                path,
                allowed,
                found,
            } => write!(
                f,
                "{path}: {found} references, {allowed} allowed; \
                 lower its ALLOWED count to {found}"
            ),
            Problem::Unchecked { path, error } => {
                write!(f, "{path}: could not check: {error}")
            }
        }
    }
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.what)
    }
}

/// The references in the repository's files, by path, and the files that
/// could not be checked.
struct Scan {
    files: usize,
    found: BTreeMap<String, Vec<Site>>,
    unchecked: Vec<Problem>,
}

fn scan(repo: &Repo) -> std::result::Result<Scan, Failure> {
    let paths = files(repo)?;
    if paths.is_empty() {
        return Err(Failure::new("git listed no files to search"));
    }
    let mut scan = Scan {
        files: paths.len(),
        found: BTreeMap::new(),
        unchecked: Vec::new(),
    };
    for path in paths {
        match file_references(repo, &path) {
            Ok(Some(sites)) if !sites.is_empty() => {
                scan.found.insert(path, sites);
            }
            Ok(_) => {}
            Err(error) => {
                scan.unchecked.push(Problem::Unchecked { path, error })
            }
        }
    }
    Ok(scan)
}

pub fn check(ctx: &mut Ctx) -> Result {
    let Scan {
        files,
        found,
        mut unchecked,
    } = scan(ctx.repo)?;
    ctx.log.line(&format!("searched {files} files"));
    for (path, sites) in &found {
        for site in sites {
            ctx.log.line(&format!("{path}:{site}"));
        }
    }
    let mut problems = compare(&found, ALLOWED);
    problems.append(&mut unchecked);
    if problems.is_empty() {
        return Ok(());
    }
    let message = if problems.iter().any(Problem::is_excess) {
        "inline assembly outside the allowed sites. Use plain Rust, a \
         core::arch intrinsic, or an extern \"C\" import instead; \
         extending ALLOWED in gnaw/src/inline_asm.rs needs maintainer \
         approval"
    } else if problems
        .iter()
        .any(|p| matches!(p, Problem::Unchecked { .. }))
    {
        "files the inline-asm check could not read or lex"
    } else {
        "ALLOWED in gnaw/src/inline_asm.rs is out of date"
    };
    let mut failure = Failure::new(message);
    failure.details = problems
        .iter()
        .flat_map(|p| {
            p.to_string()
                .lines()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .take(30)
        .collect();
    Err(failure.into())
}

/// Repository-relative paths of the files to search.
fn files(repo: &Repo) -> std::result::Result<BTreeSet<String>, Failure> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--exclude-from=.local_src_ignore",
            "--",
        ])
        .args(PATHSPECS)
        .current_dir(&repo.root)
        .output()
        .map_err(|e| {
            Failure::new(format!("could not run git ls-files: {e}"))
        })?;
    if !output.status.success() {
        return Err(Failure::new(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    // A set: during a merge, an unmerged path is listed once per stage.
    Ok(output
        .stdout
        .split(|&b| b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect())
}

/// The references in the file at `path`, or `None` if it does not exist: a
/// tracked file deleted from the working tree.
fn file_references(
    repo: &Repo,
    path: &str,
) -> std::result::Result<Option<Vec<Site>>, String> {
    let is_assembly = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ASSEMBLY_EXTENSIONS.contains(&e));
    let full = repo.root.join(path);
    if is_assembly {
        return Ok(full.is_file().then(|| {
            vec![Site {
                line: 1,
                column: 1,
                what: "assembly source file".to_string(),
            }]
        }));
    }
    match std::fs::read_to_string(&full) {
        Ok(source) => references(&source).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// The references to inline assembly macros in Rust `source`.
pub fn references(source: &str) -> std::result::Result<Vec<Site>, String> {
    let tokens: TokenStream = source
        .parse()
        .map_err(|e: proc_macro2::LexError| format!("lexing failed: {e}"))?;
    let mut sites = Vec::new();
    collect(tokens, &mut sites);
    Ok(sites)
}

fn collect(tokens: TokenStream, sites: &mut Vec<Site>) {
    // A lifetime lexes as a joint `'` and then an identifier.
    let mut lifetime = false;
    for tree in tokens {
        let after_quote = std::mem::replace(
            &mut lifetime,
            matches!(
                &tree,
                TokenTree::Punct(p)
                    if p.as_char() == '\'' && p.spacing() == Spacing::Joint
            ),
        );
        match tree {
            TokenTree::Group(group) => collect(group.stream(), sites),
            TokenTree::Ident(_) if after_quote => {}
            TokenTree::Ident(ident) => {
                let name = ident.to_string();
                let name = name.strip_prefix("r#").unwrap_or(&name);
                if MACROS.contains(&name) {
                    let start = ident.span().start();
                    sites.push(Site {
                        line: start.line,
                        column: start.column + 1,
                        what: format!("`{name}`"),
                    });
                }
            }
            TokenTree::Punct(_) | TokenTree::Literal(_) => {}
        }
    }
}

/// How the references `found`, by path, disagree with `allowed`.
pub fn compare(
    found: &BTreeMap<String, Vec<Site>>,
    allowed: &[(&str, usize)],
) -> Vec<Problem> {
    let allowed: BTreeMap<&str, usize> = allowed.iter().copied().collect();
    let mut problems = Vec::new();
    for (path, sites) in found {
        let limit = allowed.get(path.as_str()).copied().unwrap_or(0);
        if sites.len() > limit {
            problems.push(Problem::Excess {
                path: path.clone(),
                allowed: limit,
                sites: sites.clone(),
            });
        }
    }
    for (&path, &limit) in &allowed {
        let count = found.get(path).map_or(0, Vec::len);
        if count < limit {
            problems.push(Problem::Stale {
                path: path.to_string(),
                allowed: limit,
                found: count,
            });
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(source: &str) -> Vec<(usize, String)> {
        references(source)
            .unwrap()
            .into_iter()
            .map(|s| (s.line, s.what))
            .collect()
    }

    #[test]
    fn finds_each_macro_and_its_import() {
        let source = r#"
use core::arch::asm;
fn f() {
    unsafe { asm!("nop") }
    unsafe { core::arch::asm!("nop") }
    unsafe { ::std::arch::asm ! ("nop") }
}
core::arch::global_asm!(".globl x");
#[unsafe(naked)]
extern "C" fn g() { core::arch::naked_asm!("ret") }
fn h() { unsafe { llvm_asm!("nop") } }
"#;
        assert_eq!(
            names(source),
            [
                (2, "`asm`".to_string()),
                (4, "`asm`".to_string()),
                (5, "`asm`".to_string()),
                (6, "`asm`".to_string()),
                (8, "`global_asm`".to_string()),
                (10, "`naked_asm`".to_string()),
                (11, "`llvm_asm`".to_string()),
            ]
        );
    }

    #[test]
    fn finds_renamed_and_generated_uses() {
        let source = r#"
use core::arch::{asm as emit, global_asm};
macro_rules! trap { () => { core::arch::asm!("ebreak") } }
fn gen() -> TokenStream { quote! { ::core::arch::asm!("nop") } }
fn raw() { unsafe { r#asm!("nop") } }
"#;
        let lines: Vec<usize> =
            names(source).into_iter().map(|(line, _)| line).collect();
        assert_eq!(lines, [2, 2, 3, 4, 5]);
    }

    #[test]
    fn ignores_comments_strings_and_longer_names() {
        let source = r##"
// asm!("nop") in a comment
/* nested /* asm!("nop") */ still a comment */
/// Docs that mention `asm!` and `naked_asm!`.
//! Inner docs that mention `global_asm!`.
const S: &str = "asm!(\"nop\")";
const R: &str = r#"core::arch::asm!("nop")"#;
const B: &[u8] = b"asm!";
fn asmx() {}
fn my_asm() {}
struct Asm;
fn lifetimes<'asm>(x: &'asm u8) -> char { let _ = x; 'a' }
"##;
        assert_eq!(names(source), []);
    }

    #[test]
    fn reports_columns_from_one() {
        let sites = references("fn f() { unsafe { asm!(\"nop\") } }").unwrap();
        assert_eq!(sites.len(), 1);
        assert_eq!((sites[0].line, sites[0].column), (1, 19));
    }

    #[test]
    fn rejects_source_it_cannot_lex() {
        assert!(references("fn f() { \"unterminated }").is_err());
    }

    fn site(line: usize) -> Site {
        Site {
            line,
            column: 1,
            what: "`asm`".to_string(),
        }
    }

    fn found(entries: &[(&str, usize)]) -> BTreeMap<String, Vec<Site>> {
        entries
            .iter()
            .map(|&(path, n)| (path.to_string(), (1..=n).map(site).collect()))
            .collect()
    }

    #[test]
    fn accepts_exact_counts() {
        let allowed = [("a.rs", 2), ("b.rs", 1)];
        assert_eq!(compare(&found(&allowed), &allowed), []);
    }

    #[test]
    fn rejects_a_new_file() {
        let problems =
            compare(&found(&[("a.rs", 2), ("new.rs", 1)]), &[("a.rs", 2)]);
        assert_eq!(
            problems,
            [Problem::Excess {
                path: "new.rs".to_string(),
                allowed: 0,
                sites: vec![site(1)],
            }]
        );
        assert_eq!(problems[0].to_string(), "new.rs:1:1: `asm`");
    }

    #[test]
    fn rejects_a_new_site_in_an_allowed_file() {
        let problems = compare(&found(&[("a.rs", 3)]), &[("a.rs", 2)]);
        assert!(matches!(
            &problems[..],
            [Problem::Excess { allowed: 2, sites, .. }] if sites.len() == 3
        ));
        assert_eq!(
            problems[0].to_string(),
            "a.rs: 3 references, 2 allowed:\n  a.rs:1:1: `asm`\n  \
             a.rs:2:1: `asm`\n  a.rs:3:1: `asm`"
        );
    }

    #[test]
    fn rejects_stale_entries() {
        let problems =
            compare(&found(&[("a.rs", 1)]), &[("a.rs", 2), ("gone.rs", 1)]);
        assert_eq!(
            problems,
            [
                Problem::Stale {
                    path: "a.rs".to_string(),
                    allowed: 2,
                    found: 1,
                },
                Problem::Stale {
                    path: "gone.rs".to_string(),
                    allowed: 1,
                    found: 0,
                },
            ]
        );
        assert!(!problems.iter().any(Problem::is_excess));
        assert_eq!(
            problems[1].to_string(),
            "gone.rs: no references left; remove its ALLOWED entry"
        );
    }

    /// Every file lexes, and the allowlist matches the working tree.
    #[test]
    fn repository_matches_the_allowlist() {
        let scan = scan(&Repo::locate().unwrap()).unwrap();
        assert_eq!(scan.unchecked, []);
        assert_eq!(compare(&scan.found, ALLOWED), []);
    }
}
