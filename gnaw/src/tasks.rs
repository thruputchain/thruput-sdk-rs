//! Every task and group.
//!
//! Registry order is also run order among tasks that are ready at the same
//! time, so cheap checks that fail often come first.

use std::path::Path;

use regex::Regex;

use crate::repo::{Fixture, RV32_TARGET, contract_packages};
use crate::rv32::{self, elf_path};
use crate::task::{
    Cmd, Ctx, Failure, Lane, Result, Task, notable_lines, strip_ansi,
};
use crate::{elf, specs};

pub struct Group {
    pub name: &'static str,
    pub about: &'static str,
    /// Task ids or other group names.
    pub members: &'static [&'static str],
}

pub static GROUPS: &[Group] = &[
    Group {
        name: "ci",
        about: "Everything CI runs",
        members: &["quick", "sdk", "fixtures", "integration"],
    },
    Group {
        name: "quick",
        about: "Fast checks: formatting, annotations, versions, SDK lint",
        members: &["fmt", "annotations", "version-check", "lint:sdk"],
    },
    Group {
        name: "sdk",
        about: "SDK tests, guest builds, and documentation",
        members: &[
            "sdk:test",
            "sdk:rv32",
            "sdk:macros-guest",
            "sdk:logging",
            "sdk:docs",
            "sdk:docsrs",
            "sdk:cargo-guide",
        ],
    },
    Group {
        name: "fixtures",
        about: "Fixture contracts: rv32 builds, tests, negative fixtures, lint",
        members: &[
            "fixtures-negative:domain-collision",
            "fixtures-negative:dup-entrypoint",
            "fixtures-negative:registry-stripped",
            "fixtures-negative:typedef-conflict",
            "fixtures-negative:event-signature-conflict",
            "fixtures-negative:iface-declarer-conflict",
            "fixtures-negative:missing-entrypoints",
            "fixtures:rv32",
            "fixtures:specs",
            "fixtures:test",
            "fixtures:exception-stack",
            "lint:fixtures",
        ],
    },
    Group {
        name: "integration",
        about: "Integration contracts (needs the submodule)",
        // version-check again: with the submodule checked out, it also
        // scans the integration manifests, which `quick` cannot see.
        members: &["itc:rv32", "itc:test", "lint:itc", "version-check"],
    },
    Group {
        name: "lint",
        about: "Clippy on the SDK and every contract",
        members: &["lint:sdk", "lint:fixtures", "lint:itc"],
    },
    Group {
        name: "metrics-inputs",
        about: "The rv32 artifacts scripts/metrics.sh reads",
        members: &["fixtures:rv32", "itc:rv32"],
    },
];

pub static TASKS: &[Task] = &[
    Task::new("fmt", "rustfmt check", fmt).lane(Lane::Free),
    Task::new(
        "annotations",
        "No XXX annotations in the source",
        annotations,
    )
    .lane(Lane::Free),
    Task::new(
        "cli",
        "The Thruput CLI is on PATH; warns if it is not the pinned version",
        cli,
    )
    .lane(Lane::Free),
    Task::new("lint:sdk", "Clippy on the SDK libraries", lint_sdk),
    Task::new(
        "sdk:rv32",
        "thruput-sdk builds for the guest target",
        sdk_rv32,
    ),
    Task::new(
        "sdk:macros-guest",
        "The macros crate-override fixture builds for the guest",
        sdk_macros_guest,
    ),
    Task::new(
        "sdk:logging",
        "logging-test builds for the guest with logging enabled",
        sdk_logging,
    ),
    Task::new("sdk:test", "The SDK test sequence", sdk_test),
    Task::new("sdk:docs", "Workspace rustdoc without warnings", sdk_docs),
    Task::new(
        "sdk:docsrs",
        "Each publishable crate documents in its docs.rs configuration",
        sdk_docsrs,
    )
    .lane(Lane::Docsrs),
    Task::new(
        "sdk:cargo-guide",
        "The Cargo-only guide builds as a standalone consumer",
        sdk_cargo_guide,
    )
    .lane(Lane::Free),
    Task::new(
        "fixtures-negative:domain-collision",
        "Storage domain collisions are detected",
        domain_collision,
    ),
    Task::new(
        "fixtures-negative:dup-entrypoint",
        "Host and guest builds with duplicate entrypoints fail",
        dup_entrypoint,
    )
    .needs(&["cli"]),
    // Before fixtures:rv32: this builds complex-storage stripped, into the
    // release directory the rv32 build writes its unstripped ELFs to.
    Task::new(
        "fixtures-negative:registry-stripped",
        "The storage domain registry is absent from guest builds",
        registry_stripped,
    ),
    Task::new(
        "fixtures:rv32",
        "rv32 fixture builds and ELF checks",
        fixtures_rv32,
    )
    .needs(&["cli"])
    .after(&["fixtures-negative:registry-stripped"]),
    Task::new(
        "fixtures-negative:typedef-conflict",
        "Two shapes for one type id fail the rv32 build",
        typedef_conflict,
    )
    .needs(&["cli"]),
    Task::new(
        "fixtures-negative:event-signature-conflict",
        "Two events with one signature fail the rv32 build",
        event_signature_conflict,
    )
    .needs(&["cli"]),
    Task::new(
        "fixtures-negative:iface-declarer-conflict",
        "Two declarers for one interface key fail the rv32 build",
        iface_declarer_conflict,
    )
    .needs(&["cli"]),
    Task::new(
        "fixtures-negative:missing-entrypoints",
        "The dispatch check rejects a contract with no entrypoints",
        missing_entrypoints,
    ),
    Task::new(
        "fixtures:specs",
        "The rv32 fixtures' spec sidecars",
        fixtures_specs,
    )
    .needs(&["fixtures:rv32"])
    .lane(Lane::Free),
    Task::new(
        "fixtures:test",
        "Each fixture's tests, one package at a time",
        fixtures_test,
    )
    .needs(&["fixtures:rv32"]),
    Task::new(
        "fixtures:exception-stack",
        "Exception handling with a 16 KiB rv32 stack",
        exception_stack,
    )
    .needs(&["cli"]),
    Task::new(
        "lint:fixtures",
        "Clippy on each fixture, guest and host",
        lint_fixtures,
    ),
    Task::new(
        "itc:rv32",
        "rv32 integration builds and artifact checks",
        itc_rv32,
    )
    .needs(&["cli"]),
    Task::new("itc:test", "The integration workspace tests", itc_test)
        .needs(&["itc:rv32"]),
    Task::new(
        "lint:itc",
        "Clippy on each integration contract, guest and host",
        lint_itc,
    ),
    // Last: with --unlocked, the other tasks may update lockfiles.
    Task::new(
        "version-check",
        "Version pins agree across manifests and lockfiles",
        version_check,
    )
    .last(),
];

fn fmt(ctx: &mut Ctx) -> Result {
    let root = &ctx.repo.root;
    let sdk = ctx.cmd("cargo", root).args(["fmt", "--check"]);
    let gnaw = ctx.cmd("cargo", root).args([
        "fmt",
        "--check",
        "--manifest-path",
        "gnaw/Cargo.toml",
    ]);
    ctx.step("sdk", |ctx| ctx.run(&sdk))?;
    ctx.step("gnaw", |ctx| ctx.run(&gnaw))
}

fn annotations(ctx: &mut Ctx) -> Result {
    if !on_path("rg") {
        return ctx.skip("ripgrep (rg) is not on PATH");
    }
    let ignores = [
        "--ignore-file",
        ".local_src_ignore",
        "--ignore-file",
        ".xxx_ignore",
    ];
    let files = ctx
        .cmd("rg", &ctx.repo.root)
        .arg("--files")
        .args(ignores)
        .arg(".");
    let (status, output) = ctx.capture_quiet(&files)?;
    if !status.success() || output.lines().next().is_none() {
        let mut failure = Failure::new("rg found no files to search");
        failure.command = Some(files.display(ctx.repo));
        return Err(failure.into());
    }
    ctx.log
        .line(&format!("searching {} files", output.lines().count()));
    let cmd = ctx
        .cmd("rg", &ctx.repo.root)
        .arg("-i")
        .args(ignores)
        .args(["XXX", "."]);
    let (status, output) = ctx.capture(&cmd)?;
    match status.code() {
        Some(1) => Ok(()),
        Some(0) => {
            let mut failure = Failure::new("XXX annotations found");
            failure.details =
                output.lines().take(20).map(str::to_string).collect();
            Err(failure.into())
        }
        _ => Err(ctx.command_failure(&cmd, status, &output).into()),
    }
}

/// The pinned version from scripts/thruput-cli-version.sh.
fn pinned_cli_version(ctx: &Ctx) -> std::result::Result<String, Failure> {
    let path = ctx.repo.script("thruput-cli-version.sh");
    let text = std::fs::read_to_string(&path).map_err(|e| {
        Failure::new(format!("reading {}: {e}", path.display()))
    })?;
    text.lines()
        .find_map(|line| line.trim().strip_prefix("THRUPUT_CLI_VERSION="))
        .map(|v| v.trim().trim_matches('"').to_string())
        .ok_or_else(|| {
            Failure::new(format!(
                "no THRUPUT_CLI_VERSION in {}",
                path.display()
            ))
        })
}

fn cli(ctx: &mut Ctx) -> Result {
    let pinned = pinned_cli_version(ctx)?;
    if !on_path("thruput") {
        return Err(Failure::new(
            "Thruput CLI not found: run `cargo gnaw setup` to install the pin",
        )
        .into());
    }
    let cmd = ctx.cmd("thruput", &ctx.repo.root).arg("--version");
    let (status, output) = ctx.capture(&cmd)?;
    if !status.success() {
        return Err(ctx.command_failure(&cmd, status, &output).into());
    }
    // "thruput 0.42.0 (commit ...)"
    let installed = output.split_whitespace().nth(1).unwrap_or_default();
    if installed != pinned {
        ctx.warn(format!("Thruput CLI {installed} is not the pinned {pinned}; CI uses the pin"));
    }
    Ok(())
}

fn lint_sdk(ctx: &mut Ctx) -> Result {
    let cmd = ctx
        .cargo(&ctx.repo.root, "clippy")
        .args(["-F", "dev", "--lib"]);
    ctx.run(&cmd)
}

fn sdk_rv32(ctx: &mut Ctx) -> Result {
    let cmd = ctx.cargo(&ctx.repo.root, "build").args([
        "-p",
        "thruput-sdk",
        "--release",
        "--target",
        RV32_TARGET,
    ]);
    ctx.run(&cmd)
}

/// Covers `#[contract_type_spec(crate = "...")]`, whose guest-only code no
/// other build reaches. A dedicated no_std package, because the proc-macro
/// crate's own tests need std.
fn sdk_macros_guest(ctx: &mut Ctx) -> Result {
    let cmd = ctx.cargo(&ctx.repo.root, "check").args([
        "-p",
        "macros-crate-override-guest",
        "--target",
        RV32_TARGET,
        "-Zbuild-std=core,alloc",
    ]);
    ctx.run(&cmd)
}

/// logging-test enables thruput-sdk/logging, so generated client stubs call
/// debug_log!. That catches use-after-move bugs that appear only under
/// feature unification.
fn sdk_logging(ctx: &mut Ctx) -> Result {
    let cmd = ctx.cargo(&ctx.repo.logging_test(), "check").args([
        "-p",
        "htlc-logging",
        "--target",
        RV32_TARGET,
        "-Zbuild-std=core,alloc",
    ]);
    ctx.run(&cmd)
}

fn sdk_test(ctx: &mut Ctx) -> Result {
    let root = ctx.repo.root.clone();
    let steps: &[(&str, &[&str])] = &[
        ("workspace", &["test"]),
        // u256's self-dev-dependency unifies std and fmt into every test
        // build, so check the guest configuration explicitly.
        (
            "u256 no-default-features",
            &["check", "-p", "thruput-u256", "--no-default-features"],
        ),
        (
            "sdk std",
            &["test", "-p", "thruput-sdk", "-Fstd", "--tests"],
        ),
        // `remote` adds an off-chain proxy beside every generated guest
        // client: test it, then build every target with it.
        (
            "sdk remote",
            &["test", "-p", "thruput-sdk", "-Fremote", "--test", "remote"],
        ),
        (
            "all targets with remote",
            &[
                "check",
                "--all-targets",
                "-F",
                "thruput-sdk/remote,thruput-sdk/dev",
            ],
        ),
        (
            "erc20-metadata-if",
            &["test", "-p", "thruput-erc20-metadata-if", "--all-targets"],
        ),
        (
            "spec-tool",
            &["test", "-p", "thruput-spec-tool", "--all-targets"],
        ),
        ("test-vectors", &["test", "-p", "thruput-test-vectors"]),
        ("vector-tests", &["test", "-p", "vector-tests", "--tests"]),
    ];
    for (name, args) in steps {
        let cmd = ctx.cargo(&root, args[0]).args(&args[1..]);
        ctx.step(*name, |ctx| ctx.run(&cmd))?;
    }
    Ok(())
}

fn sdk_docs(ctx: &mut Ctx) -> Result {
    let cmd = ctx
        .cargo(&ctx.repo.root, "doc")
        .args(["--no-deps", "--features=dev"])
        .env("RUSTDOCFLAGS", "--deny warnings --cfg docsrs");
    ctx.run(&cmd)
}

fn sdk_docsrs(ctx: &mut Ctx) -> Result {
    let cmd = ctx.script("check-docsrs.sh");
    ctx.run(&cmd)
}

fn sdk_cargo_guide(ctx: &mut Ctx) -> Result {
    let cmd = ctx.script("cargo-example-smoke.sh");
    ctx.run(&cmd)
}

fn domain_collision(ctx: &mut Ctx) -> Result {
    let cmd = ctx.script("check-domain-collision-detected.sh");
    ctx.run(&cmd)
}

/// dup_entrypoint_neg has two colliding traits that share a method name, so
/// both impls get one entrypoint id. Two dispatch records with one id once
/// passed silently: the runtime's first-match scan routed every call to one
/// impl, and the host mock let the last registration win. Guard symbols now
/// carry the id, so both builds must fail with a duplicate symbol. The crate
/// is outside the workspace and its kind is `negative`.
fn dup_entrypoint(ctx: &mut Ctx) -> Result {
    let dir = ctx.repo.fixtures().join("dup_entrypoint_neg");
    // The guard is crate-scoped; the scope is hex, for linker safety.
    const GUARD: &str = "__thruput_entrypoint::Ledger::0.4::clash__crate__\
                         6475705f656e747279706f696e745f6e6567__302e342e30";
    let host = ctx.cargo(&dir, "build").args(["--features", "dev"]);
    ctx.step("host", |ctx| {
        expect_failure(
            ctx,
            &host,
            "a crate-scoped guard defined twice",
            |out| {
                out.lines().any(|l| {
                    l.contains(GUARD) && l.contains("already defined")
                })
            },
        )
    })?;
    let guest = ctx.cmd("thruput", &dir).args([
        "build",
        "--no-ui",
        "-p",
        "dup_entrypoint_neg",
    ]);
    ctx.step("guest", |ctx| {
        expect_failure(ctx, &guest, "a duplicate symbol", |out| {
            out.contains("already defined") || out.contains("duplicate symbol")
        })
    })
}

/// typedef_id_collision gives two declarations in one module one spec id
/// (`Decoy` takes `Colliding`'s through `name = "..."`), with different
/// shapes, and keeps both reachable. The ELF links, and the spec
/// canonicalization during lowering must refuse it.
///
/// Qualified type names let same-named types coexist (see
/// typedef_ids_coexist); this pins the half that must not relax: two shapes
/// claiming one id.
fn typedef_conflict(ctx: &mut Ctx) -> Result {
    // The id's crate and version change with every release.
    negative_rv32_build(
        ctx,
        "typedef_id_collision",
        r"conflicting contract type definitions for .*::Colliding",
    )
}

/// event_signature_collision declares one event ident in two of its modules
/// and emits both: distinct spec ids, one signature (topic 0, all a consumer
/// sees on the wire).
fn event_signature_conflict(ctx: &mut Ctx) -> Result {
    negative_rv32_build(
        ctx,
        "event_signature_collision",
        r"events share an event signature: .*::ledger::Updated .* and .*::vault::Updated",
    )
}

/// iface_declarer_collision implements two unrelated `Token` traits. They
/// share an interface key (name and major version, all the wire carries),
/// while distinct methods let the entrypoints link. The spec must not
/// publish one `Token` holding both method sets. (iface_conflict_impl, which
/// implements one `Token` and only links the other, builds: see
/// [`specs::SPECS`].)
fn iface_declarer_conflict(ctx: &mut Ctx) -> Result {
    negative_rv32_build(
        ctx,
        "iface_declarer_collision",
        r"interface declared by more than one item: Token@.* is declared by both .*::ledger::Token .* and .*::vault::Token",
    )
}

/// The rv32 build of a fixtures-negative package must fail with output
/// matching `pattern`.
fn negative_rv32_build(ctx: &mut Ctx, package: &str, pattern: &str) -> Result {
    let regex = Regex::new(pattern).expect("valid pattern");
    let cmd = ctx
        .cmd("thruput", &ctx.repo.negative_fixtures())
        .args(["build", "--no-ui", "-p", package]);
    expect_failure(ctx, &cmd, pattern, |out| regex.is_match(out))
}

/// missing_contract_macro has no `#[contract]` and registers no entrypoint by
/// hand, so nothing can call it. Omitting the macro is fine on its own
/// (raw_iface_dep and test_bytestring write their records by hand); the
/// guard is on an empty dispatch table. The ELF must link, unstripped for
/// the bracket symbols, and the dispatch check must reject it. Its linked
/// interface crate still fills the raw spec section: the shape that once
/// shipped as an uncallable module with an empty spec.
fn missing_entrypoints(ctx: &mut Ctx) -> Result {
    let package = "missing_contract_macro";
    let cmd = ctx
        .cargo(&ctx.repo.fixtures(), "build")
        .args([
            "-p",
            package,
            "-F",
            "entrypoints",
            "--release",
            "--target",
            RV32_TARGET,
            "-Zbuild-std=core,alloc",
            "-Zbuild-std-features=std/optimize_for_size",
        ])
        .env("CARGO_PROFILE_RELEASE_STRIP", "false");
    ctx.step("build", |ctx| ctx.run(&cmd))?;
    let path = elf_path(ctx.repo, package);
    let data = std::fs::read(&path).map_err(|e| {
        Failure::new(format!("reading {}: {e}", ctx.repo.rel(&path).display()))
    })?;
    match elf::check_dispatch(&data) {
        Err(e) if e.contains(elf::EMPTY_DISPATCH_TABLE) => Ok(()),
        Err(e) => Err(Failure::new(format!(
            "expected an empty dispatch table, got: {e}"
        ))
        .into()),
        Ok(_) => Err(Failure::new(
            "the dispatch check accepted an empty dispatch table",
        )
        .into()),
    }
}

/// Runs `cmd`, which must fail with output that satisfies `expected`.
fn expect_failure(
    ctx: &mut Ctx,
    cmd: &Cmd,
    what: &str,
    expected: impl Fn(&str) -> bool,
) -> Result {
    let (status, output) = ctx.capture(cmd)?;
    let output = strip_ansi(&output);
    let mut failure = if status.success() {
        Failure::new(format!("expected {what}, but the build succeeded"))
    } else if expected(&output) {
        return Ok(());
    } else {
        let mut failure = Failure::new(format!(
            "expected {what}; the build failed otherwise"
        ));
        failure.details = notable_lines(&output);
        failure
    };
    failure.command = Some(cmd.display(ctx.repo));
    Err(failure.into())
}

fn registry_stripped(ctx: &mut Ctx) -> Result {
    let cmd = ctx.script("check-storage-registry-stripped.sh");
    let (status, output) = ctx.capture(&cmd)?;
    if !status.success() {
        return Err(ctx.command_failure(&cmd, status, &output).into());
    }
    // The script exits 0 when it cannot confirm the toolchain.
    match output
        .lines()
        .find_map(|line| line.strip_prefix("SKIPPED: "))
    {
        Some(reason) => ctx.skip(reason),
        None => Ok(()),
    }
}

fn fixtures_rv32(ctx: &mut Ctx) -> Result {
    let dir = ctx.repo.fixtures();
    rv32::build_all(ctx, &dir, &dir, "fixtures")
}

fn fixtures_specs(ctx: &mut Ctx) -> Result {
    specs::check(ctx)
}

fn fixtures_test(ctx: &mut Ctx) -> Result {
    let dir = ctx.repo.fixtures();
    let packages = packages(&dir)?;
    for package in &packages {
        let cmd = ctx.cargo(&dir, "test").args(["-p", package]);
        ctx.step(package.as_str(), |ctx| ctx.run(&cmd))?;
    }
    // Some packages' tests, run against the native contract above, can also
    // run against the rv32 artifact: see `Fixture::rv32_artifact_vars`.
    for package in &packages {
        let vars = Fixture::read(&dir.join(package))
            .map_err(|e| Failure::new(e.to_string()))?
            .rv32_artifact_vars;
        if vars.is_empty() {
            continue;
        }
        let artifact = ctx.repo.lowered().join(format!("{package}.rv32a"));
        let mut cmd = ctx.cargo(&dir, "test").args(["-p", package]);
        for var in &vars {
            cmd = cmd.env(var, &artifact);
        }
        let step = format!("{package} (rv32 artifact)");
        ctx.step(step, |ctx| ctx.run(&cmd))?;
    }
    Ok(())
}

fn exception_stack(ctx: &mut Ctx) -> Result {
    let cmd = ctx.script("check-exception-stack.sh");
    ctx.run(&cmd)
}

fn lint_fixtures(ctx: &mut Ctx) -> Result {
    let dir = ctx.repo.fixtures();
    lint_contracts(ctx, &dir, &dir)
}

fn itc_rv32(ctx: &mut Ctx) -> Result {
    require_integration(ctx)?;
    let (workspace, contracts) =
        (ctx.repo.integration(), ctx.repo.integration_contracts());
    rv32::build_all(ctx, &workspace, &contracts, "integration")
}

fn itc_test(ctx: &mut Ctx) -> Result {
    require_integration(ctx)?;
    let cmd = ctx.cargo(&ctx.repo.integration(), "test").args([
        "--workspace",
        "--",
        "--show-output",
    ]);
    ctx.run(&cmd)
}

fn lint_itc(ctx: &mut Ctx) -> Result {
    require_integration(ctx)?;
    let (workspace, contracts) =
        (ctx.repo.integration(), ctx.repo.integration_contracts());
    lint_contracts(ctx, &workspace, &contracts)
}

fn version_check(ctx: &mut Ctx) -> Result {
    let cmd = ctx
        .cargo(&ctx.repo.root, "run")
        .args(["-p", "version-check"]);
    ctx.run(&cmd)
}

/// Clippy on each contract package in `contracts`, from `workspace`.
fn lint_contracts(
    ctx: &mut Ctx,
    workspace: &Path,
    contracts: &Path,
) -> Result {
    for package in packages(contracts)? {
        ctx.step(package.as_str(), |ctx| {
            lint_contract(ctx, workspace, &package)
        })?;
    }
    Ok(())
}

/// Clippy on `package`, from `workspace`: once for the guest, where the
/// workspace config supplies the rv32 flags, and once on the host with
/// `dev`.
pub fn lint_contract(
    ctx: &mut Ctx,
    workspace: &Path,
    package: &str,
) -> Result {
    let guest = ctx.cargo(workspace, "clippy").args([
        "-p",
        package,
        "-F",
        "entrypoints",
        "--release",
        "--target",
        RV32_TARGET,
        "-Zbuild-std=core,alloc",
        "--",
        "-Dwarnings",
    ]);
    let host = ctx
        .cargo(workspace, "clippy")
        .args(["-F", "dev", "-p", package]);
    ctx.step("guest", |ctx| ctx.run(&guest))?;
    ctx.step("host", |ctx| ctx.run(&host))
}

fn packages(dir: &Path) -> Result<Vec<String>> {
    let packages = contract_packages(dir).map_err(|e| {
        Failure::new(format!("listing {}: {e}", dir.display()))
    })?;
    if packages.is_empty() {
        return Err(Failure::new(format!(
            "no contract packages in {}",
            dir.display()
        ))
        .into());
    }
    Ok(packages)
}

fn require_integration(ctx: &Ctx) -> Result {
    let dir = ctx.repo.integration_contracts();
    if contract_packages(&dir)
        .map(|p| p.is_empty())
        .unwrap_or(true)
    {
        return Err(Failure::new(format!(
            "{} has no contracts: run `git submodule update --init --recursive`",
            ctx.repo.rel(&dir).display()
        ))
        .into());
    }
    Ok(())
}

pub fn on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| dir.join(program).is_file())
    })
}
