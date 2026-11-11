//! Ristretto255 host API.

use core::mem::MaybeUninit;

use crate::sys::{self, SysErr};
use crate::types::Ristretto255Point;

/// Experimental API: compute a variable-time Ristretto255 multiscalar multiplication.
///
/// Each packed term is 64 bytes: a canonical 32-byte little-endian scalar,
/// then a canonical 32-byte compressed Ristretto255 point. The runtime
/// validates every term. Empty input returns the identity. Zero scalars,
/// identity points, and duplicate points are allowed. The limit is 256 terms.
///
/// Allocates no guest memory. Use only for verification data visible to the
/// executor. Returns an error for malformed input.
/// Gas exhaustion faults the calling frame.
pub fn ristretto255_msm(
    packed_terms: &[u8],
) -> Result<Ristretto255Point, SysErr> {
    let mut output = MaybeUninit::<Ristretto255Point>::uninit();
    sys::uninit_Ristretto255Msm(packed_terms, &mut output)?;
    // SAFETY: a successful syscall initializes the complete output point.
    Ok(unsafe { output.assume_init() })
}
