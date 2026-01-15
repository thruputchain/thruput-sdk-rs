use ::bindgen;
use ::cmake;

// Compiler should be selected by setting CC/CXX environment vars

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::env;
    use std::path::PathBuf;

    let hogd_src =
        env::var("HOGD_SRC").unwrap_or_else(|_| "../hogd/".to_string());
    println!("cargo:warning=HOGD_SRC is '{}'", hogd_src);
    println!("cargo:rerun-if-env-changed=HOGD_SRC");
    println!("cargo:rerun-if-changed={}", hogd_src);
    println!("cargo:rerun-if-changed=.");

    // native exec builds for host, not for the cargo target (which might be wasm)
    let target = std::env::var("TARGET").unwrap();

    println!("cargo:warning=TARGET is '{}'", target);

    if target == "wasm32-unknown-unknown" {
        return Err(
            "native_exec does not build for wasm32-unknown-unknown".into()
        );
    }

    let native_exec_bindings =
        PathBuf::from(hogd_src.clone()).join("hogd/test_ffi/test_bindings.h");

    // Generate Rust bindings from the C header file
    let bindings = bindgen::Builder::default()
        .header(native_exec_bindings.to_str().unwrap())
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to a Rust file
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    // Generate cmake build

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
        env::var("NATIVE_EXEC_ALWAYS_CONFIGURE").is_ok_and(|s| s.len() > 0),
    );

    // important: only build a limit set of targets.  If we do this wrong
    // we could easily wind up with some kind of nonsense recursive build.

    cfg.build_target("test_ffi");
    let dst = cfg.build();

    // Tell Rust where to find the built library
    println!(
        "cargo:rustc-link-search=native={}/build/hogd/test_ffi/",
        dst.display()
    );
    println!("cargo:rustc-link-lib=dylib=test_ffi");
    Ok(())
}
