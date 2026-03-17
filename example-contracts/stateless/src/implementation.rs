use sdk::*;

pub struct Stateless;

#[contract(client = StatelessClient)]
impl Stateless {
    pub fn run() {
        Self::private();
    }

    #[allow(dead_code)]
    fn private() {}
}
