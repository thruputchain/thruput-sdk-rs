use groundhog_spec_tool::{
    canonicalize_interfaces, InterfaceId, RawInterfaceFragment,
};

#[test]
fn merges_duplicate_free_function_interfaces() {
    let raw = vec![
        RawInterfaceFragment::free_fn(
            "erc20_low_level",
            "ERC20",
            "0.1.0",
        ),
        RawInterfaceFragment::free_fn(
            "erc20_low_level",
            "ERC20",
            "0.1.0",
        ),
    ];

    let interfaces = canonicalize_interfaces(&raw).unwrap();
    assert_eq!(
        interfaces,
        vec![InterfaceId::new("ERC20", "0.1.0")],
    );
}

#[test]
fn rejects_multiple_impl_backed_self_types() {
    let raw = vec![
        RawInterfaceFragment::impl_backed(
            "demo",
            "Alpha",
            "Alpha",
            "0.3.0",
        ),
        RawInterfaceFragment::impl_backed(
            "demo",
            "Beta",
            "Beta",
            "0.3.0",
        ),
    ];

    let err = canonicalize_interfaces(&raw).unwrap_err();
    assert!(err.to_string().contains("multiple impl-backed self types"));
}

#[test]
fn rejects_empty_interface_sets() {
    let raw = vec![];
    let err = canonicalize_interfaces(&raw).unwrap_err();
    assert!(err.to_string().contains("no interface fragments"));
}
