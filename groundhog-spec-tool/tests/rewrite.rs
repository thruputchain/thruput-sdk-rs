use groundhog_spec_tool::{rewrite::rewrite_wasm, FINAL_SECTION_NAME, RAW_SECTION_NAME};
use wasm_encoder::{CustomSection, Module};
use wasmparser::{Parser, Payload};

fn custom_sections(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut sections = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::CustomSection(section) = payload.unwrap() {
            sections.push((section.name().to_owned(), section.data().to_vec()));
        }
    }
    sections
}

#[test]
fn rewrite_replaces_raw_section_with_one_final_section() {
    let mut module = Module::new();
    module.section(&CustomSection {
        name: RAW_SECTION_NAME.into(),
        data: b"iface\tdemo\tAlpha\tExplicitNamedInterface\t0.3.0\n\
                iface\tdemo\tAlpha\tVersionedCallee\t1.2.3\n"
            .as_slice()
            .into(),
    });

    let rewritten = rewrite_wasm(&module.finish()).unwrap();
    let sections = custom_sections(&rewritten);

    assert!(sections.iter().all(|(name, _)| name != RAW_SECTION_NAME));
    let final_sections: Vec<_> = sections
        .into_iter()
        .filter(|(name, _)| name == FINAL_SECTION_NAME)
        .collect();
    assert_eq!(final_sections.len(), 1);
    let body = String::from_utf8(final_sections[0].1.clone()).unwrap();
    assert!(body.contains("gh_iface_v1"));
    assert!(body.contains("ExplicitNamedInterface\t0.3.0"));
    assert!(body.contains("VersionedCallee\t1.2.3"));
}
