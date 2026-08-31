//! `match_exn!` and the [`MatchExn`] trait behind it.
//!
//! A Groundhog exception is self-describing on the wire —
//! `[CXS(selector)][payload]` — and the selector identifies the type
//! independently of any interface's `throws` list. That is what lets
//! [`match_exn!`](crate::match_exn) match an error declared by a base
//! interface and one defined by an extension side by side, as peers.
//!
//! The user-facing rules live on the macro itself; this module holds the
//! trait that decides what it can match on, and the decode helper its
//! expansion calls.

use super::{Exn, RawException, RawExceptionRef};
use crate::call::RawCallError;
use crate::xdr::ReadXDROwned;

/// What [`match_exn!`](crate::match_exn) can match on.
///
/// Implemented for [`CallError`](crate::call::CallError) (the error type of
/// every generated `try_*` method), for an owned [`RawException`], and for
/// a borrowed [`RawExceptionRef`] — the last of which lets a caller match
/// inside [`try_call_raw_with`](crate::call::try_call_raw_with)'s `on_exn`
/// closure against the call scratch directly, with no copy at all.
///
/// Returning a reborrow by value, rather than a reference, is what keeps
/// all three impls trivial.
pub trait MatchExn {
    /// The caught exception, or `None` if this failure carries none (a
    /// fault, an invalid call, an encode/decode failure).
    fn as_exn(&self) -> Option<RawExceptionRef<'_>>;

    /// Decode as `E` if this failure carries an exception of that type.
    ///
    /// The single-type shorthand for [`match_exn!`](crate::match_exn) — no
    /// `Unknown` detour, whatever the called interface declared it
    /// `throws`:
    ///
    /// ```ignore
    /// assert_eq!(
    ///     token.try_transfer(&to, 1).unwrap_err().try_as(),
    ///     Some(ERC20PausableError::EnforcedPause),
    /// );
    /// ```
    ///
    /// `None` covers all three ways this can miss: no exception at all, a
    /// different exception type, or an `E`-tagged payload that fails to
    /// decode.
    fn try_as<E: Exn + ReadXDROwned>(&self) -> Option<E> {
        self.as_exn().and_then(|e| e.try_as())
    }
}

impl MatchExn for RawException {
    fn as_exn(&self) -> Option<RawExceptionRef<'_>> {
        Some(self.as_ref_exn())
    }
}

impl MatchExn for RawExceptionRef<'_> {
    fn as_exn(&self) -> Option<RawExceptionRef<'_>> {
        // Covariance in the payload lifetime makes the copy usable here.
        Some(*self)
    }
}

impl<X: MatchExn> MatchExn for RawCallError<X> {
    fn as_exn(&self) -> Option<RawExceptionRef<'_>> {
        match self {
            RawCallError::Exception(x) => x.as_exn(),
            _ => None,
        }
    }
}

/// A reference matches as its referent, so `match_exn!(&e, { .. })` and
/// `match_exn!(e, { .. })` behave alike.
impl<T: MatchExn + ?Sized> MatchExn for &T {
    fn as_exn(&self) -> Option<RawExceptionRef<'_>> {
        (**self).as_exn()
    }
}

/// One arm's decode attempt. Not public API: `match_exn!` emits calls to it.
///
/// `T` is left entirely to inference — the arm's pattern supplies it.
#[doc(hidden)]
pub fn __try_as<T: Exn + ReadXDROwned>(
    exn: Option<RawExceptionRef<'_>>,
) -> Option<T> {
    exn.and_then(|e| e.try_as())
}

/// Match a caught exception against any exception type, whatever the called
/// interface declared it `throws`.
///
/// ```ignore
/// match token.try_transfer(&to, amount) {
///     Ok(()) => { /* ... */ }
///     Err(CallError::Fault(f)) => { /* ... */ }
///     Err(e) => match_exn!(e, {
///         ERC20Error::InsufficientBalance    => { /* ... */ }
///         ERC20PausableError::EnforcedPause  => { /* ... */ }
///         LibraryExn::NonNegativeIntOverflow => { /* ... */ }
///         _ => e.re_throw(),
///     }),
/// }
/// ```
///
/// Everything structural — `Ok`, [`Fault`](crate::call::RawCallError::Fault),
/// [`Invalid`](crate::call::RawCallError::Invalid), the encode/decode
/// failures — stays an ordinary Rust `match`, with real exhaustiveness
/// checking. This macro's only job is typed downcasting. It accepts anything
/// implementing [`MatchExn`](crate::exn::MatchExn): a
/// [`CallError`](crate::call::CallError), an owned
/// [`RawException`](crate::exn::RawException), or a borrowed
/// [`RawExceptionRef`](crate::exn::RawExceptionRef).
///
/// # How arms are matched
///
/// Arms are tried **in order, first match wins** — they lower to a chain of
/// `if let Some(<pattern>) = raw.try_as()`, not to a `match`. Each attempt is
/// guarded by a selector comparison, so an arm naming an unrelated type costs
/// one integer compare.
///
/// The macro never names the exception type: **the arm's pattern pins `T` in
/// [`try_as::<T>`](crate::exn::RawExceptionRef::try_as) by ordinary type
/// inference**, because a path pattern like `ERC20Error::TooLarge` unifies the
/// scrutinee's type with `ERC20Error`. Two constraints follow from that, and
/// they are mechanical rather than stylistic:
///
/// - **Every typed arm must be path-rooted** — `Type::Variant`,
///   `Type::Variant { .. }`, or a unit-struct `Type`. A bare identifier is a
///   *binding*, which leaves `T` unconstrained and fails inference with
///   "type annotations needed". The untyped escape hatch is `_`, where the
///   scrutinee itself is still in scope.
/// - **`|` cannot cross types.** `ERC20Error::A | ERC20PausableError::B`
///   would be one `try_as` with two `T`s. Use two arms. Alternates *within*
///   one type are fine.
///
/// Guards (`Pat if cond =>`) work, and a failing guard falls through to the
/// later arms rather than straight to `_`.
///
/// # The `_` arm is required
///
/// Arms are sequential rather than exhaustiveness-checked, so nothing can
/// prove the typed arms cover every case — and an exception whose selector
/// matches no arm is the ordinary case, not an edge one. Requiring `_` keeps
/// that control flow visible at the call site. A typical use is to propagate
/// the error with `_ => e.re_throw()`.  (That arguably could be the invisible
/// default, but isn't for the sake of clarity.)
///
/// # Decode failures
///
/// [`try_as`](crate::exn::RawExceptionRef::try_as) returns `None` both for a
/// non-matching selector and for a matching selector whose payload fails to
/// decode, so both fall to `_`. To tell them apart, compare `raw.selector()`
/// against `E::SELECTOR` there; no separate API is needed for it.
///
/// # Matching a single type
///
/// [`MatchExn::try_as`](crate::exn::MatchExn::try_as) is the shorthand when
/// only one type is of interest:
///
/// ```ignore
/// assert_eq!(
///     token.try_transfer(&to, 1).unwrap_err().try_as(),
///     Some(ERC20PausableError::EnforcedPause),
/// );
/// ```
#[macro_export]
macro_rules! match_exn {
    ($e:expr, { $($arms:tt)* }) => {{
        // The labeled block is a `let` initializer purely so a *statement*
        // attribute can cover every `break` inside it (attributes on
        // expressions are still unstable). Two expected-and-harmless
        // conditions need silencing: an arm body that diverges (`panic!`,
        // `re_throw()`) leaves the `break` carrying its value unreachable
        // and makes it a diverging sub-expression, and a `match_exn!` whose
        // only arm is `_` never breaks at all. `_ => e.re_throw()` is the
        // documented way to propagate, so this must not warn in user
        // crates building with `-D warnings`.
        #[allow(
            unreachable_code,
            unused_labels,
            clippy::diverging_sub_expression
        )]
        let __match_exn_result = '__match_exn: {
            // `&$e`, not `$e`: a `let` extends the lifetime of a temporary
            // scrutinee, and a named one is merely borrowed — leaving it
            // usable inside the `_` arm.
            let __match_exn_scrutinee = &$e;
            let __match_exn_raw =
                $crate::exn::MatchExn::as_exn(__match_exn_scrutinee);
            $crate::__match_exn_arms!(
                '__match_exn, __match_exn_raw, $($arms)*
            )
        };
        __match_exn_result
    }};
}

/// The arm muncher behind [`match_exn!`](crate::match_exn). Not public API.
///
/// Rule order is important: the `_` rules must precede the `$pat:pat`
/// rules, since `_` is itself a valid pattern and would otherwise be
/// lowered to a `try_as` with nothing to infer `T` from.
#[doc(hidden)]
#[macro_export]
macro_rules! __match_exn_arms {
    // -- terminal `_` arm ------------------------------------------------
    ($lbl:lifetime, $exn:ident, _ => $body:block $(,)?) => {
        break $lbl ($body)
    };
    ($lbl:lifetime, $exn:ident, _ => $body:expr $(,)?) => {
        break $lbl ({ $body })
    };
    // A `_` arm with anything after it: later arms would be unreachable.
    ($lbl:lifetime, $exn:ident, _ => $($rest:tt)*) => {
        ::core::compile_error!(
            "the `_` arm of `match_exn!` must come last; arms after it \
             could never be reached"
        )
    };

    // -- guarded typed arm ----------------------------------------------
    ($lbl:lifetime, $exn:ident,
     $pat:pat if $guard:expr => $body:block $(,)? $($rest:tt)*) => {{
        if let ::core::option::Option::Some($pat) = $crate::exn::__try_as($exn)
            && $guard
        {
            break $lbl ($body);
        }
        $crate::__match_exn_arms!($lbl, $exn, $($rest)*)
    }};
    ($lbl:lifetime, $exn:ident,
     $pat:pat if $guard:expr => $body:expr , $($rest:tt)*) => {{
        if let ::core::option::Option::Some($pat) = $crate::exn::__try_as($exn)
            && $guard
        {
            break $lbl ({ $body });
        }
        $crate::__match_exn_arms!($lbl, $exn, $($rest)*)
    }};

    // -- plain typed arm -------------------------------------------------
    ($lbl:lifetime, $exn:ident, $pat:pat => $body:block $(,)? $($rest:tt)*) => {{
        if let ::core::option::Option::Some($pat) = $crate::exn::__try_as($exn)
        {
            break $lbl ($body);
        }
        $crate::__match_exn_arms!($lbl, $exn, $($rest)*)
    }};
    ($lbl:lifetime, $exn:ident, $pat:pat => $body:expr , $($rest:tt)*) => {{
        if let ::core::option::Option::Some($pat) = $crate::exn::__try_as($exn)
        {
            break $lbl ({ $body });
        }
        $crate::__match_exn_arms!($lbl, $exn, $($rest)*)
    }};

    // -- ran out of arms without a `_` -----------------------------------
    ($lbl:lifetime, $exn:ident $(,)?) => {
        ::core::compile_error!(
            "`match_exn!` requires a final `_ => ...` arm: its arms are \
             tried in order rather than checked for exhaustiveness, so an \
             exception matching none of them must be handled explicitly \
             (`_ => e.re_throw()` propagates it)"
        )
    };
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use crate::exn::{FaultString, LibraryExn};
    use crate::internal::encode_xdr;
    use crate::xdr::*;

    // Hand-written `Exn` impls rather than `#[exn]`: these types exist only
    // to exercise selector dispatch, and never reach a contract spec.
    #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
    #[repr(i32)]
    enum Alpha {
        NotFound = 1,
        Big { n: u64 } = 2,
    }

    impl Exn for Alpha {
        const SELECTOR: u64 = 0xA1FA_0000_0000_0001;
    }

    #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
    #[repr(i32)]
    enum Beta {
        Rejected = 1,
    }

    impl Exn for Beta {
        const SELECTOR: u64 = 0xBE7A_0000_0000_0001;
    }

    /// Build the owned exception a caller would catch for `e`.
    fn caught<E: Exn + WriteXDR>(e: E) -> RawException {
        RawException::new(E::SELECTOR, &encode_xdr(&e).unwrap())
    }

    fn err<E: Exn + WriteXDR>(e: E) -> RawCallError {
        RawCallError::Exception(caught(e))
    }

    fn classify(e: RawCallError) -> alloc::string::String {
        use alloc::string::ToString;
        match_exn!(e, {
            Alpha::Big { n } if n > 100 => "alpha-big-huge".to_string(),
            Alpha::Big { n } => alloc::format!("alpha-big-{n}"),
            Alpha::NotFound => "alpha-not-found".to_string(),
            Beta::Rejected => "beta-rejected".to_string(),
            LibraryExn::NonNegativeIntOverflow => "lib-overflow".to_string(),
            _ => alloc::format!("other:{e:?}"),
        })
    }

    #[test]
    fn matches_each_type_by_selector() {
        assert_eq!(classify(err(Alpha::NotFound)), "alpha-not-found");
        assert_eq!(classify(err(Beta::Rejected)), "beta-rejected");
        assert_eq!(
            classify(err(LibraryExn::NonNegativeIntOverflow)),
            "lib-overflow"
        );
    }

    #[test]
    fn guard_failure_falls_through_to_a_later_arm_not_to_wildcard() {
        assert_eq!(classify(err(Alpha::Big { n: 500 })), "alpha-big-huge");
        assert_eq!(classify(err(Alpha::Big { n: 7 })), "alpha-big-7");
    }

    #[test]
    fn non_exception_failures_reach_the_wildcard() {
        assert!(classify(RawCallError::Invalid).starts_with("other:"));
        assert!(
            classify(RawCallError::Fault(FaultString::from_display("boom")))
                .starts_with("other:")
        );
    }

    #[test]
    fn unrecognised_selector_reaches_the_wildcard() {
        let raw = RawException::new(!0u64, b"whatever");
        assert!(classify(RawCallError::Exception(raw)).starts_with("other:"));
    }

    #[test]
    fn undecodable_payload_of_a_matching_selector_reaches_the_wildcard() {
        // Right selector, payload truncated past recognition.
        let raw = RawException::new(Alpha::SELECTOR, &[]);
        assert!(classify(RawCallError::Exception(raw)).starts_with("other:"));
    }

    #[test]
    fn matches_an_owned_raw_exception_directly() {
        let raw = caught(Beta::Rejected);
        let got = match_exn!(raw, {
            Beta::Rejected => 1u32,
            _ => 0,
        });
        assert_eq!(got, 1);
    }

    #[test]
    fn alternates_within_one_type_and_at_binding() {
        let got = match_exn!(err(Alpha::Big { n: 3 }), {
            Alpha::NotFound | Alpha::Big { .. } => 1u32,
            _ => 0,
        });
        assert_eq!(got, 1);

        let bound = match_exn!(err(Alpha::NotFound), {
            v @ Alpha::NotFound => alloc::format!("{v:?}"),
            _ => alloc::string::String::new(),
        });
        assert_eq!(bound, "NotFound");
    }

    #[test]
    fn arms_are_tried_in_order_first_match_wins() {
        let got = match_exn!(err(Alpha::NotFound), {
            Alpha::NotFound => 1u32,
            Alpha::NotFound => 2,
            _ => 0,
        });
        assert_eq!(got, 1);
    }

    /// A diverging arm body leaves the `break` that would carry its value
    /// unreachable. That must not warn (these crates build with
    /// `-D warnings`).
    #[test]
    #[should_panic(expected = "diverging arm")]
    fn diverging_arm_bodies_do_not_warn() {
        let _: u32 = match_exn!(err(Beta::Rejected), {
            Alpha::NotFound => 1,
            Beta::Rejected => panic!("diverging arm"),
            _ => 0,
        });
    }

    #[test]
    fn diverging_wildcard_body_does_not_warn() {
        let got = match_exn!(err(Beta::Rejected), {
            Beta::Rejected => 1u32,
            _ => panic!("unreachable in this case"),
        });
        assert_eq!(got, 1);
    }

    /// The documented propagate idiom is `_ => e.re_throw()`, which *moves*
    /// the scrutinee the macro holds a borrow of — NLL has to see that borrow
    /// end before the `_` body. `re_throw` aborts, so consume by value here
    /// instead; `compile-pass/match_exn_rethrow.rs` pins the real spelling.
    #[test]
    fn wildcard_arm_moves_the_named_scrutinee() {
        fn consume(e: RawCallError) -> u32 {
            match e {
                RawCallError::Exception(_) => 7,
                _ => 0,
            }
        }

        let e = err(Alpha::NotFound);
        let got = match_exn!(e, {
            Beta::Rejected => 1u32,
            _ => consume(e),
        });
        assert_eq!(got, 7);
    }

    #[test]
    fn wildcard_only_is_legal() {
        let got = match_exn!(err(Alpha::NotFound), { _ => 5u32 });
        assert_eq!(got, 5);
    }

    #[test]
    fn block_bodies_need_no_trailing_comma() {
        let got = match_exn!(err(Beta::Rejected), {
            Alpha::NotFound => { 1u32 }
            Beta::Rejected => { 2 }
            _ => { 0 }
        });
        assert_eq!(got, 2);
    }
}
