use groundhog_xdr::storage_cost::StorageCostModelXdr;

/// The chain's storage price list (`protocol/storage_cost.x`).
///
/// All costs are denominated in storage tokens — the same units account
/// storage balances hold and test-environment grants
/// (`setup_grant_storage_tokens`) are expressed in.
pub struct StorageCostModel {
    cost_model: StorageCostModelXdr,
}

impl StorageCostModel {
    pub fn new(cost_model: StorageCostModelXdr) -> Self {
        Self { cost_model }
    }

    /// A model in which every storage operation costs zero.
    ///
    /// For tests only. Real networks charge nonzero storage costs, so a
    /// contract exercised only under this model has never paid for storage —
    /// keep at least one test on a charging model if storage funding matters
    /// to the contract.
    pub fn no_cost_model() -> Self {
        Self {
            cost_model: StorageCostModelXdr {
                ext: groundhog_xdr::types::ExtensionPoint::_0,
                base_data_item_cost: 0u64,
                bytestring_cost_per_byte: 0u32,
                base_contract_deployment_cost: 0u64,
                contract_deployment_cost_per_byte: 0u64,
                auth_per_key_cost: 0u64,
                rcq_cost_per_entry: 0u64,
                replay_cache_cost_per_entry: 0u64,
                balance_cost: 0u64,
            },
        }
    }

    pub fn serialize(&self) -> xdrs::WriteResult<xdrs::Vec<u8>> {
        xdrs::txs::encode(&self.cost_model)
    }

    fn base_data_item_cost(&self) -> i128 {
        self.cost_model.base_data_item_cost.into()
    }

    fn bytestring_cost_per_byte(&self) -> i128 {
        self.cost_model.bytestring_cost_per_byte.into()
    }

    /// Cost of a bytestring of the given length
    pub fn bytestring_cost(&self, length: usize) -> i128 {
        self.base_data_item_cost()
            + self.bytestring_cost_per_byte() * length as i128
    }

    /// Cost of a non-negative integer
    pub fn nnint_cost(&self) -> i128 {
        self.base_data_item_cost()
    }

    fn base_contract_deployment_cost(&self) -> i128 {
        self.cost_model.base_contract_deployment_cost.into()
    }

    fn contract_deployment_cost_per_byte(&self) -> i128 {
        self.cost_model.contract_deployment_cost_per_byte.into()
    }

    /// Cost of a contract deployment of the given length
    pub fn contract_deployment_cost(&self, length: usize) -> i128 {
        self.base_contract_deployment_cost()
            + self.contract_deployment_cost_per_byte() * length as i128
    }

    /// Cost of a native token balance
    pub fn balance_cost(&self) -> i128 {
        self.cost_model.balance_cost.into()
    }

    /// Cost of each key retained in an auth data item, in storage tokens per
    /// key (the full item costs `base_data_item_cost + auth_per_key_cost *
    /// num_keys`).
    pub fn auth_per_key_cost(&self) -> i128 {
        self.cost_model.auth_per_key_cost.into()
    }

    /// Cost of one retro-causal queue entry, in storage tokens per entry.
    pub fn rcq_cost_per_entry(&self) -> i128 {
        self.cost_model.rcq_cost_per_entry.into()
    }

    /// Cost of one replay-cache entry, in storage tokens per entry.
    pub fn replay_cache_cost_per_entry(&self) -> i128 {
        self.cost_model.replay_cache_cost_per_entry.into()
    }

    pub fn as_xdr(&self) -> StorageCostModelXdr {
        self.cost_model.clone()
    }
}
