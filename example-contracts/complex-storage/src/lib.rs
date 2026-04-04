#![cfg_attr(not(feature = "dev"), no_std)]

sdk::declare_mock_package!();

pub use sdk::storage::*;
pub use sdk::*;

#[derive(Storage)]
pub struct InnerStorage {
    value: Value<u64>,
    balances: Map<ContractAddress, Value<u64>>,
}

#[derive(Debug, Storage)]
pub struct ComplexStorage {
    inners: Vector<Node<InnerStorage>>,
    value: Value<u64>,
}

#[contract]
impl ComplexStorage {
    pub fn push_inner(&mut self) {
        let addr: ContractAddress = [0; 32].into();
        self.inners.push_with(|inner| {
            inner.value.set(&10);
            inner.balances.at_mut(&addr).set(&100);
        });
    }

    pub fn get_inner_balance(&self, addr: ContractAddress) -> u64 {
        assert!(!self.inners.is_empty(), "No inner storage");
        let last = self.inners.len() - 1;
        let inner = self.inners.at(last);
        inner.balances.at(&addr).get()
    }

    pub fn set_unconditional(&mut self, val: u64) {
        self.value.set(&val);
    }

    pub fn set_value(&mut self, val: u64) {
        *self.value.load_mut() = val;
    }

    pub fn get_value(&self) -> u64 {
        self.value.get()
    }
}
