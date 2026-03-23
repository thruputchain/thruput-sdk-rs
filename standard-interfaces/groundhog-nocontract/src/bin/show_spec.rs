fn main() {
    let spec = groundhog_nocontract_if::NoContractWallet_spec();

    println!("\n{spec}");

    println!("\n{}", spec.print_json_summary());
}
