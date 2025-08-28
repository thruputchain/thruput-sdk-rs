fn main() {
    let spec = groundhog_erc1271_if::ERC1271_spec();

    println!("\n{spec}");

    println!("\n{}", spec.print_json_summary());
}
