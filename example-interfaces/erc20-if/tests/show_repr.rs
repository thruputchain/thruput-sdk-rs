use erc20_if;

#[test]
fn show_repr() {
    let contract = erc20_if::ERC20_repr();

    println!("\n{contract:?}");
}
