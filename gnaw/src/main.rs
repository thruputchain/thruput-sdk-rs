use std::io::IsTerminal;
use std::process::{Command, ExitCode};

use clap::{Parser, Subcommand, ValueEnum};

use gnaw::exec::{ExecOptions, execute};
use gnaw::plan::{Selection, resolve};
use gnaw::repo::Repo;
use gnaw::report::{Gha, Plain, Renderer, Tty};
use gnaw::task::{Lane, RunOptions};
use gnaw::tasks::{GROUPS, TASKS};

/// Builds, tests, and checks the Thruput SDK repository.
#[derive(Parser)]
#[command(name = "gnaw", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run tasks and their prerequisites.
    Run(RunArgs),
    /// List tasks and groups.
    List {
        /// List groups and their members instead of tasks.
        #[arg(long)]
        groups: bool,
    },
    /// Install the pinned Thruput CLI and give it a config profile if it has
    /// none. Honors THRUPUT_CLI_ROOT.
    Setup,
}

#[derive(clap::Args)]
struct RunArgs {
    /// Task ids, group names, or globs such as `lint:*`.
    #[arg(default_value = "ci")]
    targets: Vec<String>,
    /// Tasks to run at once. Tasks that share a lane still run one at a time.
    #[arg(short, long, default_value_t = 4)]
    jobs: usize,
    /// Run only the named tasks; assume their prerequisites' outputs exist.
    #[arg(long)]
    no_deps: bool,
    /// Leave out a task, group, or glob.
    #[arg(long, value_name = "TARGET")]
    skip: Vec<String>,
    /// After a failure, keep running tasks that do not depend on it.
    #[arg(short, long)]
    keep_going: bool,
    /// Print the plan without running it.
    #[arg(long)]
    dry_run: bool,
    /// Let cargo update lockfiles (omit --locked).
    #[arg(long)]
    unlocked: bool,
    /// Fail tasks that would be skipped because a tool is missing.
    #[arg(long)]
    strict: bool,
    #[arg(long, value_enum, default_value_t = Progress::Auto)]
    progress: Progress,
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
enum Progress {
    /// `gha` under GitHub Actions, `tty` on a terminal, `plain` otherwise.
    Auto,
    /// Live status lines.
    Tty,
    /// One line per event.
    Plain,
    /// One line per event, with logs in collapsed groups.
    Gha,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let repo = match Repo::locate() {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("gnaw: {e}");
            return ExitCode::FAILURE;
        }
    };
    prepend_cli_root_to_path();
    match cli.command {
        Commands::Run(args) => run(&repo, args),
        Commands::List { groups } => {
            list(groups);
            ExitCode::SUCCESS
        }
        Commands::Setup => setup(&repo),
    }
}

/// The CLI install script puts the CLI under THRUPUT_CLI_ROOT when it is set.
fn prepend_cli_root_to_path() {
    let Some(root) = std::env::var_os("THRUPUT_CLI_ROOT") else {
        return;
    };
    let bin = std::path::Path::new(&root).join("bin");
    let path = std::env::var_os("PATH").unwrap_or_default();
    let joined = std::env::join_paths(
        std::iter::once(bin).chain(std::env::split_paths(&path)),
    )
    .expect("PATH entries are joinable");
    // SAFETY: called from main before any other thread starts.
    unsafe { std::env::set_var("PATH", joined) };
}

fn run(repo: &Repo, args: RunArgs) -> ExitCode {
    let selection = Selection {
        targets: &args.targets,
        skip: &args.skip,
        no_deps: args.no_deps,
    };
    let plan = match resolve(&selection) {
        Ok(plan) => plan,
        Err(e) => {
            eprintln!("gnaw: {e}");
            return ExitCode::FAILURE;
        }
    };
    if args.dry_run {
        for planned in &plan.tasks {
            let needs: Vec<&str> = planned
                .needs
                .iter()
                .map(|&j| plan.tasks[j].task.id)
                .collect();
            let lane = match planned.task.lane {
                Lane::Free => "",
                Lane::Target => "  [lane: target/]",
                Lane::Docsrs => "  [lane: target/docsrs]",
            };
            let needs = if needs.is_empty() {
                String::new()
            } else {
                format!("  (needs {})", needs.join(", "))
            };
            println!("{}{needs}{lane}", planned.task.id);
        }
        return ExitCode::SUCCESS;
    }

    let jobs = args.jobs.max(1);
    let progress = match args.progress {
        Progress::Auto if std::env::var_os("GITHUB_ACTIONS").is_some() => {
            Progress::Gha
        }
        Progress::Auto if std::io::stdout().is_terminal() => Progress::Tty,
        Progress::Auto => Progress::Plain,
        other => other,
    };
    let mut renderer: Box<dyn Renderer> = match progress {
        Progress::Gha => Box::new(Gha::default()),
        Progress::Tty => Box::new(Tty::default()),
        _ => Box::new(Plain::new(jobs == 1)),
    };
    let run_options = RunOptions {
        locked: !args.unlocked,
        color: progress != Progress::Plain,
    };
    let exec_options = ExecOptions {
        jobs,
        keep_going: args.keep_going,
        strict: args.strict,
    };
    let results =
        execute(&plan, repo, &run_options, &exec_options, renderer.as_mut());
    renderer.done(&results, repo);
    if results.iter().all(|r| r.outcome.satisfies_dependents()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn list(groups: bool) {
    if groups {
        for (i, group) in GROUPS.iter().enumerate() {
            if i > 0 {
                println!();
            }
            println!("{}: {}", group.name, group.about);
            for member in group.members {
                if GROUPS.iter().any(|g| g.name == *member) {
                    println!("    {member} (group)");
                } else {
                    println!("    {member}");
                }
            }
        }
        return;
    }
    let width = TASKS.iter().map(|t| t.id.len()).max().unwrap_or(0);
    for task in TASKS {
        let needs = if task.needs.is_empty() {
            String::new()
        } else {
            format!(" (needs {})", task.needs.join(", "))
        };
        println!("{:width$}  {}{needs}", task.id, task.about);
    }
}

fn setup(repo: &Repo) -> ExitCode {
    // The install script is sourced: it exports PATH for the commands after it.
    let script = r#"set -e
. ./scripts/install-thruput-cli.sh
# The CLI refuses to run without an active config profile, even offline.
if ! thruput config get --json >/dev/null 2>&1; then
    thruput config new ci --network local --use --global --quiet
fi
thruput --version"#;
    match Command::new("bash")
        .arg("-c")
        .arg(script)
        .current_dir(&repo.root)
        .status()
    {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => {
            eprintln!("gnaw: setup failed ({status})");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("gnaw: running bash: {e}");
            ExitCode::FAILURE
        }
    }
}
