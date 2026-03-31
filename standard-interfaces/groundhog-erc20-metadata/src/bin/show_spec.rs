fn main() {
    let spec = groundhog_erc20_metadata_if::ERC20Metadata_spec();

    println!("\n{spec}");

    println!("\n{}", spec.print_json_summary());
}
