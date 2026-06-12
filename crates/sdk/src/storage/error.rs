use crate::exn::*;
use crate::xdr::*;

pub fn oob_throw(len: u64, access: u64) -> ! {
    library_throw(LibraryExn::StorageIndexOutOfBounds { len, access })
}

pub fn throw_load_exn(_r: ReadError) -> ! {
    library_throw(LibraryExn::StorageSerialization)
}

pub fn throw_store_exn(_: WriteError) -> ! {
    library_throw(LibraryExn::StorageSerialization)
}
