//! Commands on named contracts, outside the task graph.
//!
//! `build` and `lint` share the tasks' code and log to
//! `target/gnaw/logs/<command>.log`. `test`, `expand`, and `tree` hand the
//! terminal to cargo.

use std::io::IsTerminal;
use std::process::ExitCode;
use std::sync::Arc;

use crate::contract::Contract;
use crate::repo::{RV32_TARGET, Repo};
use crate::report::print_failure;
use crate::rv32;
use crate::task::{Cmd, Ctx, Failure, Log, RunOptions, Stop};
use crate::tasks::{lint_contract, on_path};

/// Looks up each of `specs`, or prints why one is not a contract.
pub fn contracts(repo: &Repo, specs: &[String]) -> Option<Vec<Contract>> {
    let mut contracts = Vec::new();
    for spec in specs {
        match repo.contract(spec) {
            Ok(contract) => contracts.push(contract),
            Err(e) => {
                eprintln!("gnaw: {e}");
                return None;
            }
        }
    }
    Some(contracts)
}

pub fn build(repo: &Repo, contracts: &[Contract], locked: bool) -> ExitCode {
    in_ctx(repo, "build", locked, |ctx| {
        if !on_path("thruput") {
            return Err(Failure::new(
                "Thruput CLI not found: run `cargo gnaw setup`",
            )
            .into());
        }
        for c in contracts {
            ctx.step(c.package.as_str(), |ctx| {
                rv32::build(ctx, &c.workspace, &c.package)
            })?;
        }
        Ok(())
    })
}

pub fn lint(repo: &Repo, contracts: &[Contract], locked: bool) -> ExitCode {
    in_ctx(repo, "lint", locked, |ctx| {
        for c in contracts {
            ctx.step(c.package.as_str(), |ctx| {
                lint_contract(ctx, &c.workspace, &c.package)
            })?;
        }
        Ok(())
    })
}

/// Each contract's native tests, one package at a time: testing several
/// together can change behavior through feature unification. `args` go to
/// the test binaries.
pub fn test(
    repo: &Repo,
    contracts: &[Contract],
    args: &[String],
    locked: bool,
) -> ExitCode {
    for c in contracts {
        let mut cmd =
            Cmd::cargo(&c.workspace, "test", locked).args(["-p", &c.package]);
        if !args.is_empty() {
            cmd = cmd.arg("--").args(args);
        }
        if !passthrough(repo, &cmd) {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

/// The contract's macro expansion as the guest build sees it. Needs
/// `cargo-expand`.
pub fn expand(repo: &Repo, contract: &Contract, args: &[String]) -> ExitCode {
    let cmd = Cmd::new("cargo", &contract.workspace)
        .args([
            "expand",
            "-p",
            &contract.package,
            "-F",
            "entrypoints",
            "--target",
            RV32_TARGET,
        ])
        .args(args);
    exit_code(passthrough(repo, &cmd))
}

/// The contract's guest dependency tree: no proc-macro, build, or dev
/// edges, and the standard library as the guest build compiles it.
pub fn tree(
    repo: &Repo,
    contract: &Contract,
    args: &[String],
    locked: bool,
) -> ExitCode {
    let cmd = Cmd::cargo(&contract.workspace, "tree", locked)
        .args([
            "-p",
            &contract.package,
            "--target",
            RV32_TARGET,
            "--edges",
            "no-proc-macro,no-build,no-dev",
            "-Zbuild-std=core,alloc",
            "-Zbuild-std-features=std/optimize_for_size",
        ])
        .args(args);
    exit_code(passthrough(repo, &cmd))
}

/// Runs `cmd` on this terminal. Says what ran when it fails.
fn passthrough(repo: &Repo, cmd: &Cmd) -> bool {
    let line = cmd.display(repo);
    eprintln!("$ {line}");
    match cmd.command().status() {
        Ok(status) if status.success() => true,
        Ok(status) => {
            eprintln!("gnaw: failed ({status}): {line}");
            false
        }
        Err(e) => {
            eprintln!("gnaw: could not run {line}: {e}");
            false
        }
    }
}

/// Runs `body` as a task would, with its output both logged and printed.
fn in_ctx(
    repo: &Repo,
    command: &str,
    locked: bool,
    body: impl FnOnce(&mut Ctx) -> crate::task::Result,
) -> ExitCode {
    let path = repo.logs().join(format!("{command}.log"));
    let log = match Log::create(path.clone(), true) {
        Ok(log) => Arc::new(log),
        Err(e) => {
            eprintln!("gnaw: creating {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };
    let opts = RunOptions {
        locked,
        color: std::io::stdout().is_terminal(),
    };
    let mut ctx = Ctx::new(repo, &opts, log);
    let result = body(&mut ctx);
    for warning in ctx.into_warnings() {
        println!("warning: {warning}");
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Stop::Fail(failure)) => {
            let args: Vec<String> = std::env::args().skip(1).collect();
            let rerun = format!("cargo gnaw {}", args.join(" "));
            print_failure(command, &failure, &path, &rerun, repo);
            ExitCode::FAILURE
        }
        Err(Stop::Skip(reason)) => {
            eprintln!("gnaw: {reason}");
            ExitCode::FAILURE
        }
    }
}

fn exit_code(ok: bool) -> ExitCode {
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
