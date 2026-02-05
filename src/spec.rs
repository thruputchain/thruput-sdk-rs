//! Specification of contract interfaces

pub use std::{vec, format, eprintln, string::*, borrow::ToOwned};
pub use xdrs::xdr_repr::*;
pub use xdrs::*;
pub use crate::types::*;
pub use crate::contract::*;

#[derive(Debug)]
pub struct Contract {
    pub interface_id: String,
    pub interface_vers: String,
    pub methods: Vec<Method>,
}

impl Contract {
    fn interface_id_preimage(&self) -> ContractInterfaceIDPreimage {
        ContractInterfaceIDPreimage {
            name: StringM(self.interface_id.as_bytes().to_owned()),
            vers: StringM(self.interface_vers.as_bytes().to_owned()),
        }
    }

    fn entrypoint_id(&self, method_idx: usize) -> ContractEntrypointID {
        use sha2::{Sha256, Digest};

        let interface_bytes = xdrs::txs::encode(&self.interface_id_preimage()).unwrap();
        let interface_hashed = Sha256::digest(&interface_bytes);
        let mut id: [u8; 8] = [0; 8];
        id[0..6].copy_from_slice(&interface_hashed[0..6]);
        let method: u16 = method_idx.try_into().unwrap();
        let method_bytes: [u8; 2] = method.to_be_bytes();
        id[6..8].copy_from_slice(&method_bytes);
        id.into()
    }
    // JSON-like display format
    pub fn print_json_summary(&self) -> String {
        let mut result = format!("{{\n  \"interface\": \"{}\",\n  \"version\": \"{}\",\n  \"methods\": [", 
                                &self.interface_id, &self.interface_vers);
        
        for (i, method) in self.methods.iter().enumerate() {
            if i > 0 { result.push(','); }
            let ret = if let Some(ret) = method.ret.as_ref() {
                ret.get_type_json()
            } else {
                " { \"type\": \"void\" }".to_owned()
            };
            let args = method.args.iter().map(|arg| arg.print_json_summary()).collect::<Vec<String>>().join(",\n");
            result.push_str("\n    {");
            result.push_str(&format!("\n      \"name\": \"{}\"", method.name));
            result.push_str(&format!(",\n      \"return\": {}", ret));
            if let Some(err) = method.err.as_ref() {
                result.push_str(&format!(",\n      \"err\": \"{}\"", err));
            }
            result.push_str(&format!(",\n      \"entrypoint\": \"{}\"", display_entrypoint(self.entrypoint_id(i))));
            result.push_str(&format!(",\n      \"args\": [\n{}\n      ]", args));
            result.push_str("\n    }");
        }
        result.push_str("\n  ]\n}");
        result
    }
}

fn display_entrypoint(id: ContractEntrypointID) -> String {
    let n = u64::from_be_bytes(id.into_inner());
    format!("0x{n:016x}")
}

#[derive(Debug)]
pub struct Method {
    pub name: String,
    pub args: Vec<Arg>,
    // Returns ret if the frame succeeds,
    // err if the frame reverts
    pub ret: Option<xdr::Declaration>,
    pub err: Option<xdr::TypeSpec>,
}

#[derive(Debug)]
pub struct Arg {
    pub name: String,
    pub decl: xdr::Declaration,
}

impl Arg {
    pub fn print_json_summary(&self) -> String {
        format!("        {{\n          \"name\": \"{}\",\n          \"typespec\": {}\n        }}", self.name, self.decl.get_type_json())
    }
}

const _: () = {
    use core::fmt::{Display, Formatter};
    use core::fmt::Result as FmtResult;

    impl Display for Contract {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            writeln!(f, "{}v{} {{", &self.interface_id, &self.interface_vers)?;
            for (i, method) in self.methods.iter().enumerate() {
                let ret = if let Some(ret) = method.ret.as_ref() {
                    ret.to_string()
                } else {
                    "void".to_owned()
                };
                // TODO: something with the error type
                writeln!(f, "\t{}", ret)?;
                writeln!(f, "\t{}(", &method.name)?;
                for arg in &method.args {
                    writeln!(f, "\t\t{},\n", arg.decl)?;
                }
                writeln!(f, "\t) = {};", display_entrypoint(self.entrypoint_id(i)))?;
            }
            f.write_str("}\n")?;
            Ok(())
        }
    }
};

pub trait HasContractSpec {
    fn contract_spec() -> Contract;
}

pub fn json_spec_main<T : HasContractSpec>() {
    use std::env;
    use std::fs::File;
    use std::io::Write;
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <output_filename>", args[0]);
        std::process::exit(1);
    }
    
    let output_path = &args[1];
    let spec = T::contract_spec();
    let json_summary = spec.print_json_summary();
    
    let mut file = File::create(output_path).unwrap();
    file.write_all(json_summary.as_bytes()).unwrap();
}
