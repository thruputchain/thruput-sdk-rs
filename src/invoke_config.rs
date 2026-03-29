use crate::transaction::InvokeFlagSet;

#[derive(Clone, Copy)]
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
}

impl Default for InvokeConfig {
    fn default() -> Self {
        Self::new()
    }
}
