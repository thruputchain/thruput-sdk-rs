use core::marker::PhantomData;
use xdrs::{ReadXDR, WriteXDR};

#[derive(ReadXDR, WriteXDR)]
pub struct KMap<Key, Val> {
    _marker: PhantomData<fn() -> (Key, Val)>,
}
