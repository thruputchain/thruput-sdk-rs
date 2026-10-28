use crate::network_config::NetworkConfigBuilder;
use crate::test_environment::{
    TestEnvironment, TestEnvironmentHandle, mock_genesis_root_contract,
    test_network_hash,
};
use thruput_storage_cost::StorageCostModel;
use thruput_xdr::contract::Contract;
use thruput_xdr::network_config::{ConfigParam, NetworkConfig};
use thruput_xdr::types::NetworkHash;

/// Builder for a [`TestEnvironment`] that needs something other than the
/// stock chain settings.
///
/// The plain constructors ([`TestEnvironment::new`],
/// [`no_cost`](TestEnvironment::no_cost), and the rest) cover the ordinary
/// case and are unaffected by this type. Reach for the builder when a test
/// wants to drive contract behavior that a network config parameter
/// governs -- what a contract does as it approaches a cap the chain sets,
/// which is otherwise only observable on a chain configured that way:
///
/// ```no_run
/// use thruput_native_exec::test_environment::TestEnvironment;
///
/// let env = TestEnvironment::builder()
///     .no_cost()
///     .with_max_sha256_buffers(2)
///     .build();
/// ```
///
/// Every setter is optional and independent; an unset one keeps what
/// [`TestEnvironment::new`] would have used, and unstated config
/// parameters keep their stock values (see
/// [`stock_network_config`](crate::network_config::stock_network_config)).
/// A builder with no configuration overrides at all creates its database
/// exactly as the plain constructors do. A parameter setter needs a hogd
/// build that knows the parameter; see [`NetworkConfigBuilder`].
#[must_use = "a TestEnvironmentBuilder does nothing until `build` is called"]
pub struct TestEnvironmentBuilder {
    cost_model: Option<StorageCostModel>,
    network_hash: NetworkHash,
    genesis_root_contract: Contract,
    network_config: NetworkConfigBuilder,
}

impl Default for TestEnvironmentBuilder {
    fn default() -> Self {
        Self {
            cost_model: None,
            network_hash: test_network_hash(),
            genesis_root_contract: mock_genesis_root_contract(),
            network_config: NetworkConfigBuilder::new(),
        }
    }
}

impl TestEnvironmentBuilder {
    /// A builder that would produce exactly `TestEnvironment::new(None)`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Charge storage against `cost_model` instead of the chain default.
    pub fn with_cost_model(mut self, cost_model: StorageCostModel) -> Self {
        self.cost_model = Some(cost_model);
        self
    }

    /// Make every storage operation free, as
    /// [`TestEnvironment::no_cost`] does.
    pub fn no_cost(self) -> Self {
        self.with_cost_model(StorageCostModel::no_cost_model())
    }

    /// Run on `network_hash` instead of [`test_network_hash`].
    ///
    /// The network hash is part of every transaction signature preimage, so
    /// pin it when a test's signatures were produced for a specific
    /// network.
    pub fn with_network_hash(mut self, network_hash: NetworkHash) -> Self {
        self.network_hash = network_hash;
        self
    }

    /// Install `contract` as the genesis root deployment instead of
    /// [`mock_genesis_root_contract`].
    pub fn with_genesis_root_contract(mut self, contract: Contract) -> Self {
        self.genesis_root_contract = contract;
        self
    }

    /// Apply the parameter setters below to `config` rather than to the
    /// stock configuration.
    ///
    /// This is the whole-configuration escape hatch: pass one read back off
    /// another environment, or one built by hand. Anything the setters do
    /// not restate is taken from it verbatim.
    pub fn with_network_config(mut self, config: NetworkConfig) -> Self {
        self.network_config = self.network_config.with_base(config);
        self
    }

    /// State one config parameter the typed setters below do not cover.
    pub fn with_config_param(mut self, param: ConfigParam) -> Self {
        self.network_config = self.network_config.with_config_param(param);
        self
    }

    /// Concurrently open incremental SHA-256 buffers permitted per invocation
    /// frame (the cap `thruput_sdk::Hasher` allocates against).
    pub fn with_max_sha256_buffers(mut self, max_sha256_buffers: u32) -> Self {
        self.network_config = self
            .network_config
            .with_max_sha256_buffers(max_sha256_buffers);
        self
    }

    /// Invocation frames one transaction may have open at once.
    ///
    /// The harness opens its frames directly, so one the executor refuses is
    /// an unrecoverable panic rather than the error a contract would observe.
    /// To assert on the cap, have the frame call through the Invoke syscall
    /// (`thruput_sdk::call::try_call_raw_with`), which reports it as
    /// `RawCallError::DepthExceeded`.
    pub fn with_max_cross_invoke_depth(
        mut self,
        max_cross_invoke_depth: u32,
    ) -> Self {
        self.network_config = self
            .network_config
            .with_max_cross_invoke_depth(max_cross_invoke_depth);
        self
    }

    /// Contracts one transaction may attach in `contracts_to_deploy`.
    pub fn with_max_new_contracts_per_tx(
        mut self,
        max_new_contracts_per_tx: u32,
    ) -> Self {
        self.network_config = self
            .network_config
            .with_max_new_contracts_per_tx(max_new_contracts_per_tx);
        self
    }

    /// Bytes one bytestring storage item may hold.
    pub fn with_max_bytestring_len(mut self, max_bytestring_len: u32) -> Self {
        self.network_config = self
            .network_config
            .with_max_bytestring_len(max_bytestring_len);
        self
    }

    /// Bytes one attached contract's code blob (`ContractBlob.data`) may
    /// hold, checked before any attached contract is verified or executed.
    pub fn with_max_contract_size_bytes(
        mut self,
        max_contract_size_bytes: u32,
    ) -> Self {
        self.network_config = self
            .network_config
            .with_max_contract_size_bytes(max_contract_size_bytes);
        self
    }

    /// Storage savepoints one transaction may have open at once: one per
    /// `TryInvoke` frame still executing, plus the one the root commit
    /// takes.
    pub fn with_max_savepoint_depth(
        mut self,
        max_savepoint_depth: u32,
    ) -> Self {
        self.network_config = self
            .network_config
            .with_max_savepoint_depth(max_savepoint_depth);
        self
    }

    /// Bytes one incremental SHA-256 buffer may absorb between resets, the
    /// per-buffer companion of
    /// [`with_max_sha256_buffers`](Self::with_max_sha256_buffers).
    pub fn with_max_sha256_buffer_bytes(
        mut self,
        max_sha256_buffer_bytes: u32,
    ) -> Self {
        self.network_config = self
            .network_config
            .with_max_sha256_buffer_bytes(max_sha256_buffer_bytes);
        self
    }

    /// Levels of `AuthTokenNode` nesting an auth token may carry, counted
    /// from its root.
    pub fn with_max_auth_token_depth(
        mut self,
        max_auth_token_depth: u32,
    ) -> Self {
        self.network_config = self
            .network_config
            .with_max_auth_token_depth(max_auth_token_depth);
        self
    }

    /// Bytes of code (`code_size` in the artifact header) an RV32 artifact
    /// may carry to be attached as a contract.
    pub fn with_max_rv32_code_bytes(
        mut self,
        max_rv32_code_bytes: u32,
    ) -> Self {
        self.network_config = self
            .network_config
            .with_max_rv32_code_bytes(max_rv32_code_bytes);
        self
    }

    /// Bytes of guest data memory (`memory_bytes` in the artifact header)
    /// an RV32 artifact may declare to be attached as a contract.
    pub fn with_max_rv32_memory_bytes(
        mut self,
        max_rv32_memory_bytes: u32,
    ) -> Self {
        self.network_config = self
            .network_config
            .with_max_rv32_memory_bytes(max_rv32_memory_bytes);
        self
    }

    /// Create the configured environment.
    pub fn build(self) -> TestEnvironmentHandle {
        // Nothing to configure: create the database the way the plain
        // constructors do, rather than restating hogd's own defaults back
        // at it (which would also cost a probe for them).
        let network_config = if self.network_config.is_stock() {
            None
        } else {
            Some(self.network_config.build())
        };

        TestEnvironment::from_parts(
            self.cost_model.as_ref(),
            &self.network_hash,
            &self.genesis_root_contract,
            network_config.as_ref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network_config::{
        config_params, find_config_param, stock_network_config,
    };
    use crate::test_helpers::Named;
    use thruput_xdr::network_config::ConfigParamID;
    use thruput_xdr::syscalls::{raw_BytestringSet, raw_SHA256Update};
    use thruput_xdr::types::{ContractAddress, StorageKeyContractData};

    fn max_sha256_buffers(config: &NetworkConfig) -> u32 {
        match find_config_param(
            config,
            ConfigParamID::kConfigParamMaxSHA256Buffers,
        ) {
            Some(ConfigParam::kConfigParamMaxSHA256Buffers {
                max_sha256_buffers,
            }) => *max_sha256_buffers,
            other => panic!("no SHA-256 buffer cap in the config: {other:?}"),
        }
    }

    #[test]
    fn a_builder_with_no_overrides_matches_the_plain_constructor() {
        let built = TestEnvironment::builder().build();
        let plain = TestEnvironment::new(None);

        assert_eq!(built.get_network_config(), plain.get_network_config());
        assert_eq!(built.get_network_config(), stock_network_config());
    }

    #[test]
    fn no_cost_builds_the_free_cost_model() {
        assert_eq!(
            TestEnvironment::builder()
                .no_cost()
                .build()
                .get_current_cost_model()
                .bytestring_cost(32),
            0,
        );
        assert!(
            TestEnvironment::builder()
                .build()
                .get_current_cost_model()
                .bytestring_cost(32)
                > 0,
            "the chain default charges for storage",
        );
    }

    #[test]
    fn an_overridden_parameter_is_the_only_one_that_changes() {
        let stock = stock_network_config();
        let env = TestEnvironment::builder()
            .with_max_sha256_buffers(max_sha256_buffers(&stock) + 1)
            .build();
        let config = env.get_network_config();

        assert_eq!(
            max_sha256_buffers(&config),
            max_sha256_buffers(&stock) + 1,
        );
        assert_eq!(config_params(&config).len(), config_params(&stock).len());
        assert_eq!(
            &config_params(&config)[1..],
            &config_params(&stock)[1..],
            "the parameters left alone keep their stock values",
        );
    }

    // The point of the whole builder: a parameter the chain sets, set
    // differently, actually governs execution. Every live incremental
    // SHA-256 stream costs one buffer, so the cap is what the VM refuses
    // the next one against.
    #[test]
    fn the_sha256_buffer_cap_the_builder_sets_is_the_one_the_vm_enforces() {
        let env = TestEnvironment::builder()
            .no_cost()
            .with_max_sha256_buffers(2)
            .build();

        env.transact(|tx| {
            tx.call(&ContractAddress::named("sha256_cap")).run(|| {
                assert!(raw_SHA256Update(0, &[], true).is_ok());
                assert!(raw_SHA256Update(1, &[], true).is_ok());
                assert!(
                    raw_SHA256Update(2, &[], true).is_err(),
                    "a third live stream must exceed the cap of 2",
                );
            })
        })
        .assert_success();
    }

    #[test]
    fn the_stock_cap_leaves_room_for_more_streams_than_that() {
        let env = TestEnvironment::builder().no_cost().build();
        let cap = max_sha256_buffers(&env.get_network_config());
        assert!(cap > 2, "this test needs a stock cap above the one above");

        env.transact(|tx| {
            tx.call(&ContractAddress::named("sha256_stock_cap")).run(
                move || {
                    for index in 0..cap {
                        assert!(raw_SHA256Update(index, &[], true).is_ok());
                    }
                    assert!(raw_SHA256Update(cap, &[], true).is_err());
                },
            )
        })
        .assert_success();
    }

    #[test]
    fn the_max_bytestring_len_parameter_is_settable() {
        let env = TestEnvironment::builder()
            .no_cost()
            .with_max_bytestring_len(64)
            .build();

        assert_eq!(
            find_config_param(
                &env.get_network_config(),
                ConfigParamID::kConfigParamMaxBytestringLen,
            ),
            Some(&ConfigParam::kConfigParamMaxBytestringLen {
                max_bytestring_len: 64
            }),
        );
    }

    // As with the SHA-256 cap: the parameter the builder sets is the one
    // the write is measured against. It is checked where the bytestring is
    // written, so that is where the boundary shows.
    #[test]
    fn the_bytestring_cap_the_builder_sets_is_the_one_the_vm_enforces() {
        let env = TestEnvironment::builder()
            .no_cost()
            .with_max_bytestring_len(64)
            .build();
        let key = StorageKeyContractData::new([0x77; 32]);

        env.transact(|tx| {
            tx.call(&ContractAddress::named("bytestring_cap"))
                .run(move || {
                    assert!(
                        raw_BytestringSet(&key, &[0u8; 64]).is_ok(),
                        "a bytestring at the cap must still be admitted",
                    );
                    assert!(
                        raw_BytestringSet(&key, &[0u8; 65])
                            .unwrap_err()
                            .is_invalid_input(),
                        "one byte over must not be",
                    );
                })
        })
        .assert_success();
    }

    #[test]
    fn an_explicit_network_config_is_taken_verbatim() {
        let stock = stock_network_config();
        let env = TestEnvironment::builder()
            .with_network_config(stock.clone())
            .build();

        assert_eq!(env.get_network_config(), stock);
    }
}
