use groundhog_contract_util::{
    contract_entrypoint_id, entrypoint_export_name, NormalizedMethodSignature,
    semver::major_version_from_string,
};
use serde_json::{json, Value};

struct EntrypointCase {
    interface: Option<&'static str>,
    version_str: &'static str,
    method: &'static str,
}

fn cases() -> Vec<EntrypointCase> {
    vec![
        // Basic single-word methods with standard versions
        EntrypointCase { interface: Some("Token"), version_str: "1.0.0", method: "transfer" },
        EntrypointCase { interface: Some("Token"), version_str: "1.0.0", method: "balance" },
        EntrypointCase { interface: Some("Token"), version_str: "1.0.0", method: "approve" },
        // Multi-word method names
        EntrypointCase { interface: Some("Fibonacci"), version_str: "0.1.0", method: "slow_fibonacci" },
        EntrypointCase { interface: Some("Fibonacci"), version_str: "0.1.0", method: "get_balance" },
        // Different interface names
        EntrypointCase { interface: Some("SimpleWallet"), version_str: "1.0.0", method: "send" },
        EntrypointCase { interface: Some("PaymentChannel"), version_str: "2.5.0", method: "close" },
        // Null interface (tests _unnamed_interface fallback)
        EntrypointCase { interface: None, version_str: "1.0.0", method: "transfer" },
        // Semver: major > 0
        EntrypointCase { interface: Some("Token"), version_str: "3.2.1", method: "transfer" },
        // Semver: major == 0, minor > 0 (derives "0.minor")
        EntrypointCase { interface: Some("Token"), version_str: "0.9.2", method: "transfer" },
        EntrypointCase { interface: Some("Token"), version_str: "0.1.0", method: "transfer" },
        // Semver: 0.0.z (derives "0.0.z")
        EntrypointCase { interface: Some("Token"), version_str: "0.0.2", method: "transfer" },
        EntrypointCase { interface: Some("Token"), version_str: "0.0.1", method: "transfer" },
        // Edge cases
        EntrypointCase { interface: Some("Token"), version_str: "1.0.0", method: "" },
        EntrypointCase { interface: Some("A"), version_str: "1.0.0", method: "b" },
        EntrypointCase {
            interface: Some("VeryLongInterfaceNameThatExceedsTypicalLength"),
            version_str: "1.0.0",
            method: "very_long_method_name_that_exceeds_typical_length",
        },
    ]
}

pub fn print() {
    let vectors: Vec<Value> = cases()
        .into_iter()
        .map(|case| {
            let sig = NormalizedMethodSignature::new(case.method);
            let id = contract_entrypoint_id(case.interface, case.version_str, &sig);
            let hex = entrypoint_export_name(id).to_lowercase();
            let derived = major_version_from_string(case.version_str);

            json!({
                "interface": case.interface,
                "version_str": case.version_str,
                "derived_major_version": derived,
                "method": case.method,
                "expected_id": hex,
            })
        })
        .collect();

    let output = json!({
        "version": 1,
        "category": "entrypoint_id",
        "vectors": vectors,
    });

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
