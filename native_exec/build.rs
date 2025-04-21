use ::bindgen;
use ::cmake;

// Build directory should be set with `cargo --target-dir`
// Compiler should be selected by setting CC/CXX environment vars

fn main() {
    use std::env;
    use std::path::PathBuf;
    // Generate Rust bindings from the C header file
    let bindings = bindgen::Builder::default()
        .header("../protocol-rs/protocol/native_exec_bindings.h")
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to a Rust file
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}