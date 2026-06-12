use crate::exn::*;
use crate::xdr::*;

pub fn oob_throw(len: u64, access: u64) -> ! {
    library_throw(LibraryExn::StorageIndexOutOfBounds { len, access })
}

pub fn throw_load_exn(_r: ReadError) -> ! {
    library_throw(LibraryExn::StorageSerialization)
}

/// Throw a serialization exception for an entry that exceeds the encode buffer.
/// This can only happen if a raw entry was inserted that is too large for the
/// typed decode path (which encodes through the same fixed buffer on write).
pub fn throw_load_exn_oversized() -> ! {
    library_throw(LibraryExn::StorageSerialization)
}

pub fn throw_store_exn(_: WriteError) -> ! {
    library_throw(LibraryExn::StorageSerialization)
}
