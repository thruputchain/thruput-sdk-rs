use groundhog_xdr::transaction::InvokeFlagSet;

pub struct InvokeConfig {
    pub gas_limit: u64,
    pub flags: InvokeFlagSet,
}

impl InvokeConfig {
    pub fn new() -> Self {
        Self {
            gas_limit: u64::MAX,
            flags: InvokeFlagSet::default(),
        }
    }

    pub fn limit_gas(&mut self, gas_limit: u64) -> &mut Self {
        self.gas_limit = gas_limit;
        self
    }
}

impl Default for InvokeConfig {
    fn default() -> Self {
        Self::new()
    }
}
