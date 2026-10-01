fn main() {
    println!("cargo:rerun-if-env-changed=GROUNDHOG_INTEGRATION_TEST");
    println!("cargo:rustc-check-cfg=cfg(groundhog_integration_runner)");

    if std::env::var("GROUNDHOG_INTEGRATION_TEST").as_deref() == Ok("1") {
        println!("cargo:rustc-cfg=groundhog_integration_runner");
    }
}
