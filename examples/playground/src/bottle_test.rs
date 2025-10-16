use crate::implementation::MyClient;
use bottle_test_if::*;
use sdk::*;

#[derive(Copy, Clone, Default)]
pub struct BT;

#[contract(storage = "false")]
impl BottleTest for BT {
    fn run(&mut self, target_contract_hash: &SHA256Hash) {
        let nonce = SHA256Hash::from([7; 32]);
        let deployed_addr =
            deploy_contract(&nonce, target_contract_hash).unwrap();

        let owner = get_current_address().unwrap();

        let mut client = MyClient::new(&deployed_addr);
        client.initialize(&owner).unwrap();

        client.add_frot(10).unwrap();
        let f = client.get_frot().unwrap();
        assert_eq!(f, 10, "Frot is not");

        let buf = [0u8, 1, 2];
        let len_reported = client.hello(&buf[..]);
        assert_eq!(len_reported, Ok(buf.len() as u32));
    }
}
