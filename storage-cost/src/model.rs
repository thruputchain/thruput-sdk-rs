use groundhog_xdr::storage_cost::StorageCostModelXdr;

pub struct StorageCostModel {
    cost_model: StorageCostModelXdr,
}

impl StorageCostModel {
    pub fn new(cost_model: StorageCostModelXdr) -> Self {
        Self { cost_model }
    }

    pub fn no_cost_model() -> Self {
        Self {
            cost_model: StorageCostModelXdr {
                ext: groundhog_xdr::types::ExtensionPoint::_0,
                base_data_item_cost: 0i128.into(),
                bytestring_cost_per_byte: 0,
                base_contract_deployment_cost: 0i128.into(),
                contract_deployment_cost_per_byte: 0i128.into(),
                auth_per_key_cost: 0i128.into(),
                replay_cache_cost_per_entry: 0i128.into(),
                balance_cost: 0i128.into(),
            },
        }
    }

    pub fn serialize(&self) -> xdrs::WriteResult<xdrs::Vec<u8>> {
        xdrs::txs::encode(&self.cost_model)
    }

    fn base_data_item_cost(&self) -> i128 {
        self.cost_model.base_data_item_cost.clone().into()
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
        self.cost_model.base_contract_deployment_cost.clone().into()
    }

    fn contract_deployment_cost_per_byte(&self) -> i128 {
        self.cost_model
            .contract_deployment_cost_per_byte
            .clone()
            .into()
    }

    /// Cost of a contract deployment of the given length
    pub fn contract_deployment_cost(&self, length: usize) -> i128 {
        self.base_contract_deployment_cost()
            + self.contract_deployment_cost_per_byte() * length as i128
    }

    /// Cost of a native token balance
    pub fn balance_cost(&self) -> i128 {
        self.cost_model.balance_cost.clone().into()
    }

    pub fn as_xdr(&self) -> StorageCostModelXdr {
        self.cost_model.clone()
    }
}
