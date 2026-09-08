use crate::network_config::NetworkConfigBuilder;
use crate::test_environment::{
    TestEnvironment, TestEnvironmentHandle, mock_genesis_root_contract,
    test_network_hash,
};
use groundhog_storage_cost::StorageCostModel;
use groundhog_xdr::contract::Contract;
use groundhog_xdr::network_config::{ConfigParam, NetworkConfig};
use groundhog_xdr::types::NetworkHash;

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
/// use groundhog_native_exec::test_environment::TestEnvironment;
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
/// exactly as the plain constructors do.
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

    /// Concurrently open incremental SHA-256 buffers permitted per
    /// invocation frame (the cap `sdk::hasher::Hasher` allocates against).
    pub fn with_max_sha256_buffers(mut self, max_sha256_buffers: u32) -> Self {
        self.network_config = self
            .network_config
            .with_max_sha256_buffers(max_sha256_buffers);
        self
    }

    /// Invocation frames one transaction may have open at once.
    ///
    /// To drive a test past this cap, open the frame with
    /// [`try_run`](crate::frame_builder::FrameBuilder::try_run): a refused
    /// frame comes back as the rule that refused it, leaving the parent
    /// usable. Plain `run` panics on one, so it stays the right choice for
    /// a frame the test expects to open.
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

    /// The longest bytestring a storage item may hold, checked where one is
    /// written rather than read.
    pub fn with_max_bytestring_len(mut self, max_bytestring_len: u32) -> Self {
        self.network_config = self
            .network_config
            .with_max_bytestring_len(max_bytestring_len);
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
    use groundhog_xdr::network_config::ConfigParamID;
    use groundhog_xdr::syscalls::raw_SHA256Update;
    use groundhog_xdr::types::ContractAddress;

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

    // The cap the builder sets is now one a test can assert on: production
    // refuses the frame, try_run reports which rule, and the parent frame
    // stays usable -- where this used to take the test binary with it.
    #[test]
    fn the_cross_invoke_depth_cap_is_assertable() {
        use crate::frame_builder::FrameAction;
        use groundhog_xdr::syscall_error::SyscallErrorCode;

        // One frame open at a time: the root admits, a child does not.
        let env = TestEnvironment::builder()
            .no_cost()
            .with_max_cross_invoke_depth(1)
            .build();

        env.transact_manual(|tx| {
            let mut root = tx
                .call(&ContractAddress::named("depth_root"))
                .run_continuable(|frame| {
                    let refused = frame
                        .child_call(&ContractAddress::named("depth_child"))
                        .try_run(|| {});
                    assert_eq!(
                        refused.unwrap_err(),
                        SyscallErrorCode::kSyscallErrorCodeInvokeDepthExceeded,
                    );
                    FrameAction::Close
                });
            // The refusal opened no frame and owed no finish, so the root
            // closes normally on top of it.
            root.finish().assert_success();
            tx.try_commit_transaction();
        });
    }

    // The same, for the rule a test is likelier to hit by accident.
    #[test]
    fn a_reentrant_frame_is_reported_not_fatal() {
        use crate::frame_builder::FrameAction;
        use groundhog_xdr::syscall_error::SyscallErrorCode;

        let env = TestEnvironment::builder().no_cost().build();
        let addr = ContractAddress::named("reentrant");

        env.transact_manual(|tx| {
            let mut root = tx.call(&addr).run_continuable(|frame| {
                let refused = frame.child_call(&addr).try_run(|| {});
                assert_eq!(
                    refused.unwrap_err(),
                    SyscallErrorCode::kSyscallErrorCodeReentrance,
                );
                FrameAction::Close
            });
            root.finish().assert_success();
            tx.try_commit_transaction();
        });
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

    #[test]
    fn an_explicit_network_config_is_taken_verbatim() {
        let stock = stock_network_config();
        let env = TestEnvironment::builder()
            .with_network_config(stock.clone())
            .build();

        assert_eq!(env.get_network_config(), stock);
    }
}
