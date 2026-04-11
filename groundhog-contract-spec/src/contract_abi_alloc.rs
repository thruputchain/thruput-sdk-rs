#[doc(hidden)]
pub trait ContractAbiAllocEnabled {}

impl ContractAbiAllocEnabled for () {}
impl ContractAbiAllocEnabled for bool {}
impl ContractAbiAllocEnabled for u8 {}
impl ContractAbiAllocEnabled for u32 {}
impl ContractAbiAllocEnabled for i32 {}
impl ContractAbiAllocEnabled for u64 {}
impl ContractAbiAllocEnabled for i64 {}
impl ContractAbiAllocEnabled for u128 {}
impl ContractAbiAllocEnabled for i128 {}
impl ContractAbiAllocEnabled for str {}
impl ContractAbiAllocEnabled for [u8] {}
impl ContractAbiAllocEnabled for groundhog_xdr::types::ContractAddress {}
impl ContractAbiAllocEnabled for groundhog_xdr::types::BlockHeight {}
impl ContractAbiAllocEnabled for groundhog_xdr::types::SHA256Hash {}

impl<T> ContractAbiAllocEnabled for &T where T: ContractAbiAllocEnabled {}
impl<T> ContractAbiAllocEnabled for &mut T where T: ContractAbiAllocEnabled {}
impl<T> ContractAbiAllocEnabled for core::option::Option<T> where
    T: ContractAbiAllocEnabled
{
}

impl<T, const N: usize> ContractAbiAllocEnabled for [T; N] where
    T: ContractAbiAllocEnabled
{
}

macro_rules! impl_contract_abi_alloc_enabled_tuple {
    ($(($($name:ident),+)),+ $(,)?) => {
        $(
            impl<$($name),+> ContractAbiAllocEnabled for ($($name,)+)
            where
                $($name: ContractAbiAllocEnabled),+
            {
            }
        )+
    };
}

impl_contract_abi_alloc_enabled_tuple!(
    (A),
    (A, B),
    (A, B, C),
    (A, B, C, D),
    (A, B, C, D, E),
    (A, B, C, D, E, F),
    (A, B, C, D, E, F, G),
    (A, B, C, D, E, F, G, H),
    (A, B, C, D, E, F, G, H, I),
    (A, B, C, D, E, F, G, H, I, J),
    (A, B, C, D, E, F, G, H, I, J, K),
    (A, B, C, D, E, F, G, H, I, J, K, L),
    (A, B, C, D, E, F, G, H, I, J, K, L, M),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O),
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P),
);

#[cfg(feature = "alloc")]
impl<T> ContractAbiAllocEnabled for ::alloc::vec::Vec<T> where
    T: ContractAbiAllocEnabled
{
}

#[cfg(feature = "alloc")]
impl<T, const N: u32> ContractAbiAllocEnabled for ::xdrs::VecM<T, N> where
    T: ContractAbiAllocEnabled
{
}

#[cfg(feature = "alloc")]
impl ContractAbiAllocEnabled for ::alloc::string::String {}
