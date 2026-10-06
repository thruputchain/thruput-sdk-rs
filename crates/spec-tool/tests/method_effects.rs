use groundhog_contract_spec::{
    ContractMethodEffect, ContractSpec, SpecFragmentView,
    SpecInterfaceAnchorView, SpecInterfaceFragmentView,
    SpecMethodFragmentView,
};
use groundhog_spec_tool::{
    canonicalize_spec, parse_raw_fragments, render_spec_json,
};

#[sdk::contract(version = "1.0.0")]
pub trait Effects {
    #[contract(effect = "pure")]
    fn calculate(&self, value: u32) -> u32;
    #[contract(effect = "readOnly")]
    fn read(&self) -> u32;
    #[contract(effect = "stateChanging")]
    fn update(&self, value: u32);
    fn undeclared(&mut self);
}

fn fragments() -> Vec<groundhog_contract_spec::SpecFragment> {
    let anchor = __GROUNDHOG_IFACE_ANCHOR_Effects;
    let mut raw = Vec::new();
    let mut emit =
        |fragment| raw.extend(xdrs::txs::encode(&fragment).unwrap());
    emit(SpecFragmentView::Interface(SpecInterfaceFragmentView {
        interface_name: anchor.interface_name,
        interface_version: anchor.interface_version,
        declared_by: anchor.declared_by,
        throws: &[],
        emits: &[],
    }));
    emit(SpecFragmentView::InterfaceAnchor(SpecInterfaceAnchorView {
        ..anchor
    }));
    for method in __GROUNDHOG_IFACE_METHODS_Effects {
        emit(SpecFragmentView::Method(SpecMethodFragmentView {
            interface_name: anchor.interface_name,
            interface_version: anchor.interface_version,
            declared_by: anchor.declared_by,
            method: *method,
        }));
    }
    parse_raw_fragments(&raw).unwrap()
}

#[test]
fn author_declarations_survive_macro_raw_final_and_json_specs() {
    let spec = canonicalize_spec(&fragments()).unwrap();
    let encoded = xdrs::txs::encode(&spec).unwrap();
    assert_eq!(xdrs::txs::decode::<ContractSpec>(&encoded).unwrap(), spec);
    let effects: Vec<_> = spec.interfaces[0]
        .methods
        .iter()
        .map(|m| m.effect)
        .collect();
    assert_eq!(
        effects,
        [
            Some(ContractMethodEffect::EffectPure),
            Some(ContractMethodEffect::EffectReadOnly),
            None,
            Some(ContractMethodEffect::EffectStateChanging),
        ]
    );
    let json: serde_json::Value =
        serde_json::from_str(&render_spec_json(&spec)).unwrap();
    let methods = json["interfaces"][0]["methods"].as_array().unwrap();
    assert_eq!(
        methods
            .iter()
            .map(|m| m.get("effect").map(|effect| effect.as_str().unwrap()))
            .collect::<Vec<_>>(),
        [Some("pure"), Some("readOnly"), None, Some("stateChanging")]
    );
}

#[test]
fn conflicting_effects_and_unsupported_values_are_rejected() {
    let mut fragments = fragments();
    let index = fragments
        .iter()
        .position(|f| {
            matches!(
                f,
                groundhog_contract_spec::SpecFragment::SpecMethod { .. }
            )
        })
        .unwrap();
    let mut conflicting = fragments[index].clone();
    if let groundhog_contract_spec::SpecFragment::SpecMethod {
        method_fragment,
    } = &mut conflicting
    {
        method_fragment.method.effect =
            Some(ContractMethodEffect::EffectStateChanging);
    }
    fragments.push(conflicting);
    assert!(canonicalize_spec(&fragments).is_err());
    fragments.pop();
    if let groundhog_contract_spec::SpecFragment::SpecMethod {
        method_fragment,
    } = &mut fragments[index]
    {
        method_fragment.method.effect = Some(ContractMethodEffect(99));
    }
    assert!(
        canonicalize_spec(&fragments)
            .unwrap_err()
            .to_string()
            .contains("unsupported method effect")
    );
}
