//! rv32 contract builds, and the checks on the ELFs they link.

use std::path::{Path, PathBuf};

use crate::elf;
use crate::repo::{RV32_TARGET, Repo, rv32_packages, undeclared_bins};
use crate::task::{Cmd, Ctx, Failure, Result, Stop};

/// The linked ELF for `package`. Each contract declares its `[[bin]]` with
/// the package name in snake case.
pub fn elf_path(repo: &Repo, package: &str) -> PathBuf {
    repo.target()
        .join(RV32_TARGET)
        .join("release")
        .join(package.replace('-', "_"))
}

/// `thruput build` for `package` in `workspace`. The CLI compiles the ELF
/// with the workspace's rv32 rustflags and lowers it into
/// `target/thruput/<snake>.{rv32a,rv32.json,spec.json,spec.xdr}`.
///
/// build-std goes on the command line: the rustflags need a core built with
/// them, and `[unstable] build-std` in the config would break host builds,
/// which have no `--target`. The ELF is unstripped, so the dispatch check can
/// read its bracket symbols; the lowered `.rv32a` has no symbol table anyway.
pub fn build_cmd(ctx: &Ctx, workspace: &Path, package: &str) -> Cmd {
    let cmd = ctx
        .cmd("thruput", workspace)
        .args(["build", "--no-ui", "-p", package, "--"]);
    let cmd = if ctx.opts.locked {
        cmd.arg("--locked")
    } else {
        cmd
    };
    cmd.arg("-Zbuild-std=core,alloc")
        .env("CARGO_PROFILE_RELEASE_STRIP", "false")
}

/// Builds `package` and checks its ELF.
pub fn build(ctx: &mut Ctx, workspace: &Path, package: &str) -> Result {
    let cmd = build_cmd(ctx, workspace, package);
    ctx.run(&cmd)?;
    check_elf(ctx, package)
}

/// The dispatch, mem-override, and embedded-path checks for `package`.
pub fn check_elf(ctx: &mut Ctx, package: &str) -> Result {
    let path = elf_path(ctx.repo, package);
    let data = std::fs::read(&path).map_err(|e| {
        Failure::new(format!("reading {}: {e}", ctx.repo.rel(&path).display()))
    })?;
    let fail = |message: String| -> Stop { Failure::new(message).into() };
    if let Some(warning) = elf::check_dispatch(&data).map_err(fail)? {
        ctx.warn(format!("{package}: {warning}"));
    }
    elf::check_mem_symbols(&data).map_err(fail)?;
    let paths = elf::rs_paths(&data);
    if !paths.is_empty() {
        let mut failure = Failure::new("embedded .rs path strings");
        failure.details = paths;
        return Err(failure.into());
    }
    ctx.log.line(&format!("ELF checks passed: {package}"));
    Ok(())
}

/// Builds every rv32 contract in `contracts`, from `workspace`. On success,
/// lists them in `<list>.txt` under [`Repo::rv32_package_lists`].
pub fn build_all(
    ctx: &mut Ctx,
    workspace: &Path,
    contracts: &Path,
    list: &str,
) -> Result {
    let list = ctx.repo.rv32_package_lists().join(format!("{list}.txt"));
    let writing = |e: std::io::Error| -> Stop {
        Failure::new(format!("writing {}: {e}", list.display())).into()
    };
    // A failed build leaves no list to pair with its partial artifacts.
    match std::fs::remove_file(&list) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(writing(e));
        }
        _ => {}
    }
    let listing = |e: std::io::Error| -> Stop {
        Failure::new(format!("listing {}: {e}", contracts.display())).into()
    };
    ctx.step("explicit bins", |_| {
        let undeclared = undeclared_bins(contracts).map_err(listing)?;
        if undeclared.is_empty() {
            return Ok(());
        }
        let mut failure = Failure::new(
            "src/main.rs without [[bin]]: cargo would discover the bin \
             without required-features, so host builds would link it, and \
             the rv32 build would skip it",
        );
        failure.details = undeclared;
        Err(failure.into())
    })?;
    let packages = rv32_packages(contracts).map_err(listing)?;
    if packages.is_empty() {
        return Err(Failure::new(format!(
            "no rv32 contracts in {}",
            ctx.repo.rel(contracts).display()
        ))
        .into());
    }
    for package in &packages {
        ctx.step(package.as_str(), |ctx| build(ctx, workspace, package))?;
    }
    std::fs::create_dir_all(ctx.repo.rv32_package_lists()).map_err(writing)?;
    let mut contents = packages.join("\n");
    contents.push('\n');
    std::fs::write(&list, contents).map_err(writing)
}
