//! Tasks, and the context a task body runs in.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};

use crate::repo::Repo;

/// A lock a task holds while it runs. Tasks in one lane run one at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// Holds no lock.
    Free,
    /// The shared `target/` directory. Every contract workspace builds there,
    /// so concurrent cargo runs would only wait on cargo's own lock.
    Target,
    /// `target/docsrs`, used only by the docs.rs check.
    Docsrs,
}

pub struct Task {
    pub id: &'static str,
    pub about: &'static str,
    /// Prerequisites. Planning adds them unless `--no-deps` is given, and a
    /// task does not run if one of them fails.
    pub needs: &'static [&'static str],
    /// Tasks that must finish first when both are planned. Ordering only.
    pub after: &'static [&'static str],
    /// Runs after every other planned task.
    pub last: bool,
    pub lane: Lane,
    pub run: fn(&mut Ctx) -> Result,
}

impl Task {
    pub const fn new(
        id: &'static str,
        about: &'static str,
        run: fn(&mut Ctx) -> Result,
    ) -> Task {
        Task {
            id,
            about,
            needs: &[],
            after: &[],
            last: false,
            lane: Lane::Target,
            run,
        }
    }

    pub const fn needs(self, needs: &'static [&'static str]) -> Task {
        Task { needs, ..self }
    }

    pub const fn after(self, after: &'static [&'static str]) -> Task {
        Task { after, ..self }
    }

    pub const fn last(self) -> Task {
        Task { last: true, ..self }
    }

    pub const fn lane(self, lane: Lane) -> Task {
        Task { lane, ..self }
    }
}

/// Why a task stopped early.
#[derive(Debug)]
pub enum Stop {
    Fail(Failure),
    /// A precondition outside the repository is missing, such as a tool.
    /// `--strict` turns this into a failure.
    Skip(String),
}

pub type Result<T = ()> = std::result::Result<T, Stop>;

#[derive(Clone, Debug, Default)]
pub struct Failure {
    /// The steps inside the task, outermost first.
    pub steps: Vec<String>,
    pub message: String,
    /// Output lines that explain the failure.
    pub details: Vec<String>,
    /// The failing command, as a shell line.
    pub command: Option<String>,
}

impl Failure {
    pub fn new(message: impl Into<String>) -> Failure {
        Failure {
            message: message.into(),
            ..Failure::default()
        }
    }
}

impl From<Failure> for Stop {
    fn from(failure: Failure) -> Stop {
        Stop::Fail(failure)
    }
}

pub struct RunOptions {
    /// Pass `--locked` to cargo.
    pub locked: bool,
    /// Ask cargo for color even though output is captured.
    pub color: bool,
}

/// A task's output log. The renderer reads its last line for live status.
pub struct Log {
    inner: Mutex<LogInner>,
    pub path: PathBuf,
}

struct LogInner {
    file: File,
    last_line: String,
    echo: bool,
}

impl Log {
    pub fn create(path: PathBuf, echo: bool) -> io::Result<Log> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = File::create(&path)?;
        Ok(Log {
            inner: Mutex::new(LogInner {
                file,
                last_line: String::new(),
                echo,
            }),
            path,
        })
    }

    pub fn write(&self, bytes: &[u8]) {
        let mut inner = self.inner.lock().unwrap();
        // A log that cannot be written must not fail the task it records.
        let _ = inner.file.write_all(bytes);
        if inner.echo {
            let mut stdout = io::stdout().lock();
            let _ = stdout.write_all(bytes);
            let _ = stdout.flush();
        }
        let text = String::from_utf8_lossy(bytes);
        if let Some(line) = text
            .split(['\n', '\r'])
            .rev()
            .find(|l| !l.trim().is_empty())
        {
            inner.last_line = strip_ansi(line).trim().to_string();
        }
    }

    pub fn line(&self, line: &str) {
        self.write(format!("{line}\n").as_bytes());
    }

    pub fn last_line(&self) -> String {
        self.inner.lock().unwrap().last_line.clone()
    }
}

/// A command to run, kept as data so a failure can show it.
pub struct Cmd {
    program: String,
    args: Vec<OsString>,
    dir: PathBuf,
    env: Vec<(String, OsString)>,
}

impl Cmd {
    pub fn new(program: &str, dir: &Path) -> Cmd {
        Cmd {
            program: program.to_string(),
            args: Vec::new(),
            dir: dir.to_path_buf(),
            env: Vec::new(),
        }
    }

    /// `cargo <subcommand>` in `dir`, with `--locked` if `locked`.
    pub fn cargo(dir: &Path, subcommand: &str, locked: bool) -> Cmd {
        let cmd = Cmd::new("cargo", dir);
        let cmd = if locked { cmd.arg("--locked") } else { cmd };
        cmd.arg(subcommand)
    }

    pub fn arg(mut self, arg: impl Into<OsString>) -> Cmd {
        self.args.push(arg.into());
        self
    }

    pub fn args<I: IntoIterator<Item = S>, S: Into<OsString>>(
        mut self,
        args: I,
    ) -> Cmd {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn env(mut self, key: &str, value: impl Into<OsString>) -> Cmd {
        self.env.push((key.to_string(), value.into()));
        self
    }

    /// The command, ready to spawn.
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args).current_dir(&self.dir);
        for (key, value) in &self.env {
            command.env(key, value);
        }
        command
    }

    /// The command as a shell line, run from the repository root.
    pub fn display(&self, repo: &Repo) -> String {
        let mut line = String::new();
        let dir = repo.rel(&self.dir);
        if !dir.as_os_str().is_empty() {
            let _ = write!(line, "(cd {} && ", quote(&dir.to_string_lossy()));
        }
        for (key, value) in &self.env {
            let _ = write!(line, "{key}={} ", quote(&value.to_string_lossy()));
        }
        line.push_str(&self.program);
        for arg in &self.args {
            line.push(' ');
            line.push_str(&quote(&arg.to_string_lossy()));
        }
        if !dir.as_os_str().is_empty() {
            line.push(')');
        }
        line
    }
}

pub struct Ctx<'a> {
    pub repo: &'a Repo,
    pub opts: &'a RunOptions,
    pub log: Arc<Log>,
    steps: Vec<String>,
    warnings: Vec<String>,
}

impl<'a> Ctx<'a> {
    pub fn new(
        repo: &'a Repo,
        opts: &'a RunOptions,
        log: Arc<Log>,
    ) -> Ctx<'a> {
        Ctx {
            repo,
            opts,
            log,
            steps: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn into_warnings(self) -> Vec<String> {
        self.warnings
    }

    /// Runs `body` as a named step. A failure inside records the step path.
    pub fn step<T>(
        &mut self,
        name: impl Into<String>,
        body: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        self.steps.push(name.into());
        self.log.line(&format!("== {}", self.steps.join(" › ")));
        let mut result = body(self);
        if let Err(Stop::Fail(failure)) = &mut result
            && failure.steps.is_empty()
        {
            failure.steps = self.steps.clone();
        }
        self.steps.pop();
        result
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.log.line(&format!("warning: {message}"));
        self.warnings.push(message);
    }

    pub fn skip<T>(&self, reason: impl Into<String>) -> Result<T> {
        Err(Stop::Skip(reason.into()))
    }

    pub fn cmd(&self, program: &str, dir: &Path) -> Cmd {
        Cmd::new(program, dir)
    }

    /// `cargo <subcommand>` in `dir`, with `--locked` unless unlocked.
    pub fn cargo(&self, dir: &Path, subcommand: &str) -> Cmd {
        Cmd::cargo(dir, subcommand, self.opts.locked)
    }

    /// `bash scripts/<name>` from the repository root.
    pub fn script(&self, name: &str) -> Cmd {
        Cmd::new("bash", &self.repo.root)
            .arg(self.repo.rel(&self.repo.script(name)).as_os_str())
    }

    /// Runs `cmd` and requires it to succeed.
    pub fn run(&mut self, cmd: &Cmd) -> Result {
        let (status, output) = self.capture(cmd)?;
        if status.success() {
            Ok(())
        } else {
            Err(self.command_failure(cmd, status, &output).into())
        }
    }

    /// Runs `cmd` and returns its status and combined output, which also goes
    /// to the log.
    pub fn capture(&mut self, cmd: &Cmd) -> Result<(ExitStatus, String)> {
        self.spawn(cmd, true)
    }

    /// Like [`Ctx::capture`], but logs only the command line.
    pub fn capture_quiet(
        &mut self,
        cmd: &Cmd,
    ) -> Result<(ExitStatus, String)> {
        self.spawn(cmd, false)
    }

    fn spawn(&mut self, cmd: &Cmd, log: bool) -> Result<(ExitStatus, String)> {
        self.log.line(&format!("$ {}", cmd.display(self.repo)));
        let log = log.then_some(&*self.log);
        let mut command = cmd.command();
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("CARGO_TERM_PROGRESS_WHEN", "never");
        if self.opts.color {
            command.env("CARGO_TERM_COLOR", "always");
        }
        let mut child = command.spawn().map_err(|e| {
            let mut failure =
                Failure::new(format!("could not start {}: {e}", cmd.program));
            failure.command = Some(cmd.display(self.repo));
            Stop::Fail(failure)
        })?;

        let output = Arc::new(Mutex::new(Vec::new()));
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let status = std::thread::scope(|scope| {
            scope.spawn(|| copy(stdout, log, &output));
            scope.spawn(|| copy(stderr, log, &output));
            child.wait()
        });
        let status = status.map_err(|e| {
            Stop::Fail(Failure::new(format!(
                "waiting for {}: {e}",
                cmd.program
            )))
        })?;
        let output =
            String::from_utf8_lossy(&output.lock().unwrap()).into_owned();
        Ok((status, output))
    }

    pub fn command_failure(
        &self,
        cmd: &Cmd,
        status: ExitStatus,
        output: &str,
    ) -> Failure {
        Failure {
            steps: Vec::new(),
            message: format!("`{}` failed ({status})", cmd.program),
            details: notable_lines(output),
            command: Some(cmd.display(self.repo)),
        }
    }
}

fn copy(mut from: impl Read, log: Option<&Log>, output: &Mutex<Vec<u8>>) {
    let mut buf = [0; 8192];
    while let Ok(n) = from.read(&mut buf) {
        if n == 0 {
            break;
        }
        if let Some(log) = log {
            log.write(&buf[..n]);
        }
        output.lock().unwrap().extend_from_slice(&buf[..n]);
    }
}

/// The lines of `output` most likely to explain a failure, last ones kept.
/// Without any recognizable error lines, the last few lines.
pub fn notable_lines(output: &str) -> Vec<String> {
    const MAX: usize = 12;
    const TAIL: usize = 6;
    const MARKERS: &[&str] = &[
        "error",
        "!! ",
        "FAIL",
        "ERROR",
        "panicked at",
        "test result: FAILED",
    ];
    let mut lines: Vec<String> = output
        .lines()
        .map(strip_ansi)
        .filter(|line| {
            let trimmed = line.trim_start();
            MARKERS.iter().any(|m| trimmed.starts_with(m))
                || line.contains("panicked at")
        })
        .map(|line| line.trim_end().to_string())
        .collect();
    lines.dedup();
    if lines.is_empty() {
        lines = output
            .lines()
            .map(|line| strip_ansi(line).trim_end().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        lines.drain(..lines.len().saturating_sub(TAIL));
    }
    if lines.len() > MAX {
        lines.drain(..lines.len() - MAX);
    }
    lines
}

/// Removes ANSI escape sequences.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // CSI: ESC [ parameters final-byte. Anything else: drop ESC alone.
            if chars.clone().next() == Some('[') {
                chars.next();
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn quote(arg: &str) -> String {
    let plain = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@%".contains(c));
    if plain {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_color() {
        assert_eq!(
            strip_ansi("\x1b[1m\x1b[31merror\x1b[0m: boom"),
            "error: boom"
        );
    }

    #[test]
    fn keeps_explanatory_lines() {
        let output = "   Compiling x\n\x1b[31merror[E0425]\x1b[0m: nope\nthread 'a' panicked at src/lib.rs:1\n!! Failed building rv32 contract foo\n";
        assert_eq!(
            notable_lines(output),
            [
                "error[E0425]: nope",
                "thread 'a' panicked at src/lib.rs:1",
                "!! Failed building rv32 contract foo"
            ]
        );
    }

    #[test]
    fn falls_back_to_the_tail() {
        let output = "a\nb\n\nc\nd\ne\nf\ng\n";
        assert_eq!(notable_lines(output), ["b", "c", "d", "e", "f", "g"]);
    }

    #[test]
    fn quotes_shell_words() {
        assert_eq!(quote("-Zbuild-std=core,alloc"), "-Zbuild-std=core,alloc");
        assert_eq!(quote("--deny warnings"), "'--deny warnings'");
    }
}
