macro_rules! impl_read_xdr_path_via_from {
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
