//! Checks on linked rv32 contract ELFs.
//!
//! These read the unstripped ELF that `thruput build` leaves under
//! `target/riscv32im-unknown-none-elf/release/`, not the lowered `.rv32a`.

use object::{Object, ObjectSection, ObjectSymbol, SymbolKind};

const ENTRYPOINTS_START: &str = "__thruput_rv32_entrypoints_start";
const ENTRYPOINTS_END: &str = "__thruput_rv32_entrypoints_end";

/// The message [`check_dispatch`] fails with when the table is empty.
pub const EMPTY_DISPATCH_TABLE: &str = "dispatch table is empty";

fn parse(data: &[u8]) -> Result<object::File<'_>, String> {
    object::File::parse(data).map_err(|e| format!("not an ELF: {e}"))
}

/// The contract uses guest-side dispatch: `_start` is the entry at address 0,
/// and the dispatch table is present and not empty.
///
/// A present `.rodata` alone proves little: string literals fill it even if
/// the dispatch section were dropped (by `--gc-sections`, a missing `KEEP`, or
/// a section-name mismatch), and then every call would reach the `ebreak`
/// backstop with no link error. So this compares the table's bracket symbols.
/// Those are link-time constants that a stripped ELF lacks; there it falls
/// back to the `.rodata` check and returns a warning.
pub fn check_dispatch(data: &[u8]) -> Result<Option<String>, String> {
    let file = parse(data)?;
    if file.entry() != 0 {
        return Err(format!(
            "_start not at address 0 (entry {:#x})",
            file.entry()
        ));
    }
    let address = |name: &str| {
        file.symbols()
            .find(|s| s.name() == Ok(name))
            .map(|s| s.address())
    };
    match (address(ENTRYPOINTS_START), address(ENTRYPOINTS_END)) {
        (Some(start), Some(end)) if end <= start => Err(format!(
            "{EMPTY_DISPATCH_TABLE} (end {end:#x} <= start {start:#x}): \
             section dropped or no methods registered"
        )),
        (Some(_), Some(_)) => Ok(None),
        _ if file
            .sections()
            .any(|s| s.name().is_ok_and(|n| n.contains(".rodata"))) =>
        {
            Ok(Some(
                "no dispatch table symbols (stripped?); only checked that \
                 _start is at 0 and .rodata is present"
                    .to_string(),
            ))
        }
        _ => {
            Err(".rodata missing: the dispatch table would be absent"
                .to_string())
        }
    }
}

/// The rv32 runtime's `memcpy`, `memmove` and `memset` overrides
/// (`crates/rv32-runtime/src/mem.rs`) won the link.
///
/// A WEAK definition means a `compiler_builtins` fallback won; ours are
/// GLOBAL, or LOCAL when LTO internalizes them. This catches a toolchain bump
/// that silently drops the override, and a build where `#![no_builtins]` no
/// longer stops the bodies from recursing into themselves. No mem symbols at
/// all is fine: nothing referenced them, and `--gc-sections` dropped them.
pub fn check_mem_symbols(data: &[u8]) -> Result<(), String> {
    let file = parse(data)?;
    for symbol in file.symbols() {
        let Ok(name) = symbol.name() else { continue };
        let is_mem = matches!(name, "memcpy" | "memmove" | "memset");
        if is_mem
            && symbol.kind() == SymbolKind::Text
            && symbol.is_definition()
            && symbol.is_weak()
        {
            return Err(format!(
                "WEAK {name} linked: the compiler_builtins fallback won"
            ));
        }
        // The current mangling of compiler_builtins' mem helpers.
        if name.contains("compiler_builtins3mem") {
            return Err(format!(
                "compiler_builtins mem symbol {name} present: the override \
                 was not applied"
            ));
        }
    }
    Ok(())
}

/// Embedded `.rs` paths, as `strings` would find them. Release builds use
/// `panic=immediate-abort`, so `#[track_caller]` and `panic!` locations
/// should leave none.
pub fn rs_paths(data: &[u8]) -> Vec<String> {
    let printable = |b: &u8| (0x20..0x7f).contains(b) || *b == b'\t';
    data.split(|b| !printable(b))
        .filter(|run| run.len() >= 4 && run.ends_with(b".rs"))
        .map(|run| String::from_utf8_lossy(run).into_owned())
        .collect()
}

pub fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_rs_paths_like_strings() {
        // `strings` reports runs of at least four printable bytes.
        let data = b"\0src/lib.rs\0.rs\0ok\0/a/b/main.rs\x01tail.rs.bak";
        assert_eq!(rs_paths(data), ["src/lib.rs", "/a/b/main.rs"]);
    }

    #[test]
    fn rejects_non_elf() {
        assert!(check_dispatch(b"not an elf").is_err());
    }
}
