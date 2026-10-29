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

/// Contract packages in `dir`: subdirectories with a `Cargo.toml` and no
/// `disabled` marker, sorted. The package name is the directory name.
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
            && !path.join("disabled").exists()
            && name != "target"
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
/// The `disabled` marker keeps the negative fixtures out, although they
/// declare bins; the tasks that own them build them by name.
///
/// scripts/metrics/sdk_codesizes_rv32*.sh apply the same rules.
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
