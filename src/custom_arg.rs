#![allow(dead_code)] // XXX

use crate::rt::trap;
use crate::syscall::to_offset;
use ::groundhog_xdr::types::{CryptographicScheme, PublicKey, Signature};

pub trait TwoCustomArgs {
    fn to_u64s(&self) -> (u64, u64);
}

// required for custom arg handling in syscall
impl TwoCustomArgs for Signature {
    fn to_u64s(&self) -> (u64, u64) {
        match self {
            Self::Ed25519 { ed25519: sig } => {
                (CryptographicScheme::Ed25519 as u64, to_offset(sig) as u64)
            }
            #[allow(unreachable_patterns)]
            _ => {
                trap();
            }
        }
    }
}

impl TwoCustomArgs for PublicKey {
    fn to_u64s(&self) -> (u64, u64) {
        match self {
            Self::Ed25519 { ed25519: pk } => {
                (CryptographicScheme::Ed25519 as u64, to_offset(pk) as u64)
            }
            #[allow(unreachable_patterns)]
            _ => {
                trap();
            }
        }
    }
}
