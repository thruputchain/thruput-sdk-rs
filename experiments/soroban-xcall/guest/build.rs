//! Derive a linker script from the SDK's, widening the CODE/DATA regions to
//! the VM ceilings (4 MiB code, 16 MiB data). The Soroban host does not fit
//! the SDK script's 1 MiB CODE region.
use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let sdk_ld = manifest.join("../../../crates/rv32-runtime/link/groundhog-rv32.ld");
    println!("cargo:rerun-if-changed={}", sdk_ld.display());
    let script = fs::read_to_string(&sdk_ld).unwrap();
    let widened = script
        .replace("CODE (rx) : ORIGIN = 0, LENGTH = 1M", "CODE (rx) : ORIGIN = 0, LENGTH = 4M")
        .replace("DATA (rw) : ORIGIN = 0, LENGTH = 1M", "DATA (rw) : ORIGIN = 0, LENGTH = 16M");
    assert_ne!(script, widened, "SDK linker script MEMORY regions changed; update build.rs");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out.join("groundhog-rv32-ceiling.ld"), widened).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
}
