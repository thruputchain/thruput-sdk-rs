// Only to be compiled for native_exec

use sdk::*;
use groundhog_erc20_if::ERC20Client;

#[derive(Default)]
pub struct Bencher;

#[contract(client = BencherClient, storage = "false")]
impl Bencher {
    // Invoke some ERC20 methods and report gas usage
    pub fn bench_erc20(&self, erc20_addr: &ContractAddress) {
        let gas_start = stats::gas_remaining();
        {
            let owner = get_current_address().unwrap();

            let mut erc20 = ERC20Client::new(erc20_addr);

            erc20.initialize(&owner).unwrap_or_else(|e|
                contract_panic!("Failed calling ERC20::initialize: {e}"));

            let _ = erc20.get_balance().unwrap();
            erc20.mint(&owner.clone().into(), 100).unwrap();
            assert_eq!(erc20.get_balance().unwrap(), 100);
        }
        let gas_end = stats::gas_remaining();

        if gas_end > gas_start {
            panic!("consumed negative {} gas", gas_end - gas_start);
        }
        let gas_consumed = gas_start - gas_end;
        println!("Gas start ({}) - end ({}) = consumed ({})", gas_start, gas_end, gas_consumed);
    }
}
