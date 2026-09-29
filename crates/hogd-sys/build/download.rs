//! Archive download with retry, shared by the build script and its tests.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// curl's exit status for an HTTP error response (`--fail` on a 4xx/5xx).
const CURL_EXIT_HTTP_ERROR: i32 = 22;

/// Download a file with retry and exponential backoff.
///
/// Downloads to a `.tmp` file first and atomically renames on success, so
/// a partial download never corrupts the cache. A definitive 4xx response
/// (other than 408 or 429) is not retried. `curl` is the program to run;
/// `version` and the bypass hints only decorate the failure message.
pub fn download_with_retry(
    curl: &Path,
    url: &str,
    dest: &Path,
    version: &str,
    max_retries: u32,
    initial_backoff: Duration,
) -> Result<(), String> {
    let tmp = dest.with_extension("tmp");
    if tmp.exists() {
        let _ = fs::remove_file(&tmp);
    }

    let mut wait = initial_backoff;
    let mut attempt = 0;
    let reason = loop {
        attempt += 1;
        let mut cmd = Command::new(curl);
        cmd.arg("--fail")
            .arg("-L")
            .arg("-o")
            .arg(&tmp)
            .arg("-w")
            .arg("%{http_code}")
            .arg(url)
            .stderr(std::process::Stdio::inherit());

        match cmd.output() {
            Ok(out) if out.status.success() && tmp.exists() => {
                // Atomic rename
                fs::rename(&tmp, dest)
                    .map_err(|e| format!("rename failed: {e}"))?;
                return Ok(());
            }
            Ok(out) => {
                let _ = fs::remove_file(&tmp);
                let code = out.status.code().unwrap_or(-1);
                let http_status = String::from_utf8_lossy(&out.stdout)
                    .trim()
                    .parse::<u16>()
                    .unwrap_or(0);
                let definitive = code == CURL_EXIT_HTTP_ERROR
                    && (400..500).contains(&http_status)
                    && !matches!(http_status, 408 | 429);
                if definitive {
                    break format!(
                        "the server returned HTTP {http_status}; not retrying"
                    );
                }
                if attempt > max_retries {
                    break match http_status {
                        0 => format!(
                            "gave up after {attempt} attempts (last curl exit {code})"
                        ),
                        _ => format!(
                            "gave up after {attempt} attempts (last HTTP {http_status})"
                        ),
                    };
                }
                println!(
                    "cargo:warning=download attempt {attempt} failed (exit {code}), retrying in {}s...",
                    wait.as_secs()
                );
                std::thread::sleep(wait);
                wait *= 2;
            }
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                break format!("curl not found or failed to execute: {e}");
            }
        }
    };

    let file = dest
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| dest.display().to_string());
    Err(format!(
        "{reason}\n\
         \x20 file:    {file}\n\
         \x20 version: test-ffi v{version}\n\
         \x20 url:     {url}\n\
         To build without downloading, set one of:\n\
         \x20 HOGD_FFI_LIB_DIR=<dir> : use libtest_ffi and bindings.rs from <dir>\n\
         \x20 HOGD_FFI_BUILD=1       : build from source \
         (requires `--features source-build`)"
    ))
}
