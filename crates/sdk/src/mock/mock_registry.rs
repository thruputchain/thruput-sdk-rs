use linkme::distributed_slice;
use thruput_native_exec::mock_contract::MockContract;

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct ContractMockFragment {
    pub package_name: &'static str,
    pub self_type_key: Option<&'static str>,
    pub register: fn(&mut MockContract),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MockRegistryError {
    NoFragments {
        package_name: String,
    },
    MultipleImplTypes {
        package_name: String,
        first: &'static str,
        second: &'static str,
    },
}

impl fmt::Display for MockRegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MockRegistryError::NoFragments { package_name } => {
                write!(
                    f,
                    "no mock fragments registered for package `{package_name}`"
                )
            }
            MockRegistryError::MultipleImplTypes {
                package_name,
                first,
                second,
            } => write!(
                f,
                "multiple impl-backed mock fragments registered for package `{package_name}`: `{first}` and `{second}`"
            ),
        }
    }
}

#[doc(hidden)]
#[distributed_slice]
pub static CONTRACT_MOCK_FRAGMENTS: [ContractMockFragment];

fn collect_mock_fragments<'a>(
    package_name: &str,
    fragments: &'a [ContractMockFragment],
) -> Result<Vec<&'a ContractMockFragment>, MockRegistryError> {
    let collected: Vec<_> = fragments
        .iter()
        .filter(|fragment| fragment.package_name == package_name)
        .collect();

    if collected.is_empty() {
        return Err(MockRegistryError::NoFragments {
            package_name: String::from(package_name),
        });
    }

    let mut impl_type: Option<&'static str> = None;
    for fragment in &collected {
        if let Some(key) = fragment.self_type_key {
            match impl_type {
                None => impl_type = Some(key),
                Some(existing) if existing == key => {}
                Some(existing) => {
                    return Err(MockRegistryError::MultipleImplTypes {
                        package_name: String::from(package_name),
                        first: existing,
                        second: key,
                    });
                }
            }
        }
    }

    Ok(collected)
}

pub fn register_mock_package(
    env: &thruput_native_exec::test_environment::TestEnvironmentHandle,
    package_name: &str,
) -> MockContract {
    let fragments =
        collect_mock_fragments(package_name, &CONTRACT_MOCK_FRAGMENTS)
            .unwrap_or_else(|err| panic!("{err}"));

    let mut contract = env.add_contract();
    for fragment in fragments {
        (fragment.register)(&mut contract);
    }
    // Mirror the rv32 dispatcher's undefined-method behavior, so a
    // contract's tests observe what its deployed image would do.
    contract.register_unknown_method(|| {
        crate::exn::library_throw(crate::exn::LibraryExn::UnknownMethod)
    });
    env.setup_create_contract(&contract.as_contract());
    contract
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop(_: &mut MockContract) {}

    #[test]
    fn collect_mock_fragments_filters_by_package() {
        let fragments = [
            ContractMockFragment {
                package_name: "alpha",
                self_type_key: Some("Alpha"),
                register: noop,
            },
            ContractMockFragment {
                package_name: "beta",
                self_type_key: Some("Beta"),
                register: noop,
            },
        ];

        let collected = collect_mock_fragments("alpha", &fragments).unwrap();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].package_name, "alpha");
    }

    #[test]
    fn collect_mock_fragments_rejects_multiple_impl_types() {
        let fragments = [
            ContractMockFragment {
                package_name: "demo",
                self_type_key: Some("Alpha"),
                register: noop,
            },
            ContractMockFragment {
                package_name: "demo",
                self_type_key: Some("Beta"),
                register: noop,
            },
        ];

        let err = collect_mock_fragments("demo", &fragments).unwrap_err();
        assert!(matches!(err, MockRegistryError::MultipleImplTypes { .. }));
    }

    #[test]
    fn collect_mock_fragments_errors_when_no_fragments_exist() {
        let fragments = [ContractMockFragment {
            package_name: "alpha",
            self_type_key: Some("Alpha"),
            register: noop,
        }];

        let err = collect_mock_fragments("beta", &fragments).unwrap_err();
        assert!(matches!(err, MockRegistryError::NoFragments { .. }));
    }

    #[test]
    fn collect_mock_fragments_errors_when_fragment_slice_is_empty() {
        let fragments = [];

        let err = collect_mock_fragments("beta", &fragments).unwrap_err();
        assert!(matches!(err, MockRegistryError::NoFragments { .. }));
    }

    #[test]
    fn collect_mock_fragments_allows_free_function_fragments_with_single_impl_type()
     {
        let fragments = [
            ContractMockFragment {
                package_name: "demo",
                self_type_key: None,
                register: noop,
            },
            ContractMockFragment {
                package_name: "demo",
                self_type_key: Some("Alpha"),
                register: noop,
            },
            ContractMockFragment {
                package_name: "demo",
                self_type_key: None,
                register: noop,
            },
        ];

        let collected = collect_mock_fragments("demo", &fragments).unwrap();
        assert_eq!(collected.len(), 3);
    }
}
