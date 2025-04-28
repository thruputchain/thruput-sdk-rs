use crate::syscall::to_offset;
use core::mem::MaybeUninit;
use groundhog_xdr::types::*;

pub trait ToU64_0 {
    fn to_u64_0(&self) -> u64;
}

pub trait ToU64_0Mut {
    fn to_u64_0(&mut self) -> u64;
}

pub trait ToU64_1 {
    fn to_u64_1(&self) -> u64;
}

// Span inputs
impl ToU64_0 for [u8] {
    fn to_u64_0(&self) -> u64 {
        self.as_ptr() as u64
    }
}

impl ToU64_1 for [u8] {
    fn to_u64_1(&self) -> u64 {
        self.len() as u64
    }
}

impl ToU64_0Mut for [MaybeUninit<u8>] {
    fn to_u64_0(&mut self) -> u64 {
        self.as_mut_ptr() as u64
    }
}

impl ToU64_1 for [MaybeUninit<u8>] {
    fn to_u64_1(&self) -> u64 {
        self.len() as u64
    }
}

macro_rules! fixed_size_inputs {
    ($($type:ty),*) => {
        $(
            impl ToU64_0 for &$type {
                fn to_u64_0(&self) -> u64 {
                    to_offset(self) as u64
                }
            }
        )*
    };
}

fixed_size_inputs!(SHA256Hash, AddressAndKey, ContractAddress, i128, u128);

macro_rules! fixed_size_outputs {
    ($($type:ty),*) => {
        $(
            impl ToU64_0Mut for MaybeUninit<$type> {
                fn to_u64_0(&mut self) -> u64 {
                    self.as_mut_ptr() as u64
                }
            }
        )*
    };
}

fixed_size_outputs!(
    SHA256Hash,
    ContractAddress,
    u32,
    u64,
    u128,
    i32,
    i64,
    i128
);

macro_rules! primitive_type_inputs {
    ($($type:ty),*) => {
        $(
            impl ToU64_0 for $type {
                fn to_u64_0(&self) -> u64 {
                    *self as u64
                }
            }
        )*
    };
}

primitive_type_inputs!(u32, u64, i32, i64);

// Optional input arguments
impl<T: ToU64_0> ToU64_0 for Option<T> {
    fn to_u64_0(&self) -> u64 {
        if let Some(x) = self {
            return x.to_u64_0();
        }
        u64::MAX
    }
}

// Custom parses
impl ToU64_0 for Signature {
    fn to_u64_0(&self) -> u64 {
        self.scheme() as u64
    }
}

impl ToU64_1 for Signature {
    fn to_u64_1(&self) -> u64 {
        match self {
            Signature::Ed25519 { ed25519: sig } => to_offset(sig) as u64,
            #[allow(unreachable_patterns)]
            _ => u64::MAX,
        }
    }
}

impl ToU64_0 for PublicKey {
    fn to_u64_0(&self) -> u64 {
        self.scheme() as u64
    }
}

impl ToU64_1 for PublicKey {
    fn to_u64_1(&self) -> u64 {
        match self {
            PublicKey::Ed25519 { ed25519: pk } => to_offset(pk) as u64,
            #[allow(unreachable_patterns)]
            _ => u64::MAX,
        }
    }
}
