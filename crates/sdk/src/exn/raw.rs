//! Re-throwing for the raw exception types, which live in `groundhog-exn`;
//! see that crate's docs for the two-tier design. Re-throwing is a syscall,
//! so its impls are here.

use crate::re_throw::ReThrow;
use groundhog_exn::{RawException, RawExceptionRef};

/// Re-throw to our caller: abort with the original wire bytes, verbatim.
/// Zero-copy; this is what plain (non-`try_`) generated methods do.
impl ReThrow for RawExceptionRef<'_> {
    #[inline]
    #[cfg_attr(not(target_arch = "riscv32"), track_caller)]
    fn re_throw(self) -> ! {
        #[cfg(all(not(target_arch = "riscv32"), feature = "std"))]
        crate::contract_op::abort_breadcrumb("Re-throwing", "Location", &self);

        crate::contract_op::contract_abort(self.wire())
    }
}

/// Re-throw to our caller with the original wire bytes, verbatim.
impl ReThrow for RawException {
    #[inline]
    #[cfg_attr(not(target_arch = "riscv32"), track_caller)]
    fn re_throw(self) -> ! {
        #[cfg(all(not(target_arch = "riscv32"), feature = "std"))]
        crate::contract_op::abort_breadcrumb("Re-throwing", "Location", &self);

        crate::contract_op::contract_abort(self.wire())
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use crate::exn::{Exn, LibraryExn};

    #[test]
    fn try_as_decodes_matching_selector_and_payload() {
        let payload =
            crate::internal::encode_xdr(&LibraryExn::NonNegativeIntOverflow)
                .unwrap();
        let c = RawException::new(LibraryExn::SELECTOR, &payload);

        assert_eq!(
            c.try_as::<LibraryExn>(),
            Some(LibraryExn::NonNegativeIntOverflow)
        );
    }

    #[test]
    fn try_as_returns_none_for_mismatched_selector() {
        let payload =
            crate::internal::encode_xdr(&LibraryExn::NonNegativeIntOverflow)
                .unwrap();
        // A valid payload for LibraryExn, but tagged with the wrong selector.
        let c = RawException::new(!LibraryExn::SELECTOR, &payload);

        assert_eq!(c.try_as::<LibraryExn>(), None);
    }

    #[test]
    fn try_as_returns_none_for_undecodable_payload() {
        let payload =
            crate::internal::encode_xdr(&LibraryExn::NonNegativeIntOverflow)
                .unwrap();
        // Truncate so the discriminant itself can't be read back.
        let truncated = &payload[..payload.len().saturating_sub(1)];
        let c = RawException::new(LibraryExn::SELECTOR, truncated);

        assert_eq!(c.try_as::<LibraryExn>(), None);
    }
}
