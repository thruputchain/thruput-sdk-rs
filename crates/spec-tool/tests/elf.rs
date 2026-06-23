use groundhog_contract_spec::{
    ContractSpecMethod, ContractSpecString, ContractType, SpecFragment,
    SpecInterfaceFragment, SpecMethodFragment,
};
use groundhog_spec_tool::{elf::elf_to_spec, RAW_SECTION_NAME};
use object::write::{Object, SectionKind};
use object::{Architecture, BinaryFormat, Endianness};
use xdrs::txs::encode;

fn s(v: &str) -> ContractSpecString {
    ContractSpecString::try_from(v).unwrap()
}

fn raw_fragments() -> Vec<u8> {
    let iface = SpecFragment::SpecInterface {
        interface_fragment: SpecInterfaceFragment {
            interface_name: s("ERC20"),
            interface_version: s("0.1.3"),
            throws: vec![].try_into().unwrap(),
        },
    };
    let method = SpecFragment::SpecMethod {
        method_fragment: SpecMethodFragment {
            interface_name: s("ERC20"),
            interface_version: s("0.1.3"),
            method: ContractSpecMethod {
                name: s("total_supply"),
                args: vec![].try_into().unwrap(),
                return_type: ContractType::U128,
            },
        },
    };
    let mut bytes = encode(&iface).unwrap();
    bytes.extend(encode(&method).unwrap());
    bytes
}

fn elf_with_spec_section(section_bytes: &[u8]) -> Vec<u8> {
    let mut obj = Object::new(
        BinaryFormat::Elf,
        Architecture::Riscv32,
        Endianness::Little,
    );
    let sec = obj.add_section(
        Vec::new(),
        RAW_SECTION_NAME.as_bytes().to_vec(),
        SectionKind::ReadOnlyData,
    );
    obj.set_section_data(sec, section_bytes.to_vec(), 1);
    obj.write().unwrap()
}

#[test]
fn elf_to_spec_extracts_interface_and_methods() {
    let elf = elf_with_spec_section(&raw_fragments());
    let spec = elf_to_spec(&elf).unwrap().expect("spec present");
    assert_eq!(spec.interfaces.len(), 1);
    let iface = &spec.interfaces[0];
    assert_eq!(std::str::from_utf8(&iface.name).unwrap(), "ERC20");
    assert!(iface
        .methods
        .iter()
        .any(|m| std::str::from_utf8(&m.name).unwrap() == "total_supply"));
}

#[test]
fn elf_with_empty_spec_section_returns_none() {
    let elf = elf_with_spec_section(&[]);
    assert!(elf_to_spec(&elf).unwrap().is_none());
}

fn elf_without_any_spec_section() -> Vec<u8> {
    let obj = Object::new(BinaryFormat::Elf, Architecture::Riscv32, Endianness::Little);
    obj.write().unwrap()
}

#[test]
fn elf_without_spec_section_returns_none() {
    let elf = elf_without_any_spec_section();
    assert!(elf_to_spec(&elf).unwrap().is_none());
}
