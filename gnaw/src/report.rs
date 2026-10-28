//! Progress output and the final summary.
//!
//! Three renderers share one event stream:
//! - `Plain` prints one line per event. With one job it streams task output.
//! - `Gha` prints one line per event and each task's log in a collapsed
//!   group, then failures as `::error` annotations, for GitHub Actions.
//! - `Tty` shows a live line per running task, for a terminal.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::repo::Repo;
use crate::task::{Failure, Log, Task, strip_ansi};

const HEARTBEAT: Duration = Duration::from_secs(60);

#[derive(Clone, Debug)]
pub enum Outcome {
    Ok,
    Skipped(String),
    Failed(Failure),
    NotRun(String),
}

impl Outcome {
    /// Whether tasks that need this one may run.
    pub fn satisfies_dependents(&self) -> bool {
        matches!(self, Outcome::Ok | Outcome::Skipped(_))
    }
}

pub struct TaskResult {
    pub id: &'static str,
    pub outcome: Outcome,
    pub elapsed: Duration,
    pub warnings: Vec<String>,
    pub log: Option<PathBuf>,
}

impl TaskResult {
    pub fn not_run(task: &Task, reason: String) -> TaskResult {
        TaskResult {
            id: task.id,
            outcome: Outcome::NotRun(reason),
            elapsed: Duration::ZERO,
            warnings: Vec::new(),
            log: None,
        }
    }

    pub fn failed(task: &Task, failure: Failure) -> TaskResult {
        TaskResult {
            outcome: Outcome::Failed(failure),
            ..TaskResult::not_run(task, String::new())
        }
    }
}

pub struct Running {
    pub id: &'static str,
    pub elapsed: Duration,
    pub log: Arc<Log>,
}

pub trait Renderer {
    /// Whether task output should go straight to stdout as well as the log.
    fn streams(&self) -> bool {
        false
    }
    fn started(&mut self, id: &str);
    fn finished(&mut self, result: &TaskResult);
    /// Called a few times a second while tasks run.
    fn tick(&mut self, _running: &[Running]) {}
    fn done(&mut self, results: &[TaskResult], repo: &Repo);
}

pub struct Plain {
    start: Instant,
    stream: bool,
    heartbeat: Instant,
}

impl Plain {
    pub fn new(stream: bool) -> Plain {
        Plain {
            start: Instant::now(),
            stream,
            heartbeat: Instant::now(),
        }
    }
}

impl Renderer for Plain {
    fn streams(&self) -> bool {
        self.stream
    }

    fn started(&mut self, id: &str) {
        println!("[{}] ▶ {id}", clock(self.start));
    }

    fn finished(&mut self, result: &TaskResult) {
        println!("[{}] {}", clock(self.start), status_line(result));
    }

    fn tick(&mut self, running: &[Running]) {
        heartbeat(&mut self.heartbeat, self.start, running);
    }

    fn done(&mut self, results: &[TaskResult], repo: &Repo) {
        if !self.stream {
            print_failed_log_tails(results, repo);
        }
        print_summary(results, self.start.elapsed(), repo);
    }
}

pub struct Gha {
    start: Instant,
    heartbeat: Instant,
}

impl Default for Gha {
    fn default() -> Gha {
        Gha {
            start: Instant::now(),
            heartbeat: Instant::now(),
        }
    }
}

impl Renderer for Gha {
    fn started(&mut self, id: &str) {
        println!("[{}] ▶ {id}", clock(self.start));
    }

    fn finished(&mut self, result: &TaskResult) {
        let title = format!("[{}] {}", clock(self.start), status_line(result));
        let Some(log) = &result.log else {
            println!("{title}");
            return;
        };
        println!("::group::{title}");
        // Task output must not be read as workflow commands.
        let token = format!(
            "gnaw-{:x}-{}",
            std::process::id(),
            self.start.elapsed().as_nanos()
        );
        println!("::stop-commands::{token}");
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(&std::fs::read(log).unwrap_or_default());
        let _ = writeln!(stdout, "\n::{token}::");
        let _ = writeln!(stdout, "::endgroup::");
        drop(stdout);
        if let Outcome::Failed(failure) = &result.outcome {
            println!(
                "::error title={}::{}",
                escape_property(&failure_path(result.id, failure)),
                escape_data(&failure.message)
            );
        }
        for warning in &result.warnings {
            println!(
                "::warning title={}::{}",
                escape_property(result.id),
                escape_data(warning)
            );
        }
    }

    fn tick(&mut self, running: &[Running]) {
        heartbeat(&mut self.heartbeat, self.start, running);
    }

    fn done(&mut self, results: &[TaskResult], repo: &Repo) {
        print_summary(results, self.start.elapsed(), repo);
    }
}

pub struct Tty {
    start: Instant,
    multi: MultiProgress,
    bars: HashMap<String, ProgressBar>,
}

impl Default for Tty {
    fn default() -> Tty {
        Tty {
            start: Instant::now(),
            multi: MultiProgress::new(),
            bars: HashMap::new(),
        }
    }
}

impl Renderer for Tty {
    fn started(&mut self, id: &str) {
        let bar = self.multi.add(ProgressBar::new_spinner());
        bar.set_style(
            ProgressStyle::with_template(
                "{spinner} {prefix} {elapsed:>4}  {wide_msg}",
            )
            .unwrap(),
        );
        bar.set_prefix(id.to_string());
        bar.enable_steady_tick(Duration::from_millis(120));
        self.bars.insert(id.to_string(), bar);
    }

    fn finished(&mut self, result: &TaskResult) {
        if let Some(bar) = self.bars.remove(result.id) {
            bar.finish_and_clear();
            self.multi.remove(&bar);
        }
        let _ = self.multi.println(status_line(result));
    }

    fn tick(&mut self, running: &[Running]) {
        for task in running {
            if let Some(bar) = self.bars.get(task.id) {
                bar.set_message(task.log.last_line());
            }
        }
    }

    fn done(&mut self, results: &[TaskResult], repo: &Repo) {
        let _ = self.multi.clear();
        print_failed_log_tails(results, repo);
        print_summary(results, self.start.elapsed(), repo);
    }
}

fn heartbeat(last: &mut Instant, start: Instant, running: &[Running]) {
    if last.elapsed() < HEARTBEAT || running.is_empty() {
        return;
    }
    *last = Instant::now();
    let list: Vec<String> = running
        .iter()
        .map(|r| format!("{} ({})", r.id, duration(r.elapsed)))
        .collect();
    println!("[{}] … running: {}", clock(start), list.join(", "));
}

pub fn status_line(result: &TaskResult) -> String {
    let mut line = match &result.outcome {
        Outcome::Ok => {
            format!("✔ {} ({})", result.id, duration(result.elapsed))
        }
        Outcome::Skipped(reason) => {
            format!("– {} skipped: {reason}", result.id)
        }
        Outcome::Failed(failure) => format!(
            "✘ {} ({})",
            failure_path(result.id, failure),
            duration(result.elapsed)
        ),
        Outcome::NotRun(reason) => {
            format!("· {} not run: {reason}", result.id)
        }
    };
    if !result.warnings.is_empty() {
        let _ = write!(line, " [{} warning(s)]", result.warnings.len());
    }
    line
}

/// `task › step › step`: where the failure happened.
fn failure_path(id: &str, failure: &Failure) -> String {
    std::iter::once(id)
        .chain(failure.steps.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" › ")
}

fn print_failed_log_tails(results: &[TaskResult], repo: &Repo) {
    const TAIL: usize = 40;
    for result in results {
        let (Outcome::Failed(_), Some(log)) = (&result.outcome, &result.log)
        else {
            continue;
        };
        let text = std::fs::read_to_string(log).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        println!(
            "\n──── {}: last {TAIL} lines of {}",
            result.id,
            repo.rel(log).display()
        );
        for line in &lines[lines.len().saturating_sub(TAIL)..] {
            println!("{line}");
        }
    }
}

fn print_summary(results: &[TaskResult], elapsed: Duration, repo: &Repo) {
    let count = |f: fn(&Outcome) -> bool| {
        results.iter().filter(|r| f(&r.outcome)).count()
    };
    let ok = count(|o| matches!(o, Outcome::Ok));
    let failed = count(|o| matches!(o, Outcome::Failed(_)));
    let skipped = count(|o| matches!(o, Outcome::Skipped(_)));
    let not_run = count(|o| matches!(o, Outcome::NotRun(_)));
    println!();
    for result in results {
        if let Outcome::Failed(failure) = &result.outcome {
            println!(
                "{}",
                diagnosis(result.id, failure, result.log.as_deref(), repo)
            );
        }
    }
    for result in results {
        if matches!(result.outcome, Outcome::Skipped(_) | Outcome::NotRun(_)) {
            println!("{}", status_line(result));
        }
        for warning in &result.warnings {
            println!("warning: {}: {warning}", result.id);
        }
    }
    println!(
        "\ngnaw: {ok} ok, {failed} failed, {skipped} skipped, {not_run} not run in {}",
        duration(elapsed)
    );
    if let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        write_step_summary(Path::new(&path), results, repo);
    }
}

fn diagnosis(
    id: &str,
    failure: &Failure,
    log: Option<&Path>,
    repo: &Repo,
) -> String {
    let mut text =
        format!("✘ {}\n    {}\n", failure_path(id, failure), failure.message);
    for line in &failure.details {
        let _ = writeln!(text, "    | {line}");
    }
    if let Some(command) = &failure.command {
        let _ = writeln!(text, "    ran:   {command}");
    }
    if let Some(log) = log {
        let _ = writeln!(text, "    log:   {}", repo.rel(log).display());
    }
    let _ = writeln!(text, "    rerun: cargo gnaw run {id} --no-deps");
    text
}

fn write_step_summary(path: &Path, results: &[TaskResult], repo: &Repo) {
    let mut md =
        String::from("| Task | Result | Time |\n| --- | --- | --- |\n");
    for result in results {
        let state = match &result.outcome {
            Outcome::Ok => "✔ ok".to_string(),
            Outcome::Skipped(reason) => format!("– skipped: {reason}"),
            Outcome::Failed(failure) => {
                format!("✘ {}", failure_path(result.id, failure))
            }
            Outcome::NotRun(reason) => format!("· not run: {reason}"),
        };
        let _ = writeln!(
            md,
            "| `{}` | {} | {} |",
            result.id,
            state.replace('|', "\\|"),
            duration(result.elapsed)
        );
    }
    for result in results {
        if let Outcome::Failed(failure) = &result.outcome {
            let _ = write!(
                md,
                "\n```\n{}```\n",
                diagnosis(result.id, failure, result.log.as_deref(), repo)
            );
        }
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
    {
        let _ = file.write_all(md.as_bytes());
    }
}

fn clock(start: Instant) -> String {
    let secs = start.elapsed().as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

pub fn duration(d: Duration) -> String {
    let secs = d.as_secs();
    match secs {
        0..60 => format!("{:.1}s", d.as_secs_f64()),
        60..3600 => format!("{}m{:02}s", secs / 60, secs % 60),
        _ => format!("{}h{:02}m", secs / 3600, secs / 60 % 60),
    }
}

/// Escapes a workflow command's message.
fn escape_data(text: &str) -> String {
    strip_ansi(text)
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// Escapes a workflow command property such as `title`.
fn escape_property(text: &str) -> String {
    escape_data(text).replace(':', "%3A").replace(',', "%2C")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(duration(Duration::from_millis(41_250)), "41.2s");
        assert_eq!(duration(Duration::from_secs(130)), "2m10s");
        assert_eq!(duration(Duration::from_secs(3720)), "1h02m");
    }

    #[test]
    fn escapes_workflow_commands() {
        assert_eq!(
            escape_property("fixtures:test › a,b"),
            "fixtures%3Atest › a%2Cb"
        );
        assert_eq!(escape_data("50%\nnext"), "50%25%0Anext");
    }
}
