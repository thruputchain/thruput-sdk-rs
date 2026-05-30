// Publishes the linker script search path so downstream bin crates can
// reference `groundhog-rv32.ld` by name without copying it. Following
// the cortex-m-rt pattern: the library exposes the script file; the
// downstream bin crate's own build.rs (or .cargo/config.toml) is
// responsible for the `-T` flag and the bare-metal link flags. The
// example-contracts workspace config shows the expected Cargo flags.

fn main() {
    println!("cargo:rerun-if-changed=link/groundhog-rv32.ld");
    println!("cargo:rerun-if-changed=build.rs");

    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.starts_with("riscv32") {
        return;
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    // Make the linker script discoverable to any bin further up the
    // dep tree without that bin needing an absolute path.
    println!("cargo:rustc-link-search={manifest_dir}/link");
}
