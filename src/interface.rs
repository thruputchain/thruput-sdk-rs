//! Specification of contract interfaces

use xdr_repr::xdr as xdr;
// use xdr_repr::ty::*;
pub use xdr_repr::construct::*;
pub use std::vec;
pub use groundhog_xdr::types::*;
pub use groundhog_xdr::contract::*;
pub use xdr_repr::format::Format;

#[derive(Debug)]
pub struct Contract {
    // pub interface_id_preimage: ContractInterfaceIDPreimage,
    pub methods: Vec<Method>,
}

#[derive(Debug)]
pub struct Method {
    pub name: String,
    pub args: Vec<Arg>,
}

#[derive(Debug)]
pub struct Arg {
    pub name: String,
    pub decl: xdr::Declaration,
}


    /*
const _: () = {
    use core::fmt::{Display, Formatter};
    use core::fmt::Result as FmtResult;

    impl Display for Contract {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            for method in &self.methods {
                f.write_str(&method.name)?;
                f.write_str(", ")
            }
        }
    }

    impl Display for Method {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            f.write_str(&self.name)?;
            f.write_str(", ")
        }
    }
};
    */
