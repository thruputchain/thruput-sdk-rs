use groundhog_contract_spec::{
    ContractEnumDef, ContractEnumVariant, ContractEventDef, ContractSpec,
    ContractSpecId, ContractSpecInterface, ContractSpecString,
    ContractStructDef, ContractTypeDef,
};
use groundhog_spec_tool::typescript::binding_warnings;

fn s(value: &str) -> ContractSpecString {
    ContractSpecString::try_from(value).unwrap()
}

/// An id for `declared` in module `module_path` of crate `krate`, major 1.
fn id(krate: &str, module_path: &str, declared: &str) -> ContractSpecId {
    ContractSpecId {
        crate_name: s(krate),
        major: s("1"),
        module_path: s(module_path),
        declared: s(declared),
    }
}

fn struct_def(name: ContractSpecId) -> ContractTypeDef {
    ContractTypeDef::TypeDefStruct {
        struct_def: ContractStructDef {
            name,
            fields: vec![].try_into().unwrap(),
        },
    }
}

fn enum_def(name: ContractSpecId, variants: &[&str]) -> ContractTypeDef {
    let variants: Vec<ContractEnumVariant> = variants
        .iter()
        .zip(1..)
        .map(|(variant, value)| ContractEnumVariant {
            name: s(variant),
            value,
        })
        .collect();
    ContractTypeDef::TypeDefEnum {
        enum_def: ContractEnumDef {
            name,
            variants: variants.try_into().unwrap(),
        },
    }
}

fn event(name: ContractSpecId) -> ContractEventDef {
    ContractEventDef {
        name,
        signature: [0xAB; 32].into(),
        topics: vec![].try_into().unwrap(),
        fields: vec![].try_into().unwrap(),
    }
}

fn spec(
    interface: &str,
    type_defs: Vec<ContractTypeDef>,
    events: Vec<ContractEventDef>,
) -> ContractSpec {
    ContractSpec {
        interfaces: vec![ContractSpecInterface {
            name: s(interface),
            vers: s("1.0.0"),
            throws: vec![].try_into().unwrap(),
            methods: vec![].try_into().unwrap(),
        }]
        .try_into()
        .unwrap(),
        type_defs: type_defs.try_into().unwrap(),
        events: events.try_into().unwrap(),
    }
}

#[test]
fn names_the_codegen_can_bind_raise_nothing() {
    // Overlapping names the codegen still binds.
    let spec = spec(
        "Demo",
        vec![
            enum_def(id("dup-a", "dup_a", "Error"), &["Denied"]),
            enum_def(id("dup-b", "dup_b", "Error"), &["Frozen"]),
            struct_def(id("demo", "demo::ledger", "Config")),
            struct_def(id("demo", "demo::vault", "Config")),
            struct_def(id("demo", "demo", "dup_a")),
            enum_def(id("demo", "demo", "status"), &["Active"]),
            enum_def(id("demo", "demo::status", "Mode"), &["On"]),
            struct_def(id("demo", "demo::Transfer", "Detail")),
        ],
        vec![event(id("demo", "demo", "Transfer"))],
    );
    assert_eq!(binding_warnings(&spec), Vec::<String>::new());
}

#[test]
fn an_enum_variant_named_like_its_modules_value() {
    // `#[contract_type(name = "status")]` on an enum beside `mod status`,
    // which declares an enum named like one of its variants.
    let spec = spec(
        "Demo",
        vec![
            enum_def(id("demo", "demo", "status"), &["Active", "Mode"]),
            enum_def(id("demo", "demo::status", "Mode"), &["On", "Off"]),
        ],
        vec![],
    );
    assert_eq!(
        binding_warnings(&spec),
        vec![
            "enum demo::status (demo@1) has a variant \"Mode\", and enum \
             demo::status::Mode (demo@1) also needs \"demo.v1.status.Mode\" \
             in the generated TypeScript, where an enum's variants share one \
             object with the values of the module named after it. Rename \
             either."
        ]
    );
}

#[test]
fn an_event_beside_a_module_holding_a_value() {
    // However deep the value sits, the event's descriptor has to hold the
    // first segment below it.
    for (module, key) in [
        ("demo::Transfer", "Kind"),
        ("demo::Transfer::deeper", "deeper"),
    ] {
        let spec = spec(
            "Demo",
            vec![enum_def(id("demo", module, "Kind"), &["Mint"])],
            vec![event(id("demo", "demo", "Transfer"))],
        );
        assert_eq!(
            binding_warnings(&spec),
            vec![format!(
                "event demo::Transfer (demo@1) and enum {module}::Kind \
                 (demo@1) both need a value under \"demo.v1.Transfer\" in \
                 the generated TypeScript: the event's descriptor is there, \
                 and it cannot also hold \"{key}\". Rename either, or move \
                 one out of that module."
            )]
        );
    }
}

#[test]
fn a_reserved_word_in_a_path() {
    // `default` is a valid Rust module name and a reserved word in
    // TypeScript.
    let spec = spec(
        "Demo",
        vec![struct_def(id("demo", "demo::default", "Config"))],
        vec![],
    );
    assert_eq!(
        binding_warnings(&spec),
        vec![
            "struct demo::default::Config (demo@1) would be \
             \"demo.v1.default.Config\" in the generated TypeScript, where \
             \"default\" is not a valid identifier. Rename it."
        ]
    );
}

#[test]
fn global_this_anywhere_in_a_path() {
    for module in ["demo::globalThis", "demo::globalThis::inner"] {
        let spec = spec(
            "Demo",
            vec![struct_def(id("demo", module, "Config"))],
            vec![],
        );
        assert_eq!(
            binding_warnings(&spec),
            vec![format!(
                "struct {module}::Config (demo@1) has \"globalThis\" in its \
                 generated TypeScript path, where it would shadow the global \
                 object the bindings reach built-in types through. Rename \
                 the module or type."
            )]
        );
    }
}

#[test]
fn the_generator_prefix_anywhere_a_name_lands() {
    let spec = spec(
        "Demo",
        vec![struct_def(id("demo", "demo::__gh_util", "Config"))],
        vec![event(id("demo", "demo", "__gh_Ping"))],
    );
    assert_eq!(
        binding_warnings(&spec),
        vec![
            "generated TypeScript name \"__gh_util\" for struct \
             demo::__gh_util::Config (demo@1) starts with \"__gh_\", which \
             the generator reserves for the names it declares itself. \
             Rename it without that prefix.",
            "generated TypeScript name \"__gh_Ping\" for event \
             demo::__gh_Ping (demo@1) starts with \"__gh_\", which the \
             generator reserves for the names it declares itself. Rename it \
             without that prefix.",
        ]
    );
}

#[test]
fn interface_names_the_codegen_declares_at_top_level() {
    for (name, warning) in [
        (
            "__gh_Token",
            "interface \"__gh_Token\" starts with \"__gh_\", which the \
             generator reserves for the names it declares itself. Rename it \
             without that prefix.",
        ),
        (
            "globalThis",
            "interface \"globalThis\" would shadow the global object the \
             bindings reach built-in types through. Rename it.",
        ),
        (
            "string",
            "interface \"string\" is not a valid TypeScript identifier. \
             Rename it.",
        ),
    ] {
        assert_eq!(binding_warnings(&spec(name, vec![], vec![])), [warning]);
    }
}

#[test]
fn a_module_named_after_a_crate_whose_types_the_bindings_use() {
    // Inside `demo.v1.other`, a reference written `other.v1.Thing` would
    // resolve to the module.
    let spec = spec(
        "Demo",
        vec![
            struct_def(id("other", "other", "Thing")),
            struct_def(id("demo", "demo::other", "Holder")),
        ],
        vec![],
    );
    assert_eq!(
        binding_warnings(&spec),
        vec![
            "struct demo::other::Holder (demo@1) has \"other\" in its \
             generated TypeScript path, which is also the name of a crate \
             whose types the bindings write as \"other.…\". Inside that \
             namespace the name would resolve to the module instead. Rename \
             the module."
        ]
    );
}

#[test]
fn a_type_and_an_event_that_take_one_name() {
    // `#[contract_type(name = "Transfer")]` on an enum beside the event, and
    // a struct named like the event's filter type.
    for (type_def, warning) in [
        (
            enum_def(id("demo", "demo", "Transfer"), &["Mint"]),
            "generated TypeScript value name \"demo.v1.Transfer\" collides \
             between enum demo::Transfer (demo@1) and event demo::Transfer \
             (demo@1)",
        ),
        (
            struct_def(id("demo", "demo", "TransferFilter")),
            "generated TypeScript value name \"demo.v1.TransferFilter\" \
             collides between struct demo::TransferFilter (demo@1) and the \
             filter type of event demo::Transfer (demo@1)",
        ),
    ] {
        let spec = spec(
            "Demo",
            vec![type_def],
            vec![event(id("demo", "demo", "Transfer"))],
        );
        assert_eq!(binding_warnings(&spec), [warning]);
    }
}

#[test]
fn an_event_named_like_another_events_filter_type() {
    let spec = spec(
        "Demo",
        vec![],
        vec![
            event(id("demo", "demo", "Transfer")),
            event(id("demo", "demo", "TransferFilter")),
        ],
    );
    assert_eq!(
        binding_warnings(&spec),
        [
            "generated TypeScript value name \"demo.v1.TransferFilter\" \
             collides between the filter type of event demo::Transfer \
             (demo@1) and event demo::TransferFilter (demo@1)"
        ]
    );
}
