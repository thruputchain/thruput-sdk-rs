use crate::test_environment::{
    DBWrapper, mock_genesis_root_contract, test_network_hash,
};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use thruput_xdr::network_config::{
    ConfigParam, ConfigParamID, NetworkConfig, NetworkConfigV0,
};

/// The configuration a database takes when none is supplied: the stock
/// genesis values of the test executor build this crate is linked against.
///
/// Read from the test executor rather than restated here, so it cannot drift from the
/// binary actually executing the tests. The first call creates (and frees) a
/// throwaway database to ask; the answer is cached for the process.
///
/// This is the base [`NetworkConfigBuilder`] overrides are applied to, and
/// what a [`TestEnvironment`](crate::test_environment::TestEnvironment)
/// built without configuration runs under.
pub fn stock_network_config() -> NetworkConfig {
    static STOCK: OnceLock<NetworkConfig> = OnceLock::new();
    STOCK
        .get_or_init(|| {
            let db = DBWrapper::new(
                None,
                &test_network_hash(),
                &mock_genesis_root_contract(),
                None,
            );
            db.get_network_config()
        })
        .clone()
}

/// The parameters `config` states, in id order.
///
/// The list is a prefix of the id space: a parameter past its end is not
/// stated by this configuration and takes the value the protocol defines
/// for it.
pub fn config_params(config: &NetworkConfig) -> &[ConfigParam] {
    match config {
        NetworkConfig::_0 { v0 } => &v0.params.0,
        other => panic!(
            "unsupported network configuration version: {other:?}\n\
             This build of the SDK only knows NetworkConfig::_0."
        ),
    }
}

/// The value `config` states for `param`, or `None` when it states none.
///
/// ```no_run
/// use thruput_native_exec::network_config;
/// use thruput_xdr::network_config::{ConfigParam, ConfigParamID};
///
/// let config = network_config::stock_network_config();
/// let cap = match network_config::find_config_param(
///     &config,
///     ConfigParamID::kConfigParamMaxSHA256Buffers,
/// ) {
///     Some(ConfigParam::kConfigParamMaxSHA256Buffers {
///         max_sha256_buffers,
///     }) => *max_sha256_buffers,
///     _ => panic!("this chain states no SHA-256 buffer cap"),
/// };
/// assert!(cap > 0);
/// ```
pub fn find_config_param(
    config: &NetworkConfig,
    param: ConfigParamID,
) -> Option<&ConfigParam> {
    config_params(config).iter().find(|p| p.param() == param)
}

/// Builder for a [`NetworkConfig`] stating non-default execution
/// parameters.
///
/// Overrides are applied to a base configuration -- [`stock_network_config`]
/// unless [`with_base`](Self::with_base) says otherwise -- so setting one
/// parameter leaves every other one at the value the chain would have used:
///
/// ```no_run
/// use thruput_native_exec::network_config::NetworkConfigBuilder;
///
/// let config = NetworkConfigBuilder::new()
///     .with_max_sha256_buffers(2)
///     .build();
/// ```
///
/// Most tests never name this type: the same `with_*` setters are on
/// [`TestEnvironmentBuilder`](crate::test_environment_builder::TestEnvironmentBuilder),
/// which builds the configuration and the environment together.
///
/// A setter is only usable against a test executor build that knows its parameter.
/// The stock configuration states what that build's genesis states, and
/// [`build`](Self::build) refuses to leave a gap below a newer parameter,
/// so the ones protocol 0.35 introduced
/// ([`with_max_contract_size_bytes`](Self::with_max_contract_size_bytes)
/// and later) need a test executor built from that protocol or newer.
#[derive(Clone, Debug, Default)]
#[must_use = "a NetworkConfigBuilder does nothing until `build` is called"]
pub struct NetworkConfigBuilder {
    base: Option<NetworkConfig>,
    overrides: BTreeMap<ConfigParamID, ConfigParam>,
}

impl NetworkConfigBuilder {
    /// A builder with no overrides, over the stock configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply the overrides to `base` instead of to
    /// [`stock_network_config`].
    ///
    /// Use this to start from a configuration a test already has in hand
    /// (e.g. one read back off another environment). `base` must itself
    /// state a gapless prefix of the id space, as any configuration the test executor
    /// accepts does.
    pub fn with_base(mut self, base: NetworkConfig) -> Self {
        self.base = Some(base);
        self
    }

    /// State `param`, replacing whatever the base says for it.
    ///
    /// The typed setters below cover the parameters this SDK release knows;
    /// this is the escape hatch for one it does not.
    pub fn with_config_param(mut self, param: ConfigParam) -> Self {
        self.overrides.insert(param.param(), param);
        self
    }

    /// Concurrently open incremental SHA-256 buffers permitted per invocation
    /// frame (the cap `thruput_sdk::Hasher` allocates against).
    pub fn with_max_sha256_buffers(self, max_sha256_buffers: u32) -> Self {
        self.with_config_param(ConfigParam::kConfigParamMaxSHA256Buffers {
            max_sha256_buffers,
        })
    }

    /// Invocation frames one transaction may have open at once.
    ///
    /// The harness opens its frames directly, so one the executor refuses is
    /// an unrecoverable panic rather than the error a contract would observe.
    /// To assert on the cap, have the frame call through the Invoke syscall
    /// (`thruput_sdk::call::try_call_raw_with`), which reports it as
    /// `RawCallError::DepthExceeded`.
    pub fn with_max_cross_invoke_depth(
        self,
        max_cross_invoke_depth: u32,
    ) -> Self {
        self.with_config_param(ConfigParam::kConfigParamMaxCrossInvokeDepth {
            max_cross_invoke_depth,
        })
    }

    /// Contracts one transaction may attach in `contracts_to_deploy`.
    pub fn with_max_new_contracts_per_tx(
        self,
        max_new_contracts_per_tx: u32,
    ) -> Self {
        self.with_config_param(ConfigParam::kConfigParamMaxNewContractsPerTx {
            max_new_contracts_per_tx,
        })
    }

    /// Bytes one bytestring storage item may hold.
    pub fn with_max_bytestring_len(self, max_bytestring_len: u32) -> Self {
        self.with_config_param(ConfigParam::kConfigParamMaxBytestringLen {
            max_bytestring_len,
        })
    }

    /// Bytes one attached contract's code blob (`ContractBlob.data`) may
    /// hold, checked before any attached contract is verified or executed.
    pub fn with_max_contract_size_bytes(
        self,
        max_contract_size_bytes: u32,
    ) -> Self {
        self.with_config_param(ConfigParam::kConfigParamMaxContractSizeBytes {
            max_contract_size_bytes,
        })
    }

    /// Storage savepoints one transaction may have open at once: one per
    /// `TryInvoke` frame still executing, plus the one the root commit
    /// takes.
    pub fn with_max_savepoint_depth(self, max_savepoint_depth: u32) -> Self {
        self.with_config_param(ConfigParam::kConfigParamMaxSavepointDepth {
            max_savepoint_depth,
        })
    }

    /// Bytes one incremental SHA-256 buffer may absorb between resets, the
    /// per-buffer companion of
    /// [`with_max_sha256_buffers`](Self::with_max_sha256_buffers).
    pub fn with_max_sha256_buffer_bytes(
        self,
        max_sha256_buffer_bytes: u32,
    ) -> Self {
        self.with_config_param(ConfigParam::kConfigParamMaxSHA256BufferBytes {
            max_sha256_buffer_bytes,
        })
    }

    /// Levels of `AuthTokenNode` nesting an auth token may carry, counted
    /// from its root.
    pub fn with_max_auth_token_depth(self, max_auth_token_depth: u32) -> Self {
        self.with_config_param(ConfigParam::kConfigParamMaxAuthTokenDepth {
            max_auth_token_depth,
        })
    }

    /// Bytes of code (`code_size` in the artifact header) an RV32 artifact
    /// may carry to be attached as a contract.
    pub fn with_max_rv32_code_bytes(self, max_rv32_code_bytes: u32) -> Self {
        self.with_config_param(ConfigParam::kConfigParamMaxRv32CodeBytes {
            max_rv32_code_bytes,
        })
    }

    /// Bytes of guest data memory (`memory_bytes` in the artifact header)
    /// an RV32 artifact may declare to be attached as a contract.
    pub fn with_max_rv32_memory_bytes(
        self,
        max_rv32_memory_bytes: u32,
    ) -> Self {
        self.with_config_param(ConfigParam::kConfigParamMaxRv32MemoryBytes {
            max_rv32_memory_bytes,
        })
    }

    /// Whether this builder would produce exactly the stock configuration:
    /// no base of its own and nothing overridden.
    pub fn is_stock(&self) -> bool {
        self.base.is_none() && self.overrides.is_empty()
    }

    /// The configuration: the base with every override applied.
    ///
    /// Panics rather than hand the test executor a list it would reject: the parameters
    /// must come out a gapless prefix of the id space (ids 0, 1, 2, ...),
    /// which is what gives one configuration one encoding, and hence one
    /// config hash.
    pub fn build(self) -> NetworkConfig {
        let base = self.base.unwrap_or_else(stock_network_config);
        let mut params = config_params(&base).to_vec();

        // Ascending id order (BTreeMap), so an override that extends the
        // prefix by one lands right after the parameters below it.
        for (param_id, param) in self.overrides {
            let id = param_id.0;
            let index = usize::try_from(id).unwrap_or_else(|_| {
                panic!("network configuration parameter id {id} is negative")
            });
            match index.cmp(&params.len()) {
                Ordering::Less => params[index] = param,
                Ordering::Equal => params.push(param),
                Ordering::Greater => panic!(
                    "cannot state {} (id {id}): the configuration states \
                     {} parameter(s), so this would leave ids {}..{id} \
                     unstated, and the list must be a gapless prefix of \
                     the id space. State the parameters in between too -- \
                     or, if this test executor build does not know id {id} at all, \
                     drop the override.",
                    param.param(),
                    params.len(),
                    params.len(),
                ),
            }
        }

        for (index, param) in params.iter().enumerate() {
            assert_eq!(
                param.param().0,
                index as i32,
                "network configuration parameters must be a gapless prefix \
                 of the id space in id order, but position {index} states \
                 {}. The test executor rejects any other arrangement (fatally, from \
                 inside the FFI).",
                param.param(),
            );
        }

        NetworkConfig::_0 {
            v0: NetworkConfigV0 {
                params: params.try_into().expect(
                    "the parameter list cannot exceed the XDR length bound",
                ),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_of(params: Vec<ConfigParam>) -> NetworkConfig {
        NetworkConfig::_0 {
            v0: NetworkConfigV0 {
                params: params.try_into().unwrap(),
            },
        }
    }

    #[test]
    fn the_stock_config_states_a_gapless_prefix_of_the_id_space() {
        let stock = stock_network_config();
        let params = config_params(&stock);

        assert!(!params.is_empty(), "a chain states its configuration");
        for (index, param) in params.iter().enumerate() {
            assert_eq!(param.param().0, index as i32);
        }
    }

    #[test]
    fn a_builder_with_no_overrides_is_the_stock_config() {
        let builder = NetworkConfigBuilder::new();
        assert!(builder.is_stock());
        assert_eq!(builder.build(), stock_network_config());
    }

    #[test]
    fn setting_a_parameter_leaves_the_others_at_their_stock_values() {
        let stock = stock_network_config();
        let built = NetworkConfigBuilder::new()
            .with_max_sha256_buffers(1)
            .build();

        assert_eq!(
            find_config_param(
                &built,
                ConfigParamID::kConfigParamMaxSHA256Buffers
            ),
            Some(&ConfigParam::kConfigParamMaxSHA256Buffers {
                max_sha256_buffers: 1
            }),
        );
        assert_eq!(&config_params(&built)[1..], &config_params(&stock)[1..],);
    }

    #[test]
    fn the_last_setter_for_a_parameter_wins() {
        let built = NetworkConfigBuilder::new()
            .with_max_sha256_buffers(1)
            .with_max_sha256_buffers(4)
            .build();

        assert_eq!(
            find_config_param(
                &built,
                ConfigParamID::kConfigParamMaxSHA256Buffers
            ),
            Some(&ConfigParam::kConfigParamMaxSHA256Buffers {
                max_sha256_buffers: 4
            }),
        );
    }

    #[test]
    fn overrides_apply_to_an_explicit_base() {
        let base = config_of(vec![
            ConfigParam::kConfigParamMaxSHA256Buffers {
                max_sha256_buffers: 7,
            },
            ConfigParam::kConfigParamMaxCrossInvokeDepth {
                max_cross_invoke_depth: 9,
            },
        ]);
        let built = NetworkConfigBuilder::new()
            .with_base(base)
            .with_max_cross_invoke_depth(3)
            .build();

        assert_eq!(
            config_params(&built),
            [
                ConfigParam::kConfigParamMaxSHA256Buffers {
                    max_sha256_buffers: 7
                },
                ConfigParam::kConfigParamMaxCrossInvokeDepth {
                    max_cross_invoke_depth: 3
                },
            ],
        );
    }

    #[test]
    fn every_typed_setter_states_its_own_parameter() {
        // Applied in id order from an empty base, so each setter extends
        // the prefix by exactly one; the result names every parameter this
        // release knows, in the order the protocol numbers them.
        let built = NetworkConfigBuilder::new()
            .with_base(config_of(vec![]))
            .with_max_sha256_buffers(1)
            .with_max_cross_invoke_depth(2)
            .with_max_new_contracts_per_tx(3)
            .with_max_bytestring_len(4)
            .with_max_contract_size_bytes(5)
            .with_max_savepoint_depth(6)
            .with_max_sha256_buffer_bytes(7)
            .with_max_auth_token_depth(8)
            .with_max_rv32_code_bytes(9)
            .with_max_rv32_memory_bytes(10)
            .build();

        assert_eq!(
            config_params(&built),
            [
                ConfigParam::kConfigParamMaxSHA256Buffers {
                    max_sha256_buffers: 1
                },
                ConfigParam::kConfigParamMaxCrossInvokeDepth {
                    max_cross_invoke_depth: 2
                },
                ConfigParam::kConfigParamMaxNewContractsPerTx {
                    max_new_contracts_per_tx: 3
                },
                ConfigParam::kConfigParamMaxBytestringLen {
                    max_bytestring_len: 4
                },
                ConfigParam::kConfigParamMaxContractSizeBytes {
                    max_contract_size_bytes: 5
                },
                ConfigParam::kConfigParamMaxSavepointDepth {
                    max_savepoint_depth: 6
                },
                ConfigParam::kConfigParamMaxSHA256BufferBytes {
                    max_sha256_buffer_bytes: 7
                },
                ConfigParam::kConfigParamMaxAuthTokenDepth {
                    max_auth_token_depth: 8
                },
                ConfigParam::kConfigParamMaxRv32CodeBytes {
                    max_rv32_code_bytes: 9
                },
                ConfigParam::kConfigParamMaxRv32MemoryBytes {
                    max_rv32_memory_bytes: 10
                },
            ],
        );
    }

    #[test]
    fn an_override_may_extend_the_prefix_by_one() {
        let base =
            config_of(vec![ConfigParam::kConfigParamMaxSHA256Buffers {
                max_sha256_buffers: 7,
            }]);
        let built = NetworkConfigBuilder::new()
            .with_base(base)
            .with_max_cross_invoke_depth(3)
            .build();

        assert_eq!(config_params(&built).len(), 2);
    }

    // The test executor fails a gapped list from inside the FFI, taking the whole test
    // binary with it. Refuse to build one in the first place.
    #[test]
    #[should_panic(expected = "gapless prefix")]
    fn an_override_that_would_leave_a_gap_is_refused() {
        let base = config_of(vec![]);
        let _ = NetworkConfigBuilder::new()
            .with_base(base)
            .with_max_cross_invoke_depth(3)
            .build();
    }

    #[test]
    #[should_panic(expected = "gapless prefix")]
    fn a_base_that_is_not_a_prefix_is_refused() {
        let base =
            config_of(vec![ConfigParam::kConfigParamMaxCrossInvokeDepth {
                max_cross_invoke_depth: 9,
            }]);
        let _ = NetworkConfigBuilder::new().with_base(base).build();
    }

    #[test]
    fn a_builder_with_a_base_is_not_stock() {
        assert!(
            !NetworkConfigBuilder::new()
                .with_base(stock_network_config())
                .is_stock()
        );
    }
}
