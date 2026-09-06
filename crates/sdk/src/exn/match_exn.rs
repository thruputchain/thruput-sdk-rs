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
    /// decode. Use [`try_decode`](Self::try_decode) to separate the last of
    /// those from the other two.
    fn try_as<E: Exn + ReadXDROwned>(&self) -> Option<E> {
        self.as_exn().and_then(|e| e.try_as())
    }

    /// [`try_as`](Self::try_as) with the decode failure kept separate:
    ///
    /// - `None` — this failure carries no exception, or one belonging to
    ///   some other type. (Match on
    ///   [`CallError::Exception`](crate::call::RawCallError::Exception) to
    ///   tell those two apart.)
    /// - `Some(Ok(e))` — the selector matched and the payload decoded.
    /// - `Some(Err(_))` — the selector matched, but the payload does not
    ///   decode as `E`.
    ///
    /// See [`RawExceptionRef::try_decode`] for when the third case arises;
    /// it is not only forgery. Nothing in the SDK acts on it for you —
    /// `match_exn!` treats it as a non-match, so a caller that must not
    /// confuse "corrupt `E`" with "not an `E`" has to ask here.
    fn try_decode<E: Exn + ReadXDROwned>(
        &self,
    ) -> Option<Result<E, crate::xdr::ReadError>> {
        self.as_exn().and_then(|e| e.try_decode())
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
/// match token.try_transfer_from(&from, &to, amount) {
///     Ok(()) => { /* ... */ }
///     Err(CallError::Fault(f)) => { /* ... */ }
///     Err(e) => match_exn!(e, {
///         ERC20Error::TooLarge               => { /* ... */ }
///         ERC20PausableError::EnforcedPause  => { /* ... */ }
///         // Overspend: balances and allowances are `NNInt`.
///         LibraryExn::NonNegativeIntOverflow => { /* ... */ }
///         _ => e.re_throw(),
///     }),
/// }
/// ```
///
/// Everything structural — `Ok`, [`Fault`](crate::call::RawCallError::Fault),
/// [`Invalid`](crate::call::RawCallError::Invalid),
/// [`Reentrance`](crate::call::RawCallError::Reentrance), the rest of the
/// pre-call rejections, the encode/decode
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
/// Commas follow `match`: an expression body needs a trailing comma, a block
/// body may take one or leave it off.
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
/// # What reaches the `_` arm
///
/// The `_` arm is not only "some error type I did not name". Five distinct
/// situations are swept into it, and the macro does not distinguish them:
///
/// 1. **No exception at all.** When the scrutinee is a
///    [`CallError`](crate::call::CallError), every variant other than
///    `Exception` — a fault, an invalid call, an argument-encoding or
///    result-decoding failure — carries nothing to decode. These are worth
///    handling as ordinary `match` arms on `CallError` *before* delegating
///    to `match_exn!`, precisely so they do not land here.
/// 2. **An exception of a type no arm names.** The expected case: an error
///    the caller does not know about, or knows about and chose not to
///    handle.
/// 3. **An exception whose selector matches a named type, but whose payload
///    does not decode as it.** A selector is
///    `sha256(major_version :: crate :: type)` and says nothing about
///    payload layout, so this is *not* only forgery or truncation: a callee
///    that adds a variant to an exception enum makes a semver-compatible
///    change that leaves the selector untouched, and an older caller cannot
///    decode the result. It arrives here looking exactly like case 2.
///    [`MatchExn::try_decode`] is the way to tell the two apart.
/// 4. **A decoded exception that matched no arm's pattern.** `A::Y` when
///    only `A::X` has an arm.
/// 5. **A decoded exception whose every candidate arm had a failing
///    guard.** Guards fall through to later arms, and then to here.
///
/// Case 3 is the one to keep in mind, because it is silent and can be
/// reached without anyone doing anything wrong. If confusing "a corrupt `E`"
/// with "not an `E`" would change a decision your contract makes, ask
/// [`try_decode`](MatchExn::try_decode) rather than relying on the arms.
///
/// One near-miss does *not* reach `_`, for symmetry's sake: decoding reads a
/// **prefix**, so a payload with extra trailing bytes decodes as the shorter
/// shape and matches. A callee that appends a field to a struct exception is
/// accepted silently by an older caller, with the added field dropped.
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
    //
    // A block body takes an optional comma, so it needs two rules. NOT one
    // rule with `$(,)? $($rest:tt)*`: when a comma is present the matcher
    // cannot tell whether it belongs to the optional repetition or is the
    // first `tt` of `$rest`, and macro_rules rejects that as a local
    // ambiguity rather than choosing — a hard error at every call site
    // written `Pat => { .. },`. Each rule below is a fixed token sequence,
    // so neither is ambiguous. See `block_body_then_comma_*` in the tests.
    ($lbl:lifetime, $exn:ident,
     $pat:pat if $guard:expr => $body:block , $($rest:tt)*) => {{
        if let ::core::option::Option::Some($pat) = $crate::exn::__try_as($exn)
            && $guard
        {
            break $lbl ($body);
        }
        $crate::__match_exn_arms!($lbl, $exn, $($rest)*)
    }};
    ($lbl:lifetime, $exn:ident,
     $pat:pat if $guard:expr => $body:block $($rest:tt)*) => {{
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
    // Two rules for the optional comma, for the reason given above.
    ($lbl:lifetime, $exn:ident, $pat:pat => $body:block , $($rest:tt)*) => {{
        if let ::core::option::Option::Some($pat) = $crate::exn::__try_as($exn)
        {
            break $lbl ($body);
        }
        $crate::__match_exn_arms!($lbl, $exn, $($rest)*)
    }};
    ($lbl:lifetime, $exn:ident, $pat:pat => $body:block $($rest:tt)*) => {{
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
    use crate::exn::{FaultInfo, LibraryExn};
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
            classify(RawCallError::Fault(FaultInfo::empty()))
                .starts_with("other:")
        );
    }

    #[test]
    fn unrecognised_selector_reaches_the_wildcard() {
        let raw = RawException::new(!0u64, b"whatever");
        assert!(classify(RawCallError::Exception(raw)).starts_with("other:"));
    }

    /// `try_decode` separates the two situations `try_as` reports alike.
    #[test]
    fn try_decode_separates_wrong_selector_from_bad_payload() {
        // Selector belongs to another type.
        let other = err(Beta::Rejected);
        assert!(other.try_decode::<Alpha>().is_none());
        assert_eq!(other.try_as::<Alpha>(), None);

        // Matched and decoded.
        let good = err(Alpha::NotFound);
        assert!(matches!(
            good.try_decode::<Alpha>(),
            Some(Ok(Alpha::NotFound))
        ));

        // Alpha's selector over a payload that is not an Alpha. `try_as`
        // cannot tell this from the first case; `try_decode` can.
        let bad = RawCallError::Exception(RawException::new(
            Alpha::SELECTOR,
            &encode_xdr(&9_999_999i32).unwrap(),
        ));
        assert_eq!(bad.try_as::<Alpha>(), None);
        assert!(matches!(bad.try_decode::<Alpha>(), Some(Err(_))));
    }

    /// The routine skew case behind the docs' warning: a callee adds an enum
    /// variant, which is semver-compatible and leaves the selector alone, so
    /// an older caller receives its own selector over a discriminant it does
    /// not know.
    #[test]
    fn try_decode_reports_an_unknown_variant_under_a_shared_selector() {
        // `AlphaPlus` stands in for a later version of `Alpha`: same
        // selector, one extra variant.
        #[derive(Debug, Clone, PartialEq, ReadXDR, WriteXDR)]
        #[repr(i32)]
        enum AlphaPlus {
            NotFound = 1,
            Big { n: u64 } = 2,
            AddedLater = 3,
        }

        impl Exn for AlphaPlus {
            const SELECTOR: u64 = Alpha::SELECTOR;
        }

        let newer = RawCallError::Exception(RawException::new(
            AlphaPlus::SELECTOR,
            &encode_xdr(&AlphaPlus::AddedLater).unwrap(),
        ));

        // The older caller sees a decode failure, not an unrelated error.
        assert_eq!(newer.try_as::<Alpha>(), None);
        assert!(matches!(newer.try_decode::<Alpha>(), Some(Err(_))));

        // And `match_exn!` sweeps it into `_`, which is the documented
        // behavior this test pins rather than endorses.
        let landed = match_exn!(newer, {
            Alpha::NotFound => "not-found",
            Alpha::Big { .. } => "big",
            _ => "wildcard",
        });
        assert_eq!(landed, "wildcard");
    }

    /// Decoding reads a prefix, so trailing bytes are accepted rather than
    /// reported. Pins the asymmetry the macro docs call out.
    #[test]
    fn try_decode_accepts_a_payload_with_trailing_bytes() {
        let mut payload = encode_xdr(&Alpha::NotFound).unwrap();
        payload.extend_from_slice(b"appended-later");
        let padded = RawCallError::Exception(RawException::new(
            Alpha::SELECTOR,
            &payload,
        ));

        assert!(matches!(
            padded.try_decode::<Alpha>(),
            Some(Ok(Alpha::NotFound))
        ));
        assert_eq!(padded.try_as::<Alpha>(), Some(Alpha::NotFound));
    }

    /// A failure carrying no exception is `None`, same as a mismatched
    /// selector; `CallError::Exception` is what separates those.
    #[test]
    fn try_decode_is_none_when_no_exception_is_carried() {
        let fault: RawCallError = RawCallError::Fault(FaultInfo::empty());
        assert!(fault.try_decode::<Alpha>().is_none());
        let invalid: RawCallError = RawCallError::Invalid;
        assert!(invalid.try_decode::<Alpha>().is_none());
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

    /// A block body may be *followed* by a comma, the way `match` arms are
    /// usually written. Regression test: collapsing the comma-required and
    /// comma-free block rules into one `$(,)? $($rest:tt)*` matcher turns
    /// this into `error: local ambiguity when calling macro
    /// `__match_exn_arms``, because a present comma could belong either to
    /// the optional repetition or to `$rest`.
    #[test]
    fn block_body_then_comma_plain_arm() {
        let got = match_exn!(err(Beta::Rejected), {
            Alpha::NotFound => { 1u32 },
            Beta::Rejected => { 2 },
            _ => 0,
        });
        assert_eq!(got, 2);
    }

    /// The guarded rule was collapsed the same way, so it needs the same
    /// cover. The guard must also still fall through to a later arm.
    #[test]
    fn block_body_then_comma_guarded_arm() {
        let classify = |e: RawCallError| {
            match_exn!(e, {
                Alpha::Big { n } if n > 100 => { 1u32 },
                Alpha::Big { n } if n > 10 => { 2 },
                Alpha::Big { .. } => { 3 },
                _ => 0,
            })
        };
        assert_eq!(classify(err(Alpha::Big { n: 500 })), 1);
        assert_eq!(classify(err(Alpha::Big { n: 50 })), 2);
        assert_eq!(classify(err(Alpha::Big { n: 5 })), 3);
        assert_eq!(classify(RawCallError::Invalid), 0);
    }

    /// Body shape and trailing comma are independent: every combination has
    /// to parse, mixed freely within one invocation.
    #[test]
    fn body_shapes_and_commas_mix_freely() {
        let got = match_exn!(err(Beta::Rejected), {
            Alpha::NotFound => 1u32,
            Alpha::Big { .. } => { 2 },
            Beta::Rejected => { 3 }
            _ => 0,
        });
        assert_eq!(got, 3);

        // Same arms, with the match falling through to `_` as a block.
        let no_exn: RawCallError = RawCallError::Invalid;
        let fell_through = match_exn!(no_exn, {
            Alpha::NotFound => 1u32,
            Beta::Rejected => { 2 },
            _ => { 0 }
        });
        assert_eq!(fell_through, 0);
    }

    /// A block body on the last typed arm, comma'd, with `_` immediately
    /// after — the shape that appears in a two-arm handler, and the one most
    /// likely to be written by hand.
    #[test]
    fn block_body_then_comma_before_wildcard() {
        let got = match_exn!(err(Alpha::NotFound), {
            Alpha::NotFound => { 7u32 },
            _ => 0,
        });
        assert_eq!(got, 7);
    }
}
