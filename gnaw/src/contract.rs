//! Finding a contract package from a path or a name.

use std::fs;
use std::path::{Path, PathBuf};

use crate::repo::{Repo, table_entries};

pub struct Contract {
    pub package: String,
    /// The package directory.
    pub dir: PathBuf,
    /// Where cargo runs: the workspace whose config holds the rv32 flags.
    pub workspace: PathBuf,
}

impl Repo {
    /// Contract directories, each with the workspace its packages build in.
    fn contract_dirs(&self) -> [(PathBuf, PathBuf); 3] {
        [
            (self.fixtures(), self.fixtures()),
            (self.integration_contracts(), self.integration()),
            (self.negative_fixtures(), self.negative_fixtures()),
        ]
    }

    /// The contract at `spec`, a package directory or a package name.
    pub fn contract(&self, spec: &str) -> Result<Contract, String> {
        let path = Path::new(spec);
        if path.join("Cargo.toml").is_file() {
            return self.contract_at(path);
        }
        if path.components().count() > 1 {
            return Err(format!("{spec} has no Cargo.toml"));
        }
        let mut found = Vec::new();
        for (dir, _) in self.contract_dirs() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let dir = entry.path();
                if package_name(&dir).as_deref() == Some(spec) {
                    found.push(dir);
                }
            }
        }
        match found.as_slice() {
            [dir] => self.contract_at(dir),
            [] => Err(format!("no contract package named {spec}")),
            _ => Err(format!(
                "{spec} names several packages; give a path: {}",
                found
                    .iter()
                    .map(|d| self.rel(d).display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    fn contract_at(&self, path: &Path) -> Result<Contract, String> {
        let dir = path
            .canonicalize()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let package = package_name(&dir).ok_or_else(|| {
            format!("{} has no [package] name", path.display())
        })?;
        let parent = dir.parent().unwrap_or(&dir);
        let workspace = self
            .contract_dirs()
            .into_iter()
            .find(|(contracts, _)| {
                contracts.canonicalize().is_ok_and(|c| c == parent)
            })
            .map(|(_, workspace)| workspace)
            .ok_or_else(|| {
                format!(
                    "{} is not in a contract directory ({})",
                    path.display(),
                    self.contract_dirs()
                        .iter()
                        .map(|(d, _)| self.rel(d).display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;
        let manifest = fs::read_to_string(dir.join("Cargo.toml"))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        // A package outside its directory's workspace is its own.
        let workspace = if has_table(&manifest, "workspace") {
            dir.clone()
        } else {
            workspace
        };
        Ok(Contract {
            package,
            dir,
            workspace,
        })
    }
}

/// The `[package]` name in `dir/Cargo.toml`.
fn package_name(dir: &Path) -> Option<String> {
    let manifest = fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    table_entries(&manifest, "package")
        .into_iter()
        .find(|&(key, _)| key == "name")
        .map(|(_, value)| value.trim_matches('"').to_string())
}

fn has_table(manifest: &str, name: &str) -> bool {
    manifest
        .lines()
        .any(|line| line.trim() == format!("[{name}]"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> Repo {
        Repo::locate().unwrap()
    }

    #[test]
    fn finds_by_name_and_path() {
        let repo = repo();
        let by_name = repo.contract("counter").unwrap();
        assert_eq!(by_name.package, "counter");
        assert_eq!(by_name.workspace, repo.fixtures());
        let path = repo.fixtures().join("counter");
        let by_path = repo.contract(path.to_str().unwrap()).unwrap();
        assert_eq!(by_path.dir, by_name.dir);
    }

    #[test]
    fn reads_the_package_name() {
        let repo = repo();
        let path = repo.fixtures().join("versioned_interface_v2");
        let contract = repo.contract(path.to_str().unwrap()).unwrap();
        assert_eq!(contract.package, "versioned_interface");
        assert_eq!(contract.workspace, contract.dir);
    }

    #[test]
    fn rejects_unknown_names() {
        assert!(repo().contract("no_such_contract").is_err());
        assert!(repo().contract("crates/sdk").is_err());
    }
}
