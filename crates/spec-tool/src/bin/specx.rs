//! Local dev tool: extract and canonicalize the contract spec from an rv32
//! ELF, mirroring the Thruput CLI's `finalize_rv32_spec` pipeline.
//!
//! Usage: `specx <elf> <out_spec_json>`

use thruput_spec_tool::{
    SpecSection, check_raw_spec_format, classify_section, render_spec_json,
};

struct Sect {
    name: String,
    offset: u32,
    size: u32,
    sh_type: u32,
}

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Minimal ELF32-LE section-table reader (same fields the CLI uses).
fn parse_sections(data: &[u8]) -> Vec<Sect> {
    assert!(data.len() >= 52, "not an ELF");
    assert_eq!(&data[0..4], b"\x7fELF", "not an ELF");
    let e_shoff = u32le(data, 32) as usize;
    let e_shentsize = u16le(data, 46) as usize;
    let e_shnum = u16le(data, 48) as usize;
    let e_shstrndx = u16le(data, 50) as usize;
    let shdr = |i: usize| -> (u32, u32, u32, u32, u32) {
        let base = e_shoff + i * e_shentsize;
        (
            u32le(data, base),      // name off
            u32le(data, base + 4),  // type
            u32le(data, base + 16), // offset
            u32le(data, base + 20), // size
            u32le(data, base + 8),  // flags
        )
    };
    let (_, _, strtab_off, strtab_size, _) = shdr(e_shstrndx);
    let strtab =
        &data[strtab_off as usize..(strtab_off + strtab_size) as usize];
    (0..e_shnum)
        .map(|i| {
            let (name_off, sh_type, offset, size, _flags) = shdr(i);
            let name_bytes = &strtab[name_off as usize..];
            let end = name_bytes
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(name_bytes.len());
            Sect {
                name: String::from_utf8_lossy(&name_bytes[..end]).into_owned(),
                offset,
                size,
                sh_type,
            }
        })
        .collect()
}

const SHT_NOBITS: u32 = 8;

fn main() {
    let mut args = std::env::args().skip(1);
    let elf_path = args.next().expect("usage: specx <elf> <out_json>");
    let out_path = args.next().expect("usage: specx <elf> <out_json>");
    let bytes = std::fs::read(&elf_path).expect("read elf");

    let mut raw_fragments = Vec::new();
    let mut final_data: Option<&[u8]> = None;
    for section in parse_sections(&bytes) {
        let kind = classify_section(&section.name);
        if kind == SpecSection::Other || section.sh_type == SHT_NOBITS {
            continue;
        }
        let start = section.offset as usize;
        let end = start + section.size as usize;
        let data = &bytes[start..end];
        match kind {
            SpecSection::Raw(format) => {
                check_raw_spec_format(format).expect("raw spec format skew");
                raw_fragments.extend(
                    thruput_spec_tool::parse_raw_fragments(data)
                        .expect("parse raw fragments"),
                );
            }
            SpecSection::Final => {
                assert!(
                    final_data.replace(data).is_none(),
                    "multiple finalized spec sections"
                );
            }
            SpecSection::Other => {}
        }
    }

    let spec = if !raw_fragments.is_empty() {
        match thruput_spec_tool::canonicalize_spec(&raw_fragments) {
            Ok(spec) => Some(spec),
            Err(thruput_spec_tool::SpecToolError::NoSpecFragments) => None,
            Err(e) => panic!("canonicalize failed: {e}"),
        }
    } else {
        final_data.map(|d| {
            xdrs::txs::decode(d).expect("decode finalized spec section")
        })
    };

    match spec {
        Some(spec) => {
            std::fs::write(&out_path, render_spec_json(&spec)).expect("write");
            eprintln!("specx: wrote {out_path}");
        }
        None => {
            eprintln!("specx: no spec in {elf_path}");
            std::process::exit(3);
        }
    }
}
