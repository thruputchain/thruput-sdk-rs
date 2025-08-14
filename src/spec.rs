//! Specification of contract interfaces

pub use xdrs::xdr_repr::*;
pub use xdrs::xdr_common::*;
pub use groundhog_xdr::types::*;
pub use groundhog_xdr::contract::*;
pub use std::vec;

#[derive(Debug)]
pub struct Contract {
    pub interface_id: String,
    pub interface_vers: String,
    pub methods: Vec<Method>,
}

impl Contract {
    fn interface_id_preimage(&self) -> ContractInterfaceIDPreimage {
        ContractInterfaceIDPreimage {
            name: BytesM(self.interface_id.as_bytes().to_owned()),
            vers: BytesM(self.interface_vers.as_bytes().to_owned()),
        }
    }

    fn entrypoint_id(&self, method_idx: usize) -> ContractEntrypointID {
        use sha2::{Sha256, Digest};

        let interface_bytes = xdrs::serialize_txs::encode(&self.interface_id_preimage()).unwrap();
        let interface_hashed = Sha256::digest(&interface_bytes);
        let mut id: [u8; 8] = [0; 8];
        id[0..6].copy_from_slice(&interface_hashed[0..6]);
        let method: u16 = method_idx.try_into().unwrap();
        let method_bytes: [u8; 2] = method.to_be_bytes();
        id[6..8].copy_from_slice(&method_bytes);
        id.into()
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
    pub ret: Option<xdr::TypeSpec>,
}

#[derive(Debug)]
pub struct Arg {
    pub name: String,
    pub decl: xdr::Declaration,
}

const _: () = {
    use core::fmt::{Display, Formatter};
    use core::fmt::Result as FmtResult;

    impl Display for Contract {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            write!(f, "{}v{} {{\n", &self.interface_id, &self.interface_vers)?;
            for (i, method) in self.methods.iter().enumerate() {
                let ret = if let Some(ret) = method.ret.as_ref() {
                    ret.to_string()
                } else {
                    "void".to_owned()
                };
                write!(f, "\t{}\n", ret)?;
                write!(f, "\t{}(\n", &method.name)?;
                for arg in &method.args {
                    write!(f, "\t\t{},\n", arg.decl)?;
                }
                write!(f, "\t) = {};\n\n", display_entrypoint(self.entrypoint_id(i)))?;
            }
            f.write_str("}\n")?;
            Ok(())
        }
    }
};
