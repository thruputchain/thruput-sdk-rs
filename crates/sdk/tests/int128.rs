mod common;
test_cfg!();

use sdk::types::{int128_xdr, uint128_xdr};
use xdrs::txs::encode;

// Can we roundtrip-convert u128->uint128_xdr->u128?
#[test]
fn uint128_roundtrip() {
    let u: u128 = 0xdeadbeef00112233facebead44556677;
    let u2 = Into::<uint128_xdr>::into(u).into();
    assert_eq!(u, u2);
}

// Can we roundtrip-convert i128->int128_xdr->i128?
#[test]
fn int128_roundtrip() {
    let i: i128 = 0x00112233facebead44556677deadbeef;
    let i2 = Into::<int128_xdr>::into(i).into();
    assert_eq!(i, i2);

    let i = -i;
    let i2 = Into::<int128_xdr>::into(i).into();
    assert_eq!(i, i2);
}

// We want the groundhog types to serialize the same as the
// default encoding from xdrs ...

// Do u128 and uint128_xdr serialize the same?
#[test]
fn int128_xdr_encoding() {
    let i: i128 = 0x00112233facebead44556677deadbeef;
    let i_xdr: int128_xdr = i.into();

    assert_eq!(encode(&i).unwrap(), encode(&i_xdr).unwrap());
}

// Do i128 and int128_xdr serialize the same?
#[test]
fn uint128_xdr_encoding() {
    let u: u128 = 0xdeadbeef00112233facebead44556677;
    let u_xdr: uint128_xdr = u.into();

    assert_eq!(encode(&u).unwrap(), encode(&u_xdr).unwrap());
}
