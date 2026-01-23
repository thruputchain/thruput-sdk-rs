use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

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

fn shared_lib_suffix() -> &'static str {
    match &target_os() as &str {
        "windows" => ".dll",
        "macos" => ".dylib",
        _ => ".so",
    }
}

fn shared_lib_name() -> String {
    format!("{}test_ffi.{}", shared_lib_prefix(), shared_lib_suffix())
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

    // Generate bindings first (needed for all methods)
    let hogd_src =
        env::var("HOGD_SRC").unwrap_or_else(|_| "../hogd/".to_string());
    println!("cargo:warning=HOGD_SRC is '{}'", hogd_src);
    println!("cargo:rerun-if-changed={}", hogd_src);
    println!("cargo:rerun-if-changed=.");

    let native_exec_bindings =
        PathBuf::from(&hogd_src).join("hogd/test_ffi/test_bindings.h");

    // Parse version from header file
    let hogd_ffi_version = parse_version_from_header(&native_exec_bindings)?;
    println!(
        "cargo:warning=Parsed test_ffi version: {}",
        hogd_ffi_version
    );

    // Generate Rust bindings from the C header file
    let bindings = bindgen::Builder::default()
        .header(native_exec_bindings.to_str().unwrap())
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    // Execute the chosen build method
    if let Some(dir) = lib_dir {
        println!("cargo:warning=Using custom library directory");
        use_prebuilt_lib(&dir)?;
    } else if use_prebuilt {
        println!(
            "cargo:warning=Platform {}-{} supports prebuilt libraries, downloading...",
            target_arch(),
            target_os()
        );
        verify_hogd_version(&hogd_src, &hogd_ffi_version)?;
        download_and_extract(
            &out_dir,
            &target_arch(),
            &target_os(),
            &hogd_ffi_version,
        )?;
    } else {
        println!("cargo:warning=Building from source (HOGD_FFI_BUILD=1)");
        verify_hogd_version(&hogd_src, &hogd_ffi_version)?;
        build_from_cmake(&hogd_src)?;
    }

    Ok(())
}

/// Verify that the hogd submodule has the correct release tag
fn verify_hogd_version(
    hogd_src: &str,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let expected_tag = format!("test-ffi-v{}", version);

    let output = Command::new("git")
        .args(&["describe", "--tags", "--exact-match"])
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

/// Download and extract the prebuilt library from GitHub releases
fn download_and_extract(
    out_dir: &PathBuf,
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
        "https://github.com/groundhogchain/hogd/releases/download/test-ffi-v{}/{}",
        version, archive_name
    );

    println!(
        "cargo:warning=Downloading prebuilt library from: {}",
        download_url
    );

    // =============================================================================
    // TODO: SWITCH TO DIRECT HTTP DOWNLOADS WHEN REPO BECOMES PUBLIC
    // =============================================================================
    // Currently using gh CLI because the groundhogchain/hogd repository is private.
    // Once the repository is made public, replace the gh CLI code below with the
    // commented-out reqwest HTTP download code for better portability and fewer
    // dependencies.
    //
    // Uncomment this code when the repo is public:
    //
    // // Download the archive
    // let response = reqwest::blocking::get(&download_url)?;
    //
    // if !response.status().is_success() {
    //     return Err(format!(
    //         "Failed to download prebuilt library: HTTP {}\n\
    //          URL: {}\n\
    //          This may indicate that version {} does not have prebuilt releases.\n\
    //          Please use HOGD_FFI_BUILD=1 to build from source instead.",
    //         response.status(),
    //         download_url,
    //         version
    //     )
    //     .into());
    // }
    //
    // let archive_bytes = response.bytes()?;
    //
    // // Download and verify checksum
    // let checksum_url = format!("{}.sha256", download_url);
    // let checksum_response = reqwest::blocking::get(&checksum_url)?;
    //
    // if checksum_response.status().is_success() {
    //     let expected_checksum = checksum_response.text()?.split_whitespace().next().unwrap().to_string();
    //
    //     use sha2::{Sha256, Digest};
    //     let mut hasher = Sha256::new();
    //     hasher.update(&archive_bytes);
    //     let actual_checksum = format!("{:x}", hasher.finalize());
    //
    //     if actual_checksum != expected_checksum {
    //         return Err(format!(
    //             "Checksum verification failed!\n\
    //              Expected: {}\n\
    //              Actual:   {}",
    //             expected_checksum, actual_checksum
    //         )
    //         .into());
    //     }
    //     println!("cargo:warning=Checksum verified: {}", actual_checksum);
    // } else {
    //     println!("cargo:warning=Checksum file not found, skipping verification");
    // }
    // =============================================================================

    // TEMPORARY: Use gh CLI to download from private repository
    let archive_path = out_dir.join(archive_name);
    let checksum_path = out_dir.join(format!("{}.sha256", archive_name));

    // Download the archive using gh CLI only if it doesn't exist
    if !archive_path.exists() {
        println!("cargo:warning=Downloading archive...");
        let output = Command::new("gh")
            .args(&[
                "release",
                "download",
                &format!("test-ffi-v{}", version),
                "--repo",
                "groundhogchain/hogd",
                "--pattern",
                archive_name,
                "--dir",
                out_dir.to_str().unwrap(),
            ])
            .output()?;

        if !output.status.success() {
            return Err(format!(
                "Failed to download prebuilt library using gh CLI: {}\n\
                 URL: {}\n\
                 Error: {}\n\
                 Please ensure gh CLI is installed and authenticated, or use HOGD_FFI_BUILD=1 to build from source.",
                String::from_utf8_lossy(&output.stderr),
                download_url,
                version
            )
            .into());
        }
    } else {
        println!("cargo:warning=Archive already exists, skipping download");
    }

    // Download the checksum file using gh CLI only if it doesn't exist
    if !checksum_path.exists() {
        println!("cargo:warning=Downloading checksum...");
        let checksum_output = Command::new("gh")
            .args(&[
                "release",
                "download",
                &format!("test-ffi-v{}", version),
                "--repo",
                "groundhogchain/hogd",
                "--pattern",
                &format!("{}.sha256", archive_name),
                "--dir",
                out_dir.to_str().unwrap(),
            ])
            .output()?;

        if !checksum_output.status.success() {
            println!(
                "cargo:warning=Checksum file download failed, will skip verification"
            );
        }
    } else {
        println!(
            "cargo:warning=Checksum file already exists, skipping download"
        );
    }

    // Read the archive for checksum verification
    let archive_bytes = fs::read(&archive_path)?;

    // Verify checksum if available
    if checksum_path.exists() {
        let checksum_content = fs::read_to_string(&checksum_path)?;
        let expected_checksum = checksum_content
            .split_whitespace()
            .next()
            .unwrap()
            .to_string();

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&archive_bytes);
        let actual_checksum = format!("{:x}", hasher.finalize());

        if actual_checksum != expected_checksum {
            return Err(format!(
                "Checksum verification failed!\n\
                 Expected: {}\n\
                 Actual:   {}",
                expected_checksum, actual_checksum
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
    let extract_dir = out_dir.join("hogd_lib");
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
