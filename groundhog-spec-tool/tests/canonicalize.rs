use groundhog_spec_tool::{
    InterfaceId, RawInterfaceFragment, SpecToolError, canonicalize_interfaces,
};

#[test]
fn merges_duplicate_free_function_interfaces() {
    let raw = vec![
        RawInterfaceFragment::free_fn("erc20_low_level", "ERC20", "0.1.0"),
        RawInterfaceFragment::free_fn("erc20_low_level", "ERC20", "0.1.0"),
    ];

    let interfaces = canonicalize_interfaces(&raw).unwrap();
    assert_eq!(interfaces, vec![InterfaceId::new("ERC20", "0.1.0")],);
}

#[test]
fn rejects_empty_interface_sets() {
    let raw = vec![];
    let err = canonicalize_interfaces(&raw).unwrap_err();
    assert!(matches!(err, SpecToolError::NoInterfaceFragments));
}
