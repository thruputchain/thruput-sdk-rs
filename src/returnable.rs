pub trait Returnable {
    fn force_revert(&self) -> bool
    {
        false
    }
}

impl<T, E> Returnable for Result<T, E> {
    fn force_revert(&self) -> bool
    {
        self.is_err()
    }
}
impl<T> Returnable for Option<T> {
    fn force_revert(&self) -> bool
    {
        self.is_none()
    }
}

macro_rules! impl_returnable {
    ($($type:ty),*) => {
        $(
            impl $crate::returnable::Returnable for $type {}
        )*
    };
}

impl_returnable!((), bool, i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);

impl<T, const N: usize> Returnable for [T; N] {}
impl<T> Returnable for ::xdrs::Vec<T> {}
impl<T1, T2> Returnable for (T1, T2) {}
impl<T1, T2, T3> Returnable for (T1, T2, T3) {}
impl<T1, T2, T3, T4> Returnable for (T1, T2, T3, T4) {}
impl<T1, T2, T3, T4, T5> Returnable for (T1, T2, T3, T4, T5) {}



