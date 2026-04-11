use sdk::contract_spec::ContractSpec;

#[test]
fn alloc_enabled_sdk_reexports_owning_contract_spec_types() {
    let spec = ContractSpec {
        interfaces: vec![],
        type_defs: vec![],
    };

    assert!(spec.interfaces.is_empty());
    assert!(spec.type_defs.is_empty());
}
