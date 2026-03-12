use sdk::*;

#[derive(Default)]
pub struct Stateless;

#[contract(client = StatelessClient, storage = "false")]
impl Stateless {
    pub fn run() {}

    #[allow(dead_code)]
    fn private() {}
}
