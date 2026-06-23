//! Builds the ERC20 interface as an rv32 spec-carrier and asserts the
//! extracted spec covers the whole interface. Requires a nightly toolchain
//! with the `riscv32im-unknown-none-elf` target + `rust-src`; otherwise skips.

use std::process::Command;

fn rv32_target_available() -> bool {
    Command::new("rustc")
        .args(["--print", "target-list"])
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|t| t == "riscv32im-unknown-none-elf")
        })
        .unwrap_or(false)
}

#[test]
fn erc20_interface_rv32_spec_is_complete() {
    if !rv32_target_available() {
        eprintln!("skipping: riscv32im-unknown-none-elf target not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let target_dir = dir.path().join("target");

    let status = Command::new(env!("CARGO"))
        .args([
            "build",
            "-p",
            "groundhog-erc20-if",
            "--bin",
            "groundhog-erc20-if",
            "--no-default-features",
            "--features",
            "entrypoints",
            "--target",
            "riscv32im-unknown-none-elf",
            "--release",
            "-Zbuild-std=core,alloc",
        ])
        .arg("--target-dir")
        .arg(&target_dir)
        .status()
        .unwrap();
    assert!(status.success(), "rv32 build failed");

    let elf = target_dir
        .join("riscv32im-unknown-none-elf/release/groundhog-erc20-if");
    let bytes = std::fs::read(&elf).unwrap();
    let spec = groundhog_spec_tool::elf::elf_to_spec(&bytes)
        .unwrap()
        .expect("ERC20 interface ELF must carry a spec");

    let iface = spec
        .interfaces
        .iter()
        .find(|i| std::str::from_utf8(&i.name).unwrap() == "ERC20")
        .expect("ERC20 interface present");
    for method in [
        "initialize",
        "transfer",
        "transfer_from",
        "adjust_allowance",
        "allowance",
        "get_balance_of",
        "get_balance",
        "mint",
        "burn",
        "total_supply",
    ] {
        assert!(
            iface
                .methods
                .iter()
                .any(|m| std::str::from_utf8(&m.name).unwrap() == method),
            "missing method {method}"
        );
    }
}
