//! Routines for reading transaction AuthClaims.
//!
//! These routines are thin wrappers around VM syscalls.
// TODO(geoff): Think about the following questions:
// 1: is there a better interface in which to wrap these calls?
// 2: Do we want some runtime-checked "single-use" checks for these authclaims?

#[allow(unused_imports)] // import is for docs
use crate::SyscallErrorCode;
use crate::sys;
use crate::sysresult::SysResult as Result;
use core::mem::MaybeUninit;
use groundhog_xdr::auth_claim::AuthClaim;

pub fn get_auth_claim(index: u32) -> Result<AuthClaim> {
    let mut auth_claim = MaybeUninit::uninit();
    sys::uninit_GetAuthClaim(index, &mut auth_claim)?;
    Ok(unsafe { auth_claim.assume_init() })
}
