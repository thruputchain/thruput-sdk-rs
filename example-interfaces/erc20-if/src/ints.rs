use sdk::prelude::*;
use xdr_derive::XDR;

#[derive(Clone, Copy, Debug, PartialEq, Eq, XDR, ReadXDR, WriteXDR)]
#[xdrs(as_type = "uint128_xdr")]
pub struct U128(u128);

impl From<uint128_xdr> for U128 {
    fn from(u: uint128_xdr) -> Self {
        Self(((u.highbits as u128) << 64) | (u.lowbits as u128))
    }
}

impl From<U128> for uint128_xdr {
    fn from(u: U128) -> Self {
        uint128_xdr {
            lowbits: (u.0 & 0xffffffffffffffff) as u64,
            highbits: (u.0 >> 64) as u64,
        }
    }
}

#[test]
fn uint128_convert() {
    let u = U128(0xdeadbeef00112233facebead44556677);
    let u2 = Into::<uint128_xdr>::into(u).into();
    assert_eq!(u, u2);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, XDR, ReadXDR, WriteXDR)]
#[xdrs(as_type = "int128_xdr")]
pub struct I128(i128);

impl From<int128_xdr> for I128 {
    fn from(i: int128_xdr) -> Self {
        Self(((i.highbits as i128) << 64) | (i.lowbits as i128))
    }
}

impl From<I128> for int128_xdr {
    fn from(i: I128) -> Self {
        int128_xdr {
            lowbits: (i.0 & 0xffffffffffffffff) as u64,
            highbits: (i.0 >> 64) as i64,
        }
    }
}

#[test]
fn int128_convert() {
    let i = I128(0x00112233facebead44556677deadbeef);
    let i2 = Into::<int128_xdr>::into(i).into();
    assert_eq!(i, i2);

    let i = I128(-i.0);
    let i2 = Into::<int128_xdr>::into(i).into();
    assert_eq!(i, i2);
}
