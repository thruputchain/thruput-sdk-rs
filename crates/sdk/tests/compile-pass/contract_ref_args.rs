#![allow(unexpected_cfgs)]

//! Every kind of `&T` parameter compiles: borrowed from calldata,
//! decoded into a local, slices, and `#[contract_type]` values.

use thruput_sdk::xdr::{BytesM, VecM};
use thruput_sdk::*;

#[contract_type]
pub struct Point {
    pub x: u32,
    pub y: u32,
}

#[thruput_sdk::storage]
pub struct BareContract;

#[contract]
impl BareContract {
    pub fn borrowed(
        &self,
        a: &ContractAddress,
        b: &[u8],
        c: &str,
        d: &[u8; 4],
        e: &SHA256Hash,
        f: &Ed25519PublicKey,
    ) -> u32 {
        (a.as_ref().len() + b.len() + c.len() + d.len() + e.as_ref().len()
            + f.as_ref().len()) as u32
    }

    pub fn owned(
        &self,
        a: &u32,
        b: &Option<u64>,
        c: &(u32, bool),
        d: &[u32; 2],
        e: &heapless::Vec<u8, 4>,
        f: &heapless::String<4>,
        g: &VecM<u32, 4>,
        h: &BytesM<4>,
        i: &Point,
    ) -> u32 {
        *a + b.unwrap_or(0) as u32 + c.0 + d[0] + e.len() as u32
            + f.len() as u32 + g.len() as u32 + h.len() as u32 + i.x
    }

    pub fn slices(
        &self,
        a: &[u32],
        b: &[ContractAddress],
        c: &[&[u8]],
        d: &[&str],
        e: &[Point],
        f: &[[u8; 8]],
    ) -> u32 {
        (a.len() + b.len() + c.len() + d.len() + e.len() + f.len()) as u32
    }

    pub fn byte_string_vectors(&self, a: VecM<&[u8], 4>, b: &VecM<&[u8], 4>) -> u32 {
        (a.len() + b.len()) as u32
    }
}

fn main() {}
