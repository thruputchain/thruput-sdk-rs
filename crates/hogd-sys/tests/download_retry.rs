//! Retry classification for the build script's archive download.
//!
//! The build script is not linkable from tests, so the download loop lives
//! in `build/download.rs` and is compiled into this test with `#[path]`.
//! A fake `curl` script stands in for the real one and records how often it
//! was invoked.
//!
//! The tests run one at a time: a child forked by one test inherits the
//! write handle of a script another test is still writing, and executing
//! that script then fails with `ETXTBSY`.

#![cfg(unix)]

#[path = "../build/download.rs"]
mod download;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
static SERIAL: Mutex<()> = Mutex::new(());

/// A temporary directory holding the fake curl, its call counter, and the
/// download destination. Removed on drop. Holds the serialization lock for
/// its lifetime.
struct Sandbox {
    dir: PathBuf,
    _serial: MutexGuard<'static, ()>,
}

impl Sandbox {
    fn new() -> Self {
        let serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "thruput-sys-download-retry-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        Self {
            dir,
            _serial: serial,
        }
    }

    /// Writes an executable `curl` that runs `body` with `$out` bound to the
    /// `-o` argument, `$calls` to this invocation's 1-based ordinal, and
    /// appends one line to the call log per invocation.
    fn fake_curl(&self, body: &str) -> PathBuf {
        let calls = self.dir.join("calls");
        let script = format!(
            "#!/bin/sh\n\
             calls_file='{}'\n\
             echo x >> \"$calls_file\"\n\
             calls=$(wc -l < \"$calls_file\" | tr -d ' ')\n\
             out=''\n\
             while [ $# -gt 0 ]; do\n\
             \x20 if [ \"$1\" = '-o' ]; then out=\"$2\"; shift; fi\n\
             \x20 shift\n\
             done\n\
             {body}\n",
            calls.display()
        );
        let path = self.dir.join("curl");
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn calls(&self) -> usize {
        fs::read_to_string(self.dir.join("calls"))
            .map(|s| s.lines().count())
            .unwrap_or(0)
    }

    fn dest(&self) -> PathBuf {
        self.dir.join("archive.tar.gz")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn run(curl: &Path, dest: &Path) -> Result<(), String> {
    download::download_with_retry(
        curl,
        "https://example.invalid/test-ffi.tar.gz",
        dest,
        "0.0.0-test",
        3,
        Duration::ZERO,
    )
}

#[test]
fn transient_503_is_retried_until_200() {
    // Arrange: first attempt fails with HTTP 503, second succeeds.
    let sandbox = Sandbox::new();
    let curl = sandbox.fake_curl(
        "if [ \"$calls\" -eq 1 ]; then printf 503; exit 22; fi\n\
         printf payload > \"$out\"\n\
         printf 200\n\
         exit 0",
    );

    // Act
    let result = run(&curl, &sandbox.dest());

    // Assert
    assert_eq!(result, Ok(()));
    assert_eq!(sandbox.calls(), 2, "expected one retry after the 503");
    assert_eq!(fs::read(sandbox.dest()).unwrap(), b"payload");
    assert!(!sandbox.dest().with_extension("tmp").exists());
}

#[test]
fn throttling_429_is_retried_until_200() {
    let sandbox = Sandbox::new();
    let curl = sandbox.fake_curl(
        "if [ \"$calls\" -le 2 ]; then printf 429; exit 22; fi\n\
         printf payload > \"$out\"\n\
         printf 200\n\
         exit 0",
    );

    let result = run(&curl, &sandbox.dest());

    assert_eq!(result, Ok(()));
    assert_eq!(sandbox.calls(), 3);
}

#[test]
fn permanent_404_fails_without_retry() {
    let sandbox = Sandbox::new();
    let curl = sandbox.fake_curl("printf 404\nexit 22");

    let err = run(&curl, &sandbox.dest()).unwrap_err();

    assert_eq!(sandbox.calls(), 1, "a 404 must not be retried");
    assert!(err.contains("HTTP 404"), "{err}");
    assert!(err.contains("not retrying"), "{err}");
    assert!(err.contains("test-ffi v0.0.0-test"), "{err}");
    assert!(err.contains("HOGD_FFI_LIB_DIR"), "{err}");
    assert!(!sandbox.dest().exists());
}

#[test]
fn persistent_503_gives_up_after_retries() {
    let sandbox = Sandbox::new();
    let curl = sandbox.fake_curl("printf 503\nexit 22");

    let err = run(&curl, &sandbox.dest()).unwrap_err();

    assert_eq!(sandbox.calls(), 4, "initial attempt plus three retries");
    assert!(
        err.contains("gave up after 4 attempts (last HTTP 503)"),
        "{err}"
    );
    assert!(err.contains("HOGD_FFI_LIB_DIR"), "{err}");
}

#[test]
fn missing_curl_reports_the_bypass() {
    let sandbox = Sandbox::new();
    let curl = sandbox.dir.join("no-such-curl");

    let err = run(&curl, &sandbox.dest()).unwrap_err();

    assert!(err.contains("curl not found or failed to execute"), "{err}");
    assert!(err.contains("HOGD_FFI_LIB_DIR"), "{err}");
    assert!(err.contains("HOGD_FFI_BUILD=1"), "{err}");
}
