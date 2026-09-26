use groundhog_contract_spec::{
    ContractEventDef, ContractSpec, ContractSpecId, ContractSpecInterface,
    ContractSpecString, ContractStructDef, ContractTypeDef,
};
use groundhog_spec_tool::{SpecWarning, spec_warnings};

fn s(value: &str) -> ContractSpecString {
    ContractSpecString::try_from(value).unwrap()
}

fn id(krate: &str, module_path: &str, declared: &str) -> ContractSpecId {
    ContractSpecId {
        crate_name: s(krate),
        major: s("0.4"),
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

fn spec(
    type_defs: Vec<ContractTypeDef>,
    events: Vec<ContractEventDef>,
    interfaces: Vec<&str>,
) -> ContractSpec {
    ContractSpec {
        interfaces: interfaces
            .into_iter()
            .map(|name| ContractSpecInterface {
                name: s(name),
                vers: s("0.4.0"),
                throws: vec![].try_into().unwrap(),
                methods: vec![].try_into().unwrap(),
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        type_defs: type_defs.try_into().unwrap(),
        events: events.try_into().unwrap(),
    }
}

/// The reserved segment each warning names, in spec order.
fn flagged(spec: &ContractSpec) -> Vec<String> {
    spec_warnings(spec)
        .into_iter()
        .map(|warning| match warning {
            SpecWarning::ReservedTypeScriptName { segment, .. } => segment,
            other => panic!("unexpected warning {other:?}"),
        })
        .collect()
}

#[test]
fn ordinary_names_are_not_flagged() {
    let spec = spec(
        vec![
            struct_def(id("my-token", "my_token", "Config")),
            // Near misses: case differs, or the prefix is not leading.
            struct_def(id("my-token", "my_token::GlobalThis", "Global")),
            struct_def(id("my-token", "my_token", "x__gh_y")),
        ],
        vec![],
        vec!["Token"],
    );

    assert_eq!(flagged(&spec), Vec::<String>::new());
}

#[test]
fn global_this_is_flagged_wherever_it_lands() {
    let spec = spec(
        vec![
            struct_def(id("demo", "demo::globalThis", "Thing")),
            struct_def(id("demo", "demo", "globalThis")),
            struct_def(id("globalThis", "globalThis", "Thing")),
        ],
        vec![ContractEventDef {
            name: id("demo", "demo::inner", "globalThis"),
            signature: [0; 32].into(),
            topics: vec![].try_into().unwrap(),
            fields: vec![].try_into().unwrap(),
        }],
        vec!["globalThis"],
    );

    assert_eq!(flagged(&spec), vec!["globalThis"; 5]);
}

#[test]
fn escape_prefix_is_flagged_wherever_it_lands() {
    let spec = spec(
        vec![
            struct_def(id("demo", "demo::__gh_mod", "Thing")),
            struct_def(id("demo", "demo", "__gh_Thing_0")),
            // A hyphenated crate name lands with `_`.
            struct_def(id("__gh-demo", "__gh_demo", "Thing")),
        ],
        vec![],
        vec!["__gh_client_contract"],
    );

    assert_eq!(
        flagged(&spec),
        vec![
            "__gh_mod",
            "__gh_Thing_0",
            "__gh_demo",
            "__gh_client_contract"
        ]
    );
}

#[test]
fn a_declaration_is_flagged_once() {
    let spec = spec(
        vec![struct_def(id("demo", "demo::globalThis", "__gh_Thing"))],
        vec![],
        vec![],
    );

    let warnings = spec_warnings(&spec);

    assert_eq!(warnings.len(), 1);
    let message = warnings[0].to_string();
    assert!(
        message.contains("demo::globalThis::__gh_Thing"),
        "{message}"
    );
    assert!(message.contains("`groundhog codegen ts`"), "{message}");
}
