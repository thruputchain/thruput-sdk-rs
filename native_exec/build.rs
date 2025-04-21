use ::bindgen;
use ::cmake;

// Build directory should be set with `cargo --target-dir`
// Compiler should be selected by setting CC/CXX environment vars

fn main() {
    use std::env;
    use std::path::PathBuf;

    let hogd_src = env::var("HOGD_SRC").unwrap_or_else(|_| "default".to_string());
    println!("cargo:warning=HOGD_SRC is '{}'", hogd_src);

    println!("cargo:rerun-if-env-changed=HOGD_SRC");

    let native_exec_bindings = PathBuf::from(hogd_src).join("hogd/test_ffi/test_bindings.h");

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
}