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
