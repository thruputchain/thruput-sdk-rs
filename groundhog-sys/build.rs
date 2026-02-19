use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const HOGD_REPO_URL: &str = "https://github.com/groundhogchain/hogd.git";

/// Hardcoded version for prebuilt downloads.
/// This must match the version in hogd/test_ffi/test_bindings.h.
/// When updating, also update the version in README.md.
const TEST_FFI_VERSION: &str = "0.7.0";

/// Clone hogd repository on-demand for build-from-source.
/// - If HOGD_SRC is set and directory exists, use it as-is
/// - If HOGD_SRC is set but empty/missing, clone there
/// - Otherwise, clone to OUT_DIR/hogd_clone
fn ensure_hogd_clone(
    out_dir: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let expected_tag = format!("test-ffi-v{}", TEST_FFI_VERSION);

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

    println!("cargo:warning=Successfully cloned hogd at tag: {}", tag);
    Ok(())
}

/// Parse the test_ffi version from test_bindings.h
/// Returns version string in the format "MAJOR.MINOR.PATCH"
fn parse_version_from_header(
    header_path: &PathBuf,
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
        (Some(maj), Some(min), Some(pat)) => {
            Ok(format!("{}.{}.{}", maj, min, pat))
        }
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
    PathBuf::from(env::var("OUT_DIR").unwrap())
        .join("hogd_download_cache")
        .join(format!("test-ffi-v{}", TEST_FFI_VERSION))
}

fn shared_lib_suffix() -> &'static str {
    match &target_os() as &str {
        "windows" => ".dll",
        "macos" => ".dylib",
        _ => ".so",
    }
}

fn shared_lib_name() -> String {
    format!("{}test_ffi{}", shared_lib_prefix(), shared_lib_suffix())
}

/// Find and copy prebuilt bindings.rs from the extracted tarball directory.
/// Returns an error if bindings.rs is not found in the tarball.
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // native exec builds for host, not for the cargo target (which might be wasm)
    let target = env::var("TARGET").unwrap();

    println!("cargo:warning=TARGET is '{}'", target);

    if target == "wasm32-unknown-unknown" {
        return Err(
            "native_exec does not build for wasm32-unknown-unknown".into()
        );
    }

    println!("cargo:rerun-if-env-changed=HOGD_FFI_BUILD");
    println!("cargo:rerun-if-env-changed=HOGD_FFI_LIB_DIR");
    println!("cargo:rerun-if-env-changed=HOGD_SRC");
    println!("cargo:rerun-if-env-changed=NATIVE_EXEC_ALWAYS_CONFIGURE");

    // Check if this platform has prebuilt library support
    let prebuilt_supported = matches!(
        (target_arch().as_str(), target_os().as_str()),
        ("x86_64", "linux") | ("aarch64", "macos")
    );

    // Check user overrides
    let build_from_source = env::var("HOGD_FFI_BUILD")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);
    let lib_dir = env::var("HOGD_FFI_LIB_DIR").ok();

    // Validate that only one override is set
    if build_from_source && lib_dir.is_some() {
        return Err(
            "Multiple build methods specified. Please set only one of: \
             HOGD_FFI_BUILD or HOGD_FFI_LIB_DIR"
                .into(),
        );
    }

    // Determine build method
    let use_prebuilt = if lib_dir.is_some() {
        // User specified custom library directory
        false
    } else if build_from_source {
        // User explicitly requested building from source
        false
    } else if prebuilt_supported {
        // Platform supports prebuilt, and no override was specified
        true
    } else {
        // Platform doesn't support prebuilt and user didn't specify an alternative
        return Err(format!(
            "No prebuilt library available for {}-{}.\n\
             Supported platforms: x86_64-linux, aarch64-macos\n\
             Please set one of the following environment variables:\n\
             - HOGD_FFI_BUILD=1        : Build from source using CMake\n\
             - HOGD_FFI_LIB_DIR=<path> : Use prebuilt library from specified directory",
            target_arch(), target_os()
        )
        .into());
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    // =========================================================================
    // PREBUILT PATH: Download prebuilt library with bundled bindings.rs
    // This path does NOT require the hogd submodule to be present.
    // =========================================================================
    if use_prebuilt {
        println!(
            "cargo:warning=Platform {}-{} supports prebuilt libraries, downloading...",
            target_arch(),
            target_os()
        );
        println!(
            "cargo:warning=Using hardcoded version: {}",
            TEST_FFI_VERSION
        );

        // Download, extract, copy bindings.rs, and link library
        download_and_extract(
            &out_dir,
            &target_arch(),
            &target_os(),
            TEST_FFI_VERSION,
        )?;

        return Ok(());
    }

    // =========================================================================
    // CUSTOM LIB DIR PATH: Uses prebuilt library with bundled bindings.rs
    // =========================================================================
    if let Some(dir) = lib_dir {
        println!("cargo:warning=Using custom library directory");
        // Look for bindings.rs in lib dir (like prebuilt tarball)
        let lib_path = PathBuf::from(&dir);
        find_prebuilt_bindings(&lib_path, &out_dir)?;
        use_prebuilt_lib(&dir)?;
        return Ok(());
    }

    // =========================================================================
    // BUILD FROM SOURCE PATH: Clone hogd on-demand
    // =========================================================================
    println!("cargo:warning=Building from source (HOGD_FFI_BUILD=1)");

    let hogd_path = ensure_hogd_clone(&out_dir)?;
    let hogd_src = hogd_path.to_str().unwrap();

    println!("cargo:warning=HOGD_SRC is '{}'", hogd_src);
    println!("cargo:rerun-if-changed={}", hogd_src);
    println!("cargo:rerun-if-changed=.");

    let native_exec_bindings = hogd_path.join("hogd/test_ffi/test_bindings.h");

    // Parse version from header file
    let hogd_ffi_version = parse_version_from_header(&native_exec_bindings)?;
    println!(
        "cargo:warning=Parsed test_ffi version: {}",
        hogd_ffi_version
    );

    // Cross-check: parsed version must match hardcoded version
    if hogd_ffi_version != TEST_FFI_VERSION {
        return Err(format!(
            "Version mismatch between hogd header and build.rs constant.\n\
             Header version:   {}\n\
             Constant version: {}\n\
             Please update TEST_FFI_VERSION in native_exec/build.rs to match the header.",
            hogd_ffi_version, TEST_FFI_VERSION
        )
        .into());
    }

    // Generate Rust bindings from the C header file
    let bindings = bindgen::Builder::default()
        .header(native_exec_bindings.to_str().unwrap())
        .no_copy("FFIDataBuffer")
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    verify_hogd_version(hogd_src, &hogd_ffi_version)?;
    build_from_cmake(hogd_src)?;

    Ok(())
}

/// Verify that the hogd submodule has the correct release tag
fn verify_hogd_version(
    hogd_src: &str,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let expected_tag = format!("test-ffi-v{}", version);

    let output = Command::new("git")
        .args(["describe", "--tags", "--exact-match"])
        .current_dir(hogd_src)
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "hogd submodule is not at a release tag. Expected tag: {}\n\
             Please checkout the correct tag in the hogd submodule:\n\
             cd {} && git checkout {}",
            expected_tag, hogd_src, expected_tag
        )
        .into());
    }

    let current_tag = String::from_utf8(output.stdout)?.trim().to_string();

    if current_tag != expected_tag {
        return Err(format!(
            "hogd submodule version mismatch.\n\
             Expected tag: {}\n\
             Current tag:  {}\n\
             Please checkout the correct tag:\n\
             cd {} && git checkout {}",
            expected_tag, current_tag, hogd_src, expected_tag
        )
        .into());
    }

    println!("cargo:warning=hogd version verified: {}", current_tag);
    Ok(())
}

/// Use a prebuilt library from a specified directory
fn use_prebuilt_lib(lib_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    let lib_path = PathBuf::from(lib_dir);

    if !lib_path.exists() {
        return Err(
            format!("Library directory does not exist: {}", lib_dir).into()
        );
    }

    // Determine the expected library filename based on target platform
    let expected_lib = shared_lib_name();

    let lib_file = lib_path.join(expected_lib.clone());
    if !lib_file.exists() {
        return Err(format!(
            "Could not find expected library file '{}' for target '{}' os '{}' in directory: {}\n\
             Please ensure the correct library file exists.",
            expected_lib, target_arch(), target_os(), lib_dir
        )
        .into());
    }

    println!("cargo:rustc-link-search=native={}", lib_dir);
    println!("cargo:rustc-link-lib=dylib=test_ffi");
    println!(
        "cargo:warning=Using prebuilt library from: {} (found: {})",
        lib_dir, expected_lib
    );

    Ok(())
}

fn add_auth_to_curl(cmd: &mut Command, user: &str) {
    if let Ok(pass) = env::var("HOGD_FFI_PASSWORD") {
        if !user.is_empty() {
            println!(
                "cargo:warning=Using credentials from HOGD_FFI_USER and HOGD_FFI_PASSWORD env variables."
            );
            cmd.arg("-u").arg(format!("{}:{}", user, pass));
        }
    } else {
        println!(
            "cargo:warning=HOGD_FFI_PASSWORD env variable not set. Attempting to download without authentication."
        );
    }
}

/// Download and extract the prebuilt library from groundhog.dev
fn download_and_extract(
    out_dir: &Path,
    target_arch: &str,
    target_os: &str,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Determine the platform-specific archive name
    let archive_name = match (target_arch, target_os) {
        ("x86_64", "linux") => "libtest_ffi-linux-x86_64.tar.gz",
        ("aarch64", "macos") => "libtest_ffi-macos-arm64.tar.gz",
        _ => {
            return Err(format!(
                "No prebuilt library available for {}-{}.\n\
                 Supported platforms: x86_64-linux, aarch64-macos\n\
                 Please use HOGD_FFI_BUILD=1 to build from source instead.",
                target_arch, target_os
            )
            .into());
        }
    };

    let download_url = format!(
        "https://groundhog.dev/builds/test-ffi/test-ffi-v{}/{}",
        version, archive_name
    );

    let download_cache_dir = download_cache_dir();
    println!("cargo:rerun-if-changed={}", download_cache_dir.display());

    fs::create_dir_all(&download_cache_dir)?;
    let archive_path = download_cache_dir.join(archive_name);

    let user =
        env::var("HOGD_FFI_USER").unwrap_or_else(|_| "marmot".to_string());

    println!("cargo:rerun-if-env-changed=HOGD_FFI_USER");
    println!("cargo:rerun-if-env-changed=HOGD_FFI_PASSWORD");

    if !archive_path.exists() {
        println!(
            "cargo:warning=Downloading prebuilt library from: {}",
            download_url
        );
        println!(
            "cargo:warning=Downloading archive to {}",
            archive_path.display()
        );

        let mut cmd = Command::new("curl");
        cmd.arg("--fail").arg("-L").arg("-o").arg(&archive_path);

        add_auth_to_curl(&mut cmd, &user);

        cmd.arg(&download_url);

        let output = cmd.output()?;

        if !output.status.success() {
            return Err(format!(
                "Failed to download prebuilt library using curl: {}\nURL: {}\nError: {}",
                String::from_utf8_lossy(&output.stderr),
                download_url,
                output.status
            )
            .into());
        }
    } else {
        println!(
            "cargo:warning=Archive already exists at {}, skipping download",
            archive_path.display()
        );
    }

    // Download checksum
    let checksum_url = format!("{}.sha256", download_url);
    let checksum_path =
        download_cache_dir.join(format!("{}.sha256", archive_name));
    if !checksum_path.exists() {
        println!("cargo:warning=Downloading checksum from: {}", checksum_url);
        let mut cmd = Command::new("curl");
        cmd.arg("--fail").arg("-L").arg("-o").arg(&checksum_path);
        add_auth_to_curl(&mut cmd, &user);

        cmd.arg(&checksum_url);
        let output = cmd.output()?;
        if !output.status.success() {
            // curl -o creates the file even if the download fails, so we remove it.
            let _ = fs::remove_file(&checksum_path);
            return Err(format!(
                "Failed to download checksum file from: {}\nStderr: {}",
                checksum_url,
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        } else {
            println!(
                "cargo:warning=Checksum downloaded to {}",
                checksum_path.display()
            );
        }
    } else {
        println!(
            "cargo:warning=Checksum file already exists, skipping download."
        );
    }

    // Read the archive for checksum verification and extraction
    let archive_bytes = fs::read(&archive_path)?;

    // Verify checksum if available
    if checksum_path.exists() {
        let checksum_content = fs::read_to_string(&checksum_path)?;
        let expected_checksum = checksum_content
            .split_whitespace()
            .next()
            .ok_or("Checksum file is empty")?
            .to_string();

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&archive_bytes);
        let actual_checksum = format!("{:x}", hasher.finalize());

        if actual_checksum != expected_checksum {
            return Err(format!(
                "Checksum verification failed!\n\
                 Expected: {}\n\
                 Actual:   {}\n\
                 Archive:  {}",
                expected_checksum,
                actual_checksum,
                archive_path.display()
            )
            .into());
        }
        println!("cargo:warning=Checksum verified: {}", actual_checksum);
    } else {
        println!(
            "cargo:warning=Checksum file not found, skipping verification"
        );
    }

    // Extract the archive
    let extract_dir = out_dir.join(format!("hogd_lib_{}", version));
    fs::create_dir_all(&extract_dir)?;

    let tar = flate2::read::GzDecoder::new(&archive_bytes[..]);
    let mut archive = tar::Archive::new(tar);
    archive.unpack(&extract_dir)?;

    println!(
        "cargo:warning=Extracted library to: {}",
        extract_dir.display()
    );

    // Verify the library file exists
    let lib_file = extract_dir.join("libtest_ffi.so").exists()
        || extract_dir.join("libtest_ffi.dylib").exists()
        || extract_dir.join("test_ffi.dll").exists();

    if !lib_file {
        return Err(format!(
            "Could not find libtest_ffi library in extracted archive at: {}",
            extract_dir.display()
        )
        .into());
    }

    // Copy bindings.rs from the tarball to OUT_DIR
    find_prebuilt_bindings(&extract_dir, out_dir)?;

    println!("cargo:rustc-link-search=native={}", extract_dir.display());
    println!("cargo:rustc-link-lib=dylib=test_ffi");

    Ok(())
}

/// Build the library from source using CMake
fn build_from_cmake(hogd_src: &str) -> Result<(), Box<dyn std::error::Error>> {
    use cmake;

    let mut cfg = cmake::Config::new(hogd_src);
    cfg.define("CMAKE_BUILD_TYPE", "Debug")
        // run cargo b -vv to see the full cmake output
        .define("CMAKE_VERBOSE_MAKEFILE", "ON")
        .define("CMAKE_OSX_SYSROOT", "")
        .define("ENABLE_TEST_FFI", "ON")
        .define("CMAKE_C_COMPILER_LAUNCHER", "ccache")
        .define("CMAKE_CXX_COMPILER_LAUNCHER", "ccache")
        .define("ENABLE_TEST_CONTRACTS", "OFF");

    #[cfg(feature = "wasm_backtraces")]
    cfg.define("WASM_DEBUG_BACKTRACES", "ON");

    // Force re-configuration before building, if requested
    cfg.always_configure(
        env::var("NATIVE_EXEC_ALWAYS_CONFIGURE").is_ok_and(|s| !s.is_empty()),
    );

    // important: only build a limited set of targets.  If we do this wrong
    // we could easily wind up with some kind of nonsense recursive build.
    cfg.build_target("test_ffi");
    let dst = cfg.build();

    // Tell Rust where to find the built library
    println!(
        "cargo:rustc-link-search=native={}/build/hogd/test_ffi/",
        dst.display()
    );
    println!("cargo:rustc-link-lib=dylib=test_ffi");
    println!(
        "cargo:warning=Built library from source at: {}",
        dst.display()
    );

    Ok(())
}
