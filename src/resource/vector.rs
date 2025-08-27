use xdrs::{ReadXDR, WriteXDR};
use core::marker::PhantomData;

#[derive(ReadXDR, WriteXDR)]
pub struct HVec<T> {
    length: u64,
    _marker: PhantomData<T>,
}
