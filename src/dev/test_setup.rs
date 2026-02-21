use crate::{TestEnvironmentHandle, Contract, MockContract};

pub trait TestRegister {
    /// Register this contract in the test environment
    fn register(env: &TestEnvironmentHandle) -> Contract {
        let mut contract = env.add_contract();

        Self::register_mock(&mut contract);

        contract.as_contract()
    }

    /// Configure a mock contract with implemented methods
    fn register_mock(_contract: &mut MockContract) {
        unimplemented!()
    }
}
