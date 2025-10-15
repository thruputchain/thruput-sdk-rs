use crate::implementation::MyClient;
use bottle_test_if::*;
use sdk::*;

#[derive(Debug)]
#[storage]
pub struct BT {
    target_addr: Option<ContractAddress>,
}

#[contract]
impl BottleTest for BT {

    #[init]
    fn setup(&mut self, target_contract_hash: &SHA256Hash) {
        // Deploy an ERC20 contract
        let nonce: SHA256Hash = [7; 32].into();
        let target_addr = deploy_contract(&nonce, target_contract_hash).unwrap();

        self.target_addr = Some(target_addr);
    }

    fn run(&mut self) {
        let target_addr = self.target_addr.as_ref().unwrap_or_else(||
            contract_panic!("playground bottle-test: target_addr unset; please run setup() before run()"));

        let owner = get_current_address().unwrap();

        let mut client = MyClient::new(target_addr);
        client.initialize(&owner).unwrap();

        client.add_frot(10).unwrap();
        let f = client.get_frot().unwrap();
        assert_eq!(f, 10, "Frot is not");

        let buf = [0u8, 1, 2];
        let len_reported = client.hello(&buf[..]);
        assert_eq!(len_reported, Ok(buf.len() as u32));
    }
}
