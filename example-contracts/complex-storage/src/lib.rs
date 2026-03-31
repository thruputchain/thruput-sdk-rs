#![cfg_attr(not(feature = "dev"), no_std)]

pub use sdk::*;
pub use sdk::storage::*;

#[derive(Storage)]
pub struct InnerStorage {
    value: Value<u64>,
    balances: Map<ContractAddress, Value<u64>>,
}

#[derive(Debug, Storage)]
pub struct ComplexStorage {
    inners: Vector<Node<InnerStorage>>,
    _foo: Value<u64>,
}


#[contract]
impl ComplexStorage {

    pub fn push_inner(&mut self) {
        self.inners.push();

        let addr : ContractAddress = [0; 32].into();

        let mut inner = self.inners.at_mut(self.inners.len() - 1);
        inner.value.set(&10);
        inner.balances.at_mut(&addr).set(&100);
    }

    pub fn get_inner_balance(&self, addr: ContractAddress) -> u64 {
        assert!(!self.inners.is_empty(), "No inner storage");
        let inner = self.inners.at(self.inners.len() - 1);
        inner.balances.at(&addr).get()
    }
}