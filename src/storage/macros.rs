macro_rules! impl_default_via_keypath {
    { $id:ident$(<$($param:ident),+>)? } => {
        const _: () = {
            use crate::storage::*;

            impl$(<$($param),+>)? DefaultFromKeypath for $id$(<$($param),+>)? {
                fn default_from_keypath(keypath: &Keypath) -> Self {
                    keypath.clone().into()
                }
            }
        };
    };
}

macro_rules! impl_load_via_keypath {
    { $id:ident$(<$($param:ident),+>)? } => {
        const _: () = {
            use crate::storage::*;
            use crate::storage::error::Result;

            impl$(<$($param),+>)? LoadKeypath for $id$(<$($param),+>)? {
                fn load_keypath(keypath: &Keypath) -> Result<Self> {
                    Ok(keypath.clone().into())
                }
            }
        };
    };
}

macro_rules! impl_load_via_read {
    { $id:ident$(<$($param:ident),+>)? } => {
        const _: () = {
            use crate::storage::*;
            use crate::storage::error::Result;

            impl$(<$($param),+>)? LoadKeypath for $id$(<$($param),+>)? {
                fn load_keypath(keypath: &Keypath) -> Result<Self> {
                    load_readable(keypath)
                }
            }
        };
    };
}

macro_rules! impl_read_via_keypath {
    { $id:ident$(<$($param:ident),+>)? } => {
        const _: () = {
            use xdrs::ReadResult;
            use crate::storage::*;

            impl$(<$($param),+>)? ReadXDRPath for $id$(<$($param),+>)? {
                fn read_xdr_path<'r, R>(r: &mut R) -> ReadResult<Self>
                where
                    R: ReaderPath<'r, Keypath>
                {
                    Ok(r.path().into())
                }
            }
        };
    };
}

macro_rules! impl_write_xdr_empty {
    { $id:ident$(<$($param:ident),+>)? } => {
        const _: () = {
            use xdrs::{WriteXDR, Writer, WriteResult};

            impl$(<$($param),+>)? WriteXDR for $id$(<$($param),+>)? {
                fn write_xdr<F, W: Writer<F>>(&self, _w: &mut W) -> WriteResult {
                    // Write nothing

                    Ok(())
                }
            }
        };
    };
}
