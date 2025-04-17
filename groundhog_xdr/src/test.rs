#[cfg(test)]
mod tests {
    use super::*;

    use crate::{ReadXdr, WriteXdr};

    #[test]
    fn loop_union() {
        let mut bytes: [u8; 64] = [0; 64];
        bytes[10] = 5;
        bytes[23] = 12;
        let x = crate::Signature::Ed25519 {
            ed25519: bytes.try_into().unwrap(),
        };

        let serialized = x.to_xdr(crate::Limits::none()).unwrap();

        let y = crate::Signature::from_xdr(&serialized, crate::Limits::none())
            .unwrap();

        assert_eq!(x, y);
    }
}
