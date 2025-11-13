use core::fmt;

pub struct Hex<'a>(&'a [u8]);

impl<'a> fmt::Display for Hex<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> core::result::Result<(), fmt::Error> {
        // f.write_str("hex!(\"")?;
        for b in self.0 {
            write!(f, "{:02x}", b)?;
        }
        // f.write_str("\")")?;
        Ok(())
    }
}

pub fn hex_encode<'a, B: AsRef<[u8]>>(b: &'a B) -> Hex<'a> {
    Hex(b.as_ref())
}
