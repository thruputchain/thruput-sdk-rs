macro_rules! impl_from_keypath {
    { $id:ident$(<$($param:ident),+>)? } => {
        const _: () = {
            use crate::storage::*;

            impl$(<$($param),+>)? FromKeypath for $id$(<$($param),+>)? {
                fn from_keypath(keypath: &Keypath) -> Self {
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
    { $( $id:ident$(<$($param:ident),+>)? ),+ $(,)? } => {
        const _: () = {
            use crate::storage::*;
            use crate::storage::error::Result;

            $(
            impl$(<$($param),+>)? LoadKeypath for $id$(<$($param),+>)? {
                fn load_keypath(keypath: &Keypath) -> Result<Self> {
                    load_readable(keypath)
                }
            }
            )*
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
