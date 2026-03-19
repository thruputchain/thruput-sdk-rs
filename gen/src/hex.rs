use core::fmt;

pub struct Hex<'a>(&'a [u8]);

impl<'a> fmt::Display for Hex<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        for b in self.0 {
            write!(f, "{:02X}", b)?;
        }
        Ok(())
    }
}

pub fn hex_encode<'a, B: AsRef<[u8]>>(b: &'a B) -> Hex<'a> {
    Hex(b.as_ref())
}
