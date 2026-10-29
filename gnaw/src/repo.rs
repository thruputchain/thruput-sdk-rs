//! Repository layout and contract discovery.
//!
//! Directory names live here so that renaming a contract directory is one edit.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const RV32_TARGET: &str = "riscv32im-unknown-none-elf";

pub struct Repo {
    pub root: PathBuf,
}

impl Repo {
    /// Finds the repository root by walking up from the current directory.
    pub fn locate() -> Result<Repo, String> {
        let cwd = std::env::current_dir()
            .map_err(|e| format!("current directory: {e}"))?;
        cwd.ancestors()
            .find(|dir| {
                dir.join("gnaw/Cargo.toml").is_file()
                    && dir.join("crates/sdk").is_dir()
            })
            .map(|root| Repo {
                root: root.to_path_buf(),
            })
            .ok_or_else(|| {
                format!("{} is not inside the SDK repository", cwd.display())
            })
    }

    /// Contracts that build and run.
    pub fn fixtures(&self) -> PathBuf {
        self.root.join("example-contracts")
    }

    /// Contracts whose rv32 build must fail.
    pub fn negative_fixtures(&self) -> PathBuf {
        self.root.join("negative-contracts")
    }

    pub fn integration(&self) -> PathBuf {
        self.root.join("integration_test")
    }

    /// The integration contracts, a Git submodule.
    pub fn integration_contracts(&self) -> PathBuf {
        self.integration().join("contracts")
    }

    pub fn logging_test(&self) -> PathBuf {
        self.root.join("logging-test")
    }

    pub fn script(&self, name: &str) -> PathBuf {
        self.root.join("scripts").join(name)
    }

    pub fn target(&self) -> PathBuf {
        self.root.join("target")
    }

    /// Where `thruput build` writes lowered artifacts.
    pub fn lowered(&self) -> PathBuf {
        self.target().join("thruput")
    }

    /// gnaw's own output: task logs.
    pub fn logs(&self) -> PathBuf {
        self.target().join("gnaw/logs")
    }

    /// `path` relative to the repository root, for display.
    pub fn rel<'a>(&self, path: &'a Path) -> &'a Path {
        path.strip_prefix(&self.root).unwrap_or(path)
    }
}

/// How gnaw treats a contract package, from the `[package.metadata.gnaw]`
/// table in its manifest. Both keys are optional:
///
/// ```toml
/// [package.metadata.gnaw]
/// kind = "library"
/// rv32-artifact-vars = ["ERC20_RV32_ARTIFACT"]
/// ```
///
/// A virtual manifest, with no `[package]`, uses `[workspace.metadata.gnaw]`.
/// Each value must fit on one line.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Fixture {
    pub kind: Kind,
    /// Variables that make the package's tests use its rv32 artifact instead
    /// of the native contract. `fixtures:test` runs the tests again with each
    /// variable naming the artifact. Tests that always use an artifact
    /// (`tests/rv32.rs`) need none.
    pub rv32_artifact_vars: Vec<String>,
}

/// The tasks build, test, and lint only [`Kind::Contract`] packages. The
/// tasks that own packages of other kinds build them by name.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A contract, built for rv32 when it declares a `[[bin]]`. The default.
    #[default]
    Contract,
    /// An interface, client, or shared-type library with no contract of its
    /// own. Its dependents build it.
    Library,
    /// A package whose build must fail.
    Negative,
    /// Built only by scripts/bigint-bench.sh.
    Benchmark,
    /// Set aside: nothing builds it.
    Disabled,
}

const METADATA_TABLES: [&str; 2] =
    ["package.metadata.gnaw", "workspace.metadata.gnaw"];

impl Fixture {
    /// Reads `dir/Cargo.toml`.
    pub fn read(dir: &Path) -> io::Result<Fixture> {
        let path = dir.join("Cargo.toml");
        Fixture::parse(&fs::read_to_string(&path)?).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: [*.metadata.gnaw]: {e}", path.display()),
            )
        })
    }

    fn parse(manifest: &str) -> Result<Fixture, String> {
        let mut fixture = Fixture::default();
        let entries = METADATA_TABLES
            .into_iter()
            .flat_map(|table| table_entries(manifest, table));
        for (key, value) in entries {
            match key {
                "kind" => fixture.kind = Kind::parse(string(value)?)?,
                "rv32-artifact-vars" => {
                    fixture.rv32_artifact_vars = strings(value)?
                }
                _ => return Err(format!("unknown key {key}")),
            }
        }
        Ok(fixture)
    }
}

impl Kind {
    fn parse(name: &str) -> Result<Kind, String> {
        Ok(match name {
            "contract" => Kind::Contract,
            "library" => Kind::Library,
            "negative" => Kind::Negative,
            "benchmark" => Kind::Benchmark,
            "disabled" => Kind::Disabled,
            _ => return Err(format!("unknown kind {name:?}")),
        })
    }
}

/// The `key = value` lines in a manifest's `[table]`, trimmed. Enough for
/// the one-line values gnaw reads.
pub(crate) fn table_entries<'a>(
    manifest: &'a str,
    table: &str,
) -> Vec<(&'a str, &'a str)> {
    let header = format!("[{table}]");
    let mut in_table = false;
    let mut entries = Vec::new();
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_table = line == header;
        } else if in_table
            && !line.starts_with('#')
            && let Some((key, value)) = line.split_once('=')
        {
            entries.push((key.trim(), value.trim()));
        }
    }
    entries
}

/// A quoted string value, without escapes.
fn string(value: &str) -> Result<&str, String> {
    value
        .strip_prefix('"')
        .and_then(|rest| rest.split_once('"'))
        .filter(|(_, after)| is_end(after))
        .map(|(s, _)| s)
        .ok_or_else(|| format!("expected a string: {value}"))
}

/// An array of strings.
fn strings(value: &str) -> Result<Vec<String>, String> {
    let (items, _) = value
        .strip_prefix('[')
        .and_then(|rest| rest.split_once(']'))
        .filter(|(_, after)| is_end(after))
        .ok_or_else(|| format!("expected an array on one line: {value}"))?;
    items
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| string(item).map(str::to_string))
        .collect()
}

/// Whether `rest` of a line holds at most a comment.
fn is_end(rest: &str) -> bool {
    let rest = rest.trim();
    rest.is_empty() || rest.starts_with('#')
}

/// Contract packages in `dir`: subdirectories whose `Cargo.toml` has
/// [`Kind::Contract`], sorted. The package name is the directory name.
pub fn contract_packages(dir: &Path) -> io::Result<Vec<String>> {
    let mut packages = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if path.join("Cargo.toml").is_file()
            && name != "target"
            && Fixture::read(&path)?.kind == Kind::Contract
        {
            packages.push(name);
        }
    }
    packages.sort();
    Ok(packages)
}

/// Contract packages in `dir` that declare a `[[bin]]`: each hosts its rv32
/// bin at `src/main.rs`, and declaring it is what enables the rv32 build.
///
/// Negative fixtures declare bins, but are not [`Kind::Contract`]; the tasks
/// that own them build them by name.
///
/// scripts/metrics/sdk_codesizes_rv32*.sh read this list through
/// `cargo gnaw packages --rv32`.
pub fn rv32_packages(dir: &Path) -> io::Result<Vec<String>> {
    let mut packages = Vec::new();
    for package in contract_packages(dir)? {
        if declares_bin(&fs::read_to_string(
            dir.join(&package).join("Cargo.toml"),
        )?) {
            packages.push(package);
        }
    }
    Ok(packages)
}

/// Contract packages in `dir` with a `src/main.rs` but no `[[bin]]`.
///
/// The bin must be explicit. Cargo would discover it without
/// `required-features`, so a host `cargo test` would link the `#![no_main]`
/// bin and fail; and [`rv32_packages`] would leave the package out of the
/// rv32 build, its checks, and the metrics. `autobins = false` turns
/// discovery off and makes a bare `src/main.rs` inert.
pub fn undeclared_bins(dir: &Path) -> io::Result<Vec<String>> {
    let mut packages = Vec::new();
    for package in contract_packages(dir)? {
        let path = dir.join(&package);
        if !path.join("src/main.rs").is_file() {
            continue;
        }
        let manifest = fs::read_to_string(path.join("Cargo.toml"))?;
        if !declares_bin(&manifest) && !disables_autobins(&manifest) {
            packages.push(package);
        }
    }
    Ok(packages)
}

fn declares_bin(manifest: &str) -> bool {
    manifest
        .lines()
        .any(|line| line.trim_start().starts_with("[[bin]]"))
}

fn disables_autobins(manifest: &str) -> bool {
    manifest.lines().any(|line| {
        line.split_once('=').is_some_and(|(key, value)| {
            key.trim() == "autobins" && value.trim() == "false"
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_metadata_table() {
        let manifest = r#"
[package]
name = "erc20"
kind = "ignored"

[package.metadata.gnaw]
# A comment.
kind = "library"  # Another.
rv32-artifact-vars = ["A", "B",]

[[bin]]
kind = "ignored"
"#;
        assert_eq!(
            Fixture::parse(manifest).unwrap(),
            Fixture {
                kind: Kind::Library,
                rv32_artifact_vars: vec!["A".into(), "B".into()],
            }
        );
    }

    #[test]
    fn defaults_to_a_contract() {
        let fixture = Fixture::parse("[package]\nname = \"x\"\n").unwrap();
        assert_eq!(fixture, Fixture::default());
        assert_eq!(fixture.kind, Kind::Contract);
    }

    #[test]
    fn reads_a_virtual_manifest() {
        let manifest = "[workspace]\n\n[workspace.metadata.gnaw]\n\
                        kind = \"negative\"\n";
        assert_eq!(Fixture::parse(manifest).unwrap().kind, Kind::Negative);
    }

    #[test]
    fn rejects_unknown_and_malformed_entries() {
        for table in [
            "kind = \"librray\"",
            "kind = library",
            "kinds = \"library\"",
            "rv32-artifact-vars = \"A\"",
            "rv32-artifact-vars = [\"A\",",
            "rv32-artifact-vars = [A]",
        ] {
            let manifest = format!("[package.metadata.gnaw]\n{table}\n");
            assert!(Fixture::parse(&manifest).is_err(), "{table}");
        }
    }
}
