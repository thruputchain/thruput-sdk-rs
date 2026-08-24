use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(feature = "source-build")]
const HOGD_REPO_URL: &str = "https://github.com/groundhogchain/hogd.git";

/// Hardcoded version for prebuilt downloads.
/// This must match the version in hogd/test_ffi/test_bindings.h.
const TEST_FFI_VERSION: &str = "0.23.0";

/// Must match the version expected by hogd/built_contracts' build scripts.
#[cfg(feature = "source-build")]
const GROUNDHOG_CLI_VERSION: &str = "0.9.0";

/// Clone hogd repository on-demand for build-from-source.
/// - If `HOGD_SRC` is set and directory exists, use it as-is
/// - If `HOGD_SRC` is set but empty/missing, clone there
/// - Otherwise, clone to `OUT_DIR/hogd_clone`
#[cfg(feature = "source-build")]
fn ensure_hogd_clone(
    out_dir: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let expected_tag = format!("test-ffi-v{TEST_FFI_VERSION}");

    // Check if HOGD_SRC is set
    if let Ok(hogd_src) = env::var("HOGD_SRC") {
        let hogd_path = PathBuf::from(&hogd_src);
        if hogd_path.exists() {
            // Repo already exists, use as-is (verify tag later)
            return Ok(hogd_path);
        }
        // HOGD_SRC set but empty - clone there
        clone_hogd_repo(&hogd_path, &expected_tag)?;
        return Ok(hogd_path);
    }

    // Default: clone to OUT_DIR/hogd_clone
    let hogd_path = out_dir.join("hogd_clone");
    if !hogd_path.exists() {
        clone_hogd_repo(&hogd_path, &expected_tag)?;
    }
    Ok(hogd_path)
}

#[cfg(feature = "source-build")]
fn clone_hogd_repo(
    target_dir: &Path,
    tag: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "cargo:warning=Cloning hogd repository to: {}",
        target_dir.display()
    );

    if let Some(parent) = target_dir.parent() {
        fs::create_dir_all(parent)?;
    }

    let output = Command::new("git")
        .args([
            "clone",
            "--recurse-submodules",
            "--branch",
            tag,
            HOGD_REPO_URL,
        ])
        .arg(target_dir)
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Failed to clone hogd repository: {}\n\
             Ensure git is installed and you have network access.",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    println!("cargo:warning=Successfully cloned hogd at tag: {tag}");
    Ok(())
}

/// Parse the `test_ffi` version from `test_bindings.h`.
/// Returns version string in the format "MAJOR.MINOR.PATCH"
#[cfg(feature = "source-build")]
fn parse_version_from_header(
    header_path: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(header_path)?;

    let mut major = None;
    let mut minor = None;
    let mut patch = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("#define TEST_FFI_VERSION_MAJOR") {
            major = line
                .split_whitespace()
                .nth(2)
                .and_then(|s| s.parse::<u32>().ok());
        } else if line.starts_with("#define TEST_FFI_VERSION_MINOR") {
            minor = line
                .split_whitespace()
                .nth(2)
                .and_then(|s| s.parse::<u32>().ok());
        } else if line.starts_with("#define TEST_FFI_VERSION_PATCH") {
            patch = line
                .split_whitespace()
                .nth(2)
                .and_then(|s| s.parse::<u32>().ok());
        }
    }

    match (major, minor, patch) {
        (Some(maj), Some(min), Some(pat)) => Ok(format!("{maj}.{min}.{pat}")),
        _ => Err("Could not parse version from test_bindings.h".into()),
    }
}

fn target_os() -> String {
    env::var("CARGO_CFG_TARGET_OS").unwrap()
}

fn target_arch() -> String {
    env::var("CARGO_CFG_TARGET_ARCH").unwrap()
}

fn shared_lib_prefix() -> &'static str {
    match &target_os() as &str {
        "windows" => "",
        _ => "lib",
    }
}

fn download_cache_dir() -> PathBuf {
    let dir: PathBuf = env::var_os("HOGD_DOWNLOAD_CACHE_DIR").map_or_else(
        || {
            PathBuf::from(env::var("OUT_DIR").unwrap())
                .join("hogd_download_cache")
        },
        Into::into,
    );

    dir.join(format!("test-ffi-v{TEST_FFI_VERSION}"))
}

fn shared_lib_suffix() -> &'static str {
    match &target_os() as &str {
        "windows" => ".dll",
        "macos" => ".dylib",
        _ => ".so",
    }
}

fn shared_lib_name() -> String {
    let prefix = shared_lib_prefix();
    let suffix = shared_lib_suffix();
    format!("{prefix}test_ffi{suffix}")
}

fn emit_link_config(lib_dir: &Path) {
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=test_ffi");
    println!("cargo:metadata=lib_dir={}", lib_dir.display());
}

fn staged_lib_dir(out_dir: &Path) -> PathBuf {
    out_dir.join(format!("hogd_lib_{TEST_FFI_VERSION}"))
}

/// Stage the selected shared library into a stable directory under OUT_DIR.
///
/// Executables load `libtest_ffi` from an rpath rooted in the crate build
/// output. If we link against an arbitrary source directory, an older staged
/// dylib can still win at runtime. Staging the chosen library into
/// `OUT_DIR/hogd_lib_*` keeps link-time and runtime resolution aligned across
/// download, custom-lib, and source-build flows.
fn stage_shared_lib(
    source_lib_dir: &Path,
    out_dir: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let lib_name = shared_lib_name();
    let src = source_lib_dir.join(&lib_name);
    if !src.exists() {
        return Err(
            format!("Shared library not found at {}", src.display()).into()
        );
    }

    let stage_dir = staged_lib_dir(out_dir);
    fs::create_dir_all(&stage_dir)?;
    let dst = stage_dir.join(&lib_name);

    if src != dst {
        fs::copy(&src, &dst)?;
        println!(
            "cargo:warning=Staged {} to {}",
            src.display(),
            dst.display()
        );
    }

    Ok(stage_dir)
}

/// Find and copy prebuilt `bindings.rs` from the extracted tarball directory.
/// Returns an error if `bindings.rs` is not found in the tarball.
fn find_prebuilt_bindings(
    extract_dir: &Path,
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let bindings_src = extract_dir.join("bindings.rs");
    let bindings_dst = out_dir.join("bindings.rs");

    if !bindings_src.exists() {
        return Err(format!(
            "bindings.rs not found in extracted tarball at: {}\n\
             The prebuilt tarball may be from an older version that doesn't include bindings.\n\
             Either:\n\
             - Update to a newer prebuilt tarball that includes bindings.rs, or\n\
             - Use HOGD_FFI_BUILD=1 to build from source (requires hogd submodule)",
            bindings_src.display()
        )
        .into());
    }

    fs::copy(&bindings_src, &bindings_dst)?;
    println!(
        "cargo:warning=Copied bindings.rs from tarball to: {}",
        bindings_dst.display()
    );

    Ok(())
}

/// Copy the shared library to the deps directory so that test binaries
/// can find it at runtime via @loader_path (macOS) or $ORIGIN (Linux)
/// rpath entries configured in .cargo/config.toml.
fn copy_lib_to_deps(lib_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    // OUT_DIR is <profile>/build/<pkg>-<hash>/out (cargo <= 1.98) or
    // <profile>/build/<pkg>/<hash>/out (cargo 1.99+), so a fixed number of
    // `parent()` hops is wrong for one layout or the other. Find the `build`
    // ancestor instead; its parent is the profile dir in both. (Our package
    // is not itself named `build`, so the first match is the right one.)
    let deps_dir = out_dir
        .ancestors()
        .find(|p| p.file_name() == Some(std::ffi::OsStr::new("build")))
        .and_then(|build| build.parent())
        .ok_or_else(|| {
            format!(
                "OUT_DIR {} has no `build` ancestor; cannot locate <profile>/deps",
                out_dir.display()
            )
        })?
        .join("deps");

    let lib_name = shared_lib_name();
    let src = lib_dir.join(&lib_name);
    let dst = deps_dir.join(&lib_name);

    if src.exists() {
        // On a cold target dir the build script can run before anything has
        // created <profile>/deps (cargo 1.99+ no longer pre-creates it).
        fs::create_dir_all(&deps_dir).map_err(|e| {
            format!("failed to create {}: {e}", deps_dir.display())
        })?;
        fs::copy(&src, &dst).map_err(|e| {
            format!(
                "failed to copy {} to {}: {e}",
                src.display(),
                dst.display()
            )
        })?;
        println!(
            "cargo:warning=Copied {} to {} for rpath resolution",
            src.display(),
            dst.display()
        );
    } else {
        println!(
            "cargo:warning=Shared library not found at {} — rpath resolution may fail at runtime",
            src.display()
        );
    }

    Ok(())
}

/// Which build strategy the build script should use.
enum BuildMethod {
    /// Download prebuilt binaries from `groundhog.dev`.
    Prebuilt,
    /// User pointed us at a directory containing the library.
    CustomLib(String),
    /// Build the C++ library from source via cmake.
    Source,
}

/// Resolve the build method from env vars and platform support.
fn resolve_build_method() -> Result<BuildMethod, Box<dyn std::error::Error>> {
    let prebuilt_supported = matches!(
        (target_arch().as_str(), target_os().as_str()),
        ("x86_64" | "aarch64", "linux") | ("aarch64", "macos")
    );
    let build_from_source = env::var("HOGD_FFI_BUILD")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);
    let lib_dir = env::var("HOGD_FFI_LIB_DIR").ok();

    if build_from_source && lib_dir.is_some() {
        return Err(
            "Multiple build methods specified. Please set only one of: \
             HOGD_FFI_BUILD or HOGD_FFI_LIB_DIR"
                .into(),
        );
    }

    if let Some(dir) = lib_dir {
        return Ok(BuildMethod::CustomLib(dir));
    }
    if build_from_source {
        return Ok(BuildMethod::Source);
    }
    if prebuilt_supported {
        return Ok(BuildMethod::Prebuilt);
    }

    let arch = target_arch();
    let os = target_os();
    Err(format!(
        "No prebuilt library available for {arch}-{os}.\n\
         Supported platforms: x86_64-linux, aarch64-linux, aarch64-macos\n\
         Please set one of the following environment variables:\n\
         - HOGD_FFI_BUILD=1        : Build from source using CMake\n\
         - HOGD_FFI_LIB_DIR=<path> : Use prebuilt library from specified directory",
    )
    .into())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target = env::var("TARGET").unwrap();
    println!("cargo:warning=TARGET is '{target}'");

    if target.starts_with("riscv32") {
        return Err(format!("native_exec does not build for {target}").into());
    }

    println!("cargo:rerun-if-env-changed=HOGD_FFI_BUILD");
    println!("cargo:rerun-if-env-changed=HOGD_FFI_LIB_DIR");
    println!("cargo:rerun-if-env-changed=HOGD_SRC");
    println!("cargo:rerun-if-env-changed=NATIVE_EXEC_ALWAYS_CONFIGURE");

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    match resolve_build_method()? {
        BuildMethod::Prebuilt => {
            run_prebuilt_download(&out_dir)?;
        }
        BuildMethod::CustomLib(dir) => {
            println!("cargo:warning=Using custom library directory");
            let lib_path = PathBuf::from(&dir);
            find_prebuilt_bindings(&lib_path, &out_dir)?;
            use_prebuilt_lib(&dir, &out_dir)?;
        }
        BuildMethod::Source => {
            #[cfg(feature = "source-build")]
            run_build_from_source(&out_dir)?;
            #[cfg(not(feature = "source-build"))]
            return Err(
                "HOGD_FFI_BUILD=1 requires the `source-build` feature so \
                 cmake and bindgen are available.\n\
                 Build with: cargo build -p groundhog-sys --features source-build"
                    .into(),
            );
        }
    }

    Ok(())
}

/// Prebuilt download path: acquire lock, download, extract, link.
fn run_prebuilt_download(
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let lock_path = out_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("groundhog-sys.fslock");
    let mut lockfile = fslock::LockFile::open(&lock_path)
        .expect("bug: could not open build lock file");
    lockfile.lock_with_pid().expect(
        "could not acquire build lock — another cargo build may be running",
    );

    let arch = target_arch();
    let os = target_os();
    println!(
        "cargo:warning=Platform {arch}-{os} supports prebuilt libraries, downloading...",
    );
    println!("cargo:warning=Using hardcoded version: {TEST_FFI_VERSION}");

    download_and_extract(out_dir, &arch, &os, TEST_FFI_VERSION)
}

/// Build-from-source path: clone hogd, generate bindings, cmake build.
#[cfg(feature = "source-build")]
fn run_build_from_source(
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:warning=Building from source (HOGD_FFI_BUILD=1)");

    let hogd_path = ensure_hogd_clone(out_dir)?;
    let hogd_src = hogd_path.to_str().unwrap();

    println!("cargo:warning=HOGD_SRC is '{hogd_src}'");
    println!("cargo:rerun-if-changed={hogd_src}");
    println!("cargo:rerun-if-changed=.");

    let native_exec_bindings = hogd_path.join("hogd/test_ffi/test_bindings.h");

    let hogd_ffi_version = parse_version_from_header(&native_exec_bindings)?;
    println!("cargo:warning=Parsed test_ffi version: {hogd_ffi_version}");

    if hogd_ffi_version != TEST_FFI_VERSION {
        return Err(format!(
            "Version mismatch between hogd header and build.rs constant.\n\
             Header version:   {hogd_ffi_version}\n\
             Constant version: {TEST_FFI_VERSION}\n\
             Please update TEST_FFI_VERSION in native_exec/build.rs to match the header.",
        )
        .into());
    }

    let bindings = bindgen::Builder::default()
        .header(native_exec_bindings.to_str().unwrap())
        .no_copy("FFIDataBuffer")
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    verify_hogd_version(hogd_src, &hogd_ffi_version)?;
    fetch_hogd_rust_deps(&hogd_path, out_dir)?;
    build_from_cmake(hogd_src, out_dir)?;

    Ok(())
}

/// Pre-fetch cargo dependencies for hogd's embedded Rust crates.
///
/// Mirrors the steps in hogd/scripts/fetch.sh:
///   1. cargo fetch -Zbuild-std              in built_contracts/
///   2. cargo fetch -Zbuild-std --target …   in built_contracts/
///   3. cargo fetch                           in hogd/rust/
///   4. cargo install groundhog-cli
#[cfg(feature = "source-build")]
fn fetch_hogd_rust_deps(
    hogd_path: &Path,
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let contracts_dir = hogd_path.join("built_contracts");
    if contracts_dir.exists() {
        println!("cargo:warning=Pre-fetching built_contracts dependencies...");

        let output = Command::new("cargo")
            .args(["fetch", "-Zbuild-std"])
            .env("CARGO_NET_GIT_FETCH_WITH_CLI", "true")
            .current_dir(&contracts_dir)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "cargo fetch -Zbuild-std in built_contracts failed: {stderr}"
            )
            .into());
        }

        let output = Command::new("cargo")
            .args([
                "fetch",
                "-Zbuild-std",
                "--target",
                "riscv32im-unknown-none-elf",
            ])
            .env("CARGO_NET_GIT_FETCH_WITH_CLI", "true")
            .current_dir(&contracts_dir)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "cargo fetch -Zbuild-std --target \
                 riscv32im-unknown-none-elf in built_contracts failed: \
                 {stderr}"
            )
            .into());
        }
    } else {
        println!(
            "cargo:warning=built_contracts not found, skipping \
             contracts pre-fetch"
        );
    }

    let rust_dir = hogd_path.join("hogd/rust");
    if rust_dir.exists() {
        println!("cargo:warning=Pre-fetching hogd Rust dependencies...");

        let output = Command::new("cargo")
            .args(["fetch"])
            .env("CARGO_NET_GIT_FETCH_WITH_CLI", "true")
            .current_dir(&rust_dir)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(
                format!("cargo fetch in hogd/rust failed: {stderr}").into()
            );
        }
    } else {
        println!(
            "cargo:warning=hogd/rust not found, skipping \
             pre-fetch"
        );
    }

    let cli_root = out_dir.join("groundhog-cli");
    let cli_bin_path = cli_root.join("bin/groundhog");
    let expected_ver = format!("groundhog {GROUNDHOG_CLI_VERSION}");
    let needs_install = if cli_bin_path.exists() {
        let output =
            Command::new(&cli_bin_path).arg("--version").output().ok();
        let installed_ver = output
            .as_ref()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        if installed_ver == expected_ver {
            println!(
                "cargo:warning=groundhog-cli {GROUNDHOG_CLI_VERSION} \
                 already installed, skipping",
            );
            false
        } else {
            println!(
                "cargo:warning=groundhog-cli version mismatch \
                 (have '{installed_ver}', want '{expected_ver}'), reinstalling",
            );
            true
        }
    } else {
        true
    };

    if needs_install {
        println!(
            "cargo:warning=Installing groundhog-cli to {}...",
            cli_root.display()
        );
        let pkg = format!("groundhog-cli@{GROUNDHOG_CLI_VERSION}");
        // --locked: build with the CLI's published Cargo.lock. Without it,
        // cargo re-resolves transitive deps to the latest compatible versions,
        // which can pair the CLI's pinned deps with newer registry crates they
        // don't compile against.
        let output = Command::new("cargo")
            .args([
                "install",
                "--locked",
                "--root",
                cli_root.to_str().unwrap(),
                "--registry",
                "groundhog",
                &pkg,
            ])
            .env(
                "CARGO_REGISTRIES_GROUNDHOG_INDEX",
                "sparse+https://groundhog.dev/crates/api/v1/crates/",
            )
            .env("CARGO_NET_GIT_FETCH_WITH_CLI", "true")
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "cargo install groundhog-cli failed: {stderr}"
            )
            .into());
        }
    }

    Ok(())
}

/// Verify that the hogd submodule has the correct release tag.
#[cfg(feature = "source-build")]
fn verify_hogd_version(
    hogd_src: &str,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let expected_tag = format!("test-ffi-v{version}");

    let output = Command::new("git")
        .args(["describe", "--tags", "--exact-match"])
        .current_dir(hogd_src)
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "hogd submodule is not at a release tag. Expected tag: {expected_tag}\n\
             Please checkout the correct tag in the hogd submodule:\n\
             cd {hogd_src} && git checkout {expected_tag}",
        )
        .into());
    }

    let current_tag = String::from_utf8(output.stdout)?.trim().to_string();

    if current_tag != expected_tag {
        return Err(format!(
            "hogd submodule version mismatch.\n\
             Expected tag: {expected_tag}\n\
             Current tag:  {current_tag}\n\
             Please checkout the correct tag:\n\
             cd {hogd_src} && git checkout {expected_tag}",
        )
        .into());
    }

    println!("cargo:warning=hogd version verified: {current_tag}");
    Ok(())
}

/// Use a prebuilt library from a specified directory.
fn use_prebuilt_lib(
    lib_dir: &str,
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let lib_path = PathBuf::from(lib_dir);

    if !lib_path.exists() {
        return Err(
            format!("Library directory does not exist: {lib_dir}").into()
        );
    }

    // Determine the expected library filename based on target platform
    let expected_lib = shared_lib_name();

    let lib_file = lib_path.join(&expected_lib);
    if !lib_file.exists() {
        let arch = target_arch();
        let os = target_os();
        return Err(format!(
            "Could not find expected library file '{expected_lib}' \
             for target '{arch}' os '{os}' in directory: {lib_dir}\n\
             Please ensure the correct library file exists.",
        )
        .into());
    }

    let staged_dir = stage_shared_lib(&lib_path, out_dir)?;
    emit_link_config(&staged_dir);
    copy_lib_to_deps(&staged_dir)?;
    println!(
        "cargo:warning=Using prebuilt library from: {lib_dir} (found: {expected_lib}); staged at {}",
        staged_dir.display(),
    );

    Ok(())
}

/// The curl config line carrying credentials, or `None` when unauthenticated.
///
/// Credentials go to curl via `--config -` on stdin, never on the command
/// line: argv is world-readable through `ps` while the download runs.
fn curl_auth_config(user: &str) -> Option<String> {
    let pass = env::var("HOGD_FFI_PASSWORD").ok()?;
    if user.is_empty() {
        return None;
    }
    println!(
        "cargo:warning=Using credentials from HOGD_FFI_USER and HOGD_FFI_PASSWORD env variables."
    );
    Some(format!(
        "user = \"{}:{}\"\n",
        curl_config_quote(user),
        curl_config_quote(&pass)
    ))
}

/// Escape a value for use inside a double-quoted curl config string.
fn curl_config_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

/// Download a file with retry and exponential backoff.
///
/// Downloads to a `.tmp` file first and atomically renames on success, so
/// a partial download never corrupts the cache.
fn download_with_retry(
    url: &str,
    dest: &Path,
    max_retries: u32,
    user: &str,
) -> Result<(), String> {
    let tmp = dest.with_extension("tmp");
    if tmp.exists() {
        let _ = fs::remove_file(&tmp);
    }

    let auth_config = curl_auth_config(user);
    if auth_config.is_none() {
        println!(
            "cargo:warning=HOGD_FFI_PASSWORD env variable not set. Attempting to download without authentication."
        );
    }

    let mut wait_secs = 5;
    for attempt in 0..=max_retries {
        let mut cmd = Command::new("curl");
        cmd.arg("--fail").arg("-L").arg("-o").arg(&tmp).arg(url);

        let status = match &auth_config {
            None => cmd.status(),
            Some(config) => {
                cmd.arg("--config").arg("-");
                cmd.stdin(std::process::Stdio::piped());
                cmd.spawn().and_then(|mut child| {
                    use std::io::Write as _;
                    // Taken (not borrowed) so the pipe closes once written;
                    // curl reads config until EOF before starting.
                    if let Some(mut stdin) = child.stdin.take() {
                        stdin.write_all(config.as_bytes())?;
                    }
                    child.wait()
                })
            }
        };

        match status {
            Ok(s) if s.success() && tmp.exists() => {
                // Atomic rename
                fs::rename(&tmp, dest)
                    .map_err(|e| format!("rename failed: {e}"))?;
                return Ok(());
            }
            Ok(s) => {
                let _ = fs::remove_file(&tmp);
                if attempt < max_retries {
                    let code = s.code().unwrap_or(-1);
                    let next = attempt + 1;
                    println!(
                        "cargo:warning=download attempt {next} failed (exit {code}), retrying in {wait_secs}s...",
                    );
                    std::thread::sleep(std::time::Duration::from_secs(
                        wait_secs,
                    ));
                    wait_secs *= 2;
                }
            }
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                return Err(format!(
                    "curl not found or failed to execute: {e}"
                ));
            }
        }
    }

    let total = max_retries + 1;
    Err(format!("download failed after {total} attempts: {url}"))
}

/// Download a file if not already cached, using URL-based invalidation.
fn ensure_cached_download(
    url: &str,
    dest: &Path,
    user: &str,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let sum_path = dest.with_extension("sum");
    let cache_valid = dest.exists()
        && sum_path.exists()
        && fs::read_to_string(&sum_path)
            .map(|cached| cached == url)
            .unwrap_or(false);

    if cache_valid {
        println!(
            "cargo:warning={label} cache hit at {}, skipping download",
            dest.display()
        );
        return Ok(());
    }

    // Stale or missing cache — remove and re-download
    let _ = fs::remove_file(dest);
    let _ = fs::remove_file(&sum_path);

    println!("cargo:warning=Downloading {label} from: {url}");

    download_with_retry(url, dest, 3, user)
        .map_err(|e| format!("{label} download failed: {e}"))?;

    fs::write(&sum_path, url)
        .map_err(|e| format!("failed to write {label} cache sum: {e}"))?;
    Ok(())
}

/// True when the user explicitly opted out of checksum verification.
fn allow_unverified() -> bool {
    env::var("HOGD_FFI_ALLOW_UNVERIFIED")
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// Verify the archive against its `.sha256` checksum file.
fn verify_checksum(
    archive_path: &Path,
    checksum_path: &Path,
    archive_bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    if !checksum_path.exists() {
        if allow_unverified() {
            println!(
                "cargo:warning=Checksum file not found; HOGD_FFI_ALLOW_UNVERIFIED=1 is set, skipping verification"
            );
            return Ok(());
        }
        return Err(format!(
            "Checksum file not found at {}; refusing to use an unverified \
             archive. Set HOGD_FFI_ALLOW_UNVERIFIED=1 to skip verification \
             explicitly.",
            checksum_path.display()
        )
        .into());
    }

    let checksum_content = fs::read_to_string(checksum_path)?;
    let expected = checksum_content
        .split_whitespace()
        .next()
        .ok_or("Checksum file is empty")?;

    let mut hasher = Sha256::new();
    hasher.update(archive_bytes);
    let actual: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    if actual != expected {
        return Err(format!(
            "Checksum verification failed!\n\
             Expected: {expected}\n\
             Actual:   {actual}\n\
             Archive:  {}",
            archive_path.display()
        )
        .into());
    }
    println!("cargo:warning=Checksum verified: {actual}");
    Ok(())
}

/// Download and extract the prebuilt library from `groundhog.dev`.
fn download_and_extract(
    out_dir: &Path,
    target_arch: &str,
    target_os: &str,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let archive_name = match (target_arch, target_os) {
        ("x86_64", "linux") => "libtest_ffi-linux-x86_64.tar.gz",
        ("aarch64", "linux") => "libtest_ffi-linux-arm64.tar.gz",
        ("aarch64", "macos") => "libtest_ffi-macos-arm64.tar.gz",
        _ => {
            return Err(format!(
                "No prebuilt library available for {target_arch}-{target_os}.\n\
                 Supported platforms: x86_64-linux, aarch64-linux, aarch64-macos\n\
                 Please use HOGD_FFI_BUILD=1 to build from source instead.",
            )
            .into());
        }
    };

    let download_url = format!(
        "https://groundhog.dev/builds/test-ffi/test-ffi-v{version}/{archive_name}",
    );

    let cache_dir = download_cache_dir();
    fs::create_dir_all(&cache_dir)?;
    let archive_path = cache_dir.join(archive_name);

    let user =
        env::var("HOGD_FFI_USER").unwrap_or_else(|_| "marmot".to_string());

    println!("cargo:rerun-if-env-changed=HOGD_FFI_USER");
    println!("cargo:rerun-if-env-changed=HOGD_FFI_PASSWORD");

    ensure_cached_download(&download_url, &archive_path, &user, "Archive")?;

    // Download checksum. A failed or absent checksum is a hard error: an
    // unverifiable archive is indistinguishable from a tampered one, so the
    // opt-out has to be explicit.
    println!("cargo:rerun-if-env-changed=HOGD_FFI_ALLOW_UNVERIFIED");
    let checksum_url = format!("{download_url}.sha256");
    let checksum_path = cache_dir.join(format!("{archive_name}.sha256"));
    if let Err(e) = ensure_cached_download(
        &checksum_url,
        &checksum_path,
        &user,
        "Checksum",
    ) {
        let _ = fs::remove_file(&checksum_path);
        if allow_unverified() {
            println!(
                "cargo:warning=Checksum download failed ({e}); HOGD_FFI_ALLOW_UNVERIFIED=1 is set, skipping verification",
            );
        } else {
            return Err(format!(
                "Checksum download failed ({e}). Refusing to use an \
                 unverified archive; set HOGD_FFI_ALLOW_UNVERIFIED=1 to skip \
                 verification explicitly."
            )
            .into());
        }
    }

    let archive_bytes = fs::read(&archive_path)?;
    verify_checksum(&archive_path, &checksum_path, &archive_bytes)?;

    // Extract the archive
    let extract_dir = out_dir.join(format!("hogd_lib_{version}"));
    fs::create_dir_all(&extract_dir)?;

    // Shell out to `tar` (present on all supported prebuilt platforms),
    // mirroring how downloads shell out to `curl`.
    //
    // The artifact layout is fully under our control, so before extracting,
    // assert the archive contains exactly the expected flat filenames and
    // nothing else. This makes path traversal (absolute names, `..`,
    // link-based nesting) impossible without relying on host tar defaults,
    // and surfaces any unexpected layout change as a loud build error.
    let listing = Command::new("tar")
        .arg("-tzf")
        .arg(&archive_path)
        .output()
        .map_err(|e| format!("tar not found or failed to execute: {e}"))?;
    if !listing.status.success() {
        return Err(format!(
            "tar failed to list {} (exit {:?})",
            archive_path.display(),
            listing.status.code()
        )
        .into());
    }
    let mut expected_entries = vec![
        shared_lib_name(),
        "test_bindings.h".into(),
        "bindings.rs".into(),
    ];
    expected_entries.sort();
    let mut found_entries: Vec<String> =
        String::from_utf8_lossy(&listing.stdout)
            .lines()
            .filter(|entry| *entry != "./")
            .map(|entry| entry.strip_prefix("./").unwrap_or(entry).to_string())
            .collect();
    found_entries.sort();
    if found_entries != expected_entries {
        return Err(format!(
            "archive {} does not contain exactly the expected files.\n\
             Expected: {expected_entries:?}\n\
             Found:    {found_entries:?}",
            archive_path.display()
        )
        .into());
    }

    let status = Command::new("tar")
        .arg("-xzf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&extract_dir)
        .status()
        .map_err(|e| format!("tar not found or failed to execute: {e}"))?;
    if !status.success() {
        return Err(format!(
            "tar failed to extract {} (exit {:?})",
            archive_path.display(),
            status.code()
        )
        .into());
    }

    println!(
        "cargo:warning=Extracted library to: {}",
        extract_dir.display()
    );

    let lib_found = extract_dir.join("libtest_ffi.so").exists()
        || extract_dir.join("libtest_ffi.dylib").exists()
        || extract_dir.join("test_ffi.dll").exists();

    if !lib_found {
        return Err(format!(
            "Could not find libtest_ffi library in extracted archive at: {}",
            extract_dir.display()
        )
        .into());
    }

    find_prebuilt_bindings(&extract_dir, out_dir)?;

    emit_link_config(&extract_dir);
    copy_lib_to_deps(&extract_dir)?;

    Ok(())
}

/// Build the library from source using cmake.
#[cfg(feature = "source-build")]
fn build_from_cmake(
    hogd_src: &str,
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfg = cmake::Config::new(hogd_src);
    cfg.define("CMAKE_BUILD_TYPE", "Debug")
        // run cargo b -vv to see the full cmake output
        .define("CMAKE_VERBOSE_MAKEFILE", "ON")
        .define("CMAKE_OSX_SYSROOT", "")
        .define("ENABLE_TEST_FFI", "ON")
        .define("CMAKE_C_COMPILER_LAUNCHER", "ccache")
        .define("CMAKE_CXX_COMPILER_LAUNCHER", "ccache")
        .define("ENABLE_TEST_CONTRACTS", "OFF");

    // Force re-configuration before building, if requested
    cfg.always_configure(
        env::var("NATIVE_EXEC_ALWAYS_CONFIGURE").is_ok_and(|s| !s.is_empty()),
    );

    // Put the locally-installed groundhog-cli on PATH so hogd's cmake
    // contract builds can find it without touching the user's global cargo bin.
    let cli_bin = out_dir.join("groundhog-cli/bin");
    if cli_bin.exists() {
        let path = env::var("PATH").unwrap_or_default();
        cfg.env("PATH", format!("{}:{path}", cli_bin.display()));
    }

    // important: only build a limited set of targets.  If we do this wrong
    // we could easily wind up with some kind of nonsense recursive build.
    cfg.build_target("test_ffi");
    let dst = cfg.build();

    // Tell Rust where to find the built library
    let lib_dir = dst.join("build/hogd/test_ffi");
    let staged_dir = stage_shared_lib(&lib_dir, out_dir)?;
    emit_link_config(&staged_dir);
    copy_lib_to_deps(&staged_dir)?;
    println!(
        "cargo:warning=Built library from source at: {}; staged at {}",
        dst.display(),
        staged_dir.display()
    );

    Ok(())
}
