// Test environment: just alias `core::cell::Cell`.
// Statics will work if placed in `contract_local!{}`
#[cfg(not(any(doc, target_family = "wasm", target_arch = "riscv32")))]
mod _impl {
    pub use core::cell::Cell as ContractCell;
}

// Test environment: alias `core::cell::RefCell`; `contract_local!` places the
// static in thread-local storage and `with_ref` goes through `LocalKey`.
#[cfg(all(
    not(any(doc, target_family = "wasm", target_arch = "riscv32")),
    feature = "std"
))]
mod _ref_impl {
    pub use core::cell::RefCell as ContractRefCell;

    impl<T: 'static> super::ContractLocalRef<T>
        for std::thread::LocalKey<ContractRefCell<T>>
    {
        fn with_ref<R>(
            &'static self,
            f: impl FnOnce(&core::cell::RefCell<T>) -> R,
        ) -> R {
            self.with(f)
        }
    }
}

// Guest environment: wrap `RefCell` so it can live in a plain static.
#[cfg(any(doc, target_family = "wasm", target_arch = "riscv32"))]
mod _ref_impl {
    /// A wrapped [`core::cell::RefCell`] that implements [`Sync`] so it can
    /// be used in statics — the by-reference companion to [`ContractCell`]
    /// for state that must be borrowed in place (e.g. the SDK's fixed
    /// buffers, which would cost a stack copy if moved through `Cell`).
    /// Declare via [`contract_local!`](crate::contract_local); the SDK
    /// accesses it through a crate-private checkout trait that works
    /// identically against the guest static and the test env's
    /// thread-local.
    ///
    /// [`ContractCell`]: super::ContractCell
    pub struct ContractRefCell<T>(core::cell::RefCell<T>);

    // SAFETY: single-threaded guest, no interrupts; the test environment
    // uses the thread-local alias instead.
    unsafe impl<T> Sync for ContractRefCell<T> {}

    impl<T> ContractRefCell<T> {
        pub const fn new(value: T) -> Self {
            Self(core::cell::RefCell::new(value))
        }
    }

    impl<T> super::ContractLocalRef<T> for ContractRefCell<T> {
        fn with_ref<R>(
            &'static self,
            f: impl FnOnce(&core::cell::RefCell<T>) -> R,
        ) -> R {
            f(&self.0)
        }
    }
}

#[allow(unused_imports)]
pub use _ref_impl::*;

/// Uniform access to a [`contract_local!`](crate::contract_local)
/// [`ContractRefCell`] static: implemented by the guest static itself and by
/// the test env's `LocalKey`, so `CELL.with_ref(|c| ...)` works in both.
///
/// Crate-private on purpose: the checkout-or-fresh policy encodes
/// SDK-internal invariants about the scratch cells, and contracts should
/// not build on it.
pub(crate) trait ContractLocalRef<T> {
    fn with_ref<R>(
        &'static self,
        f: impl FnOnce(&core::cell::RefCell<T>) -> R,
    ) -> R;

    /// Run `f` with exclusive access to the cell's contents.
    ///
    /// This is the checkout-or-fresh pattern behind the SDK's per-instance
    /// scratch buffers. Normally the cell is free, so `f` runs in place
    /// against the cached value — no copy, and for a no-alloc fixed buffer,
    /// no stack move either. If the cell is already borrowed up-stack
    /// (re-entrancy: a nested in-process frame in the native mock, or SDK
    /// re-entry while the cell is checked out — e.g. throwing an exception
    /// while the write scratch is borrowed for encoding), `f` instead runs
    /// against a fresh `T` built by `fresh` — correct, just momentarily
    /// un-cached. The fallback is out of line and `cold` so the fresh
    /// (no-alloc: `SCRATCH_MAX`-byte) value doesn't get pre-allocated in
    /// every caller's prologue when the happy path is inlined.
    ///
    /// `f` is called in *each arm*. For a small `f` that just means its body
    /// is emitted twice (a distinct closure type per use means no cross-arm
    /// sharing), which is the cheaper trade — the hot path keeps addressing
    /// the cell's static directly. For a large `f` the two call sites cost
    /// something subtler and much more expensive; see
    /// [`with_mut_or_fresh_shared`], which the call path uses.
    ///
    /// [`with_mut_or_fresh_shared`]: Self::with_mut_or_fresh_shared
    #[inline]
    fn with_mut_or_fresh<R>(
        &'static self,
        fresh: fn() -> T,
        f: impl FnOnce(&mut T) -> R,
    ) -> R {
        self.with_ref(|cell| match cell.try_borrow_mut() {
            Ok(mut guard) => f(&mut *guard),
            Err(_) => fresh_fallback(fresh, f),
        })
    }

    /// [`with_mut_or_fresh`](Self::with_mut_or_fresh) for a large `f`:
    /// identical semantics, but `f` gets *one* call site instead of one per
    /// arm — hence the `Option` fallback slot rather than an `f(..)` in each
    /// arm.
    ///
    /// The point is not to avoid emitting `f` twice. LLVM already declines to
    /// inline a body this big into two call sites: with the ordinary checkout
    /// it emits the closure once, out of line, and calls it from both arms.
    /// The point is that a *single* call site makes the body **inlinable into
    /// the caller**, and once inlined the entrypoint id, gas limit, flags and
    /// address pointer const-fold straight into the syscall setup instead of
    /// being loaded from a materialized closure. Measured on `fibonacci_call`
    /// (4 call forms): 4556 bytes of call bodies out of line, 3164 inlined.
    ///
    /// So the `Option<T>` slot is the price of inlinability. Only the cold arm
    /// ever fills it, but it sits in the frame of everyone who inlines this,
    /// which for a no-alloc call form is a `SCRATCH_MAX`-sized hole on the hot
    /// path (measured: ~4.4 KiB, against a 24 KiB guest stack). Nothing
    /// *copies* through it — `T`'s buffer is `MaybeUninit`, `None` writes only
    /// a discriminant, and `fresh()`'s return is constructed in place via its
    /// sret slot — so the cost is the frame, not a `memset`/`memcpy`.
    ///
    /// Routing both arms through a shared `#[inline(never)]` trampoline keeps
    /// the fresh `T` in the cold frame and is equally free of duplication, but
    /// it pins the body out of line — i.e. it reconstructs exactly the layout
    /// the ordinary checkout already produces, and measured ~3.4 KiB worse
    /// across the contract suite. Inlinability, not deduplication, is what is
    /// being bought here.
    ///
    /// Worth it only for a big `f`: in this SDK exactly one closure clears the
    /// bar — `with_call_scratch_shared`'s, an entire cross-contract call,
    /// re-instantiated per call form per contract. The small encode closures
    /// (events, storage values, returns) go the other way and use
    /// `with_mut_or_fresh`.
    #[inline]
    fn with_mut_or_fresh_shared<R>(
        &'static self,
        fresh: fn() -> T,
        f: impl FnOnce(&mut T) -> R,
    ) -> R {
        self.with_ref(|cell| {
            let mut fallback: Option<T> = None;
            let mut borrowed = cell.try_borrow_mut().ok();

            let value = match borrowed.as_deref_mut() {
                Some(cached) => cached,
                // Cold: the cell is checked out up-stack (re-entrancy).
                None => fallback.insert(fresh()),
            };

            f(value)
        })
    }
}

/// Re-entrant-checkout fallback for [`ContractLocalRef::with_mut_or_fresh`].
/// Out of line and `cold` so neither a fresh `T`'s construction nor the second
/// copy of `f` lands in every caller's inlined happy path.
#[cold]
#[inline(never)]
fn fresh_fallback<T, R>(fresh: fn() -> T, f: impl FnOnce(&mut T) -> R) -> R {
    let mut value = fresh();
    f(&mut value)
}

#[cfg(all(
    test,
    not(any(target_family = "wasm", target_arch = "riscv32")),
    feature = "std"
))]
mod tests {
    use super::*;

    crate::contract_local! {
        static CELL: ContractRefCell<u32> = const { ContractRefCell::new(1) };
    }

    crate::contract_local! {
        static SHARED_CELL: ContractRefCell<u32> =
            const { ContractRefCell::new(1) };
    }

    /// A re-entrant checkout (e.g. a throw while the write scratch is
    /// borrowed for encoding, or the native mock's nested in-process
    /// frames) must get a fresh transient value, and the cached value must
    /// survive it.
    #[test]
    fn reentrant_checkout_falls_back_to_fresh() {
        let inner_saw = CELL.with_mut_or_fresh(
            || 42,
            |outer| {
                *outer = 7;
                CELL.with_mut_or_fresh(|| 42, |inner| *inner)
            },
        );
        assert_eq!(inner_saw, 42);

        // The cached value survives; the fresh one was transient.
        CELL.with_mut_or_fresh(|| 42, |v| assert_eq!(*v, 7));
    }

    /// The shared-body checkout differs only in how `f` is emitted, so it
    /// owes the same semantics: the re-entrant arm gets a fresh value (via
    /// the `Option` fallback slot rather than a second copy of `f`), and the
    /// cached value survives untouched.
    #[test]
    fn reentrant_shared_checkout_falls_back_to_fresh() {
        let inner_saw = SHARED_CELL.with_mut_or_fresh_shared(
            || 42,
            |outer| {
                *outer = 7;
                SHARED_CELL.with_mut_or_fresh_shared(|| 42, |inner| *inner)
            },
        );
        assert_eq!(inner_saw, 42);

        SHARED_CELL.with_mut_or_fresh_shared(|| 42, |v| assert_eq!(*v, 7));
    }
}

// Guest environment: wrap
#[cfg(any(doc, target_family = "wasm", target_arch = "riscv32"))]
mod _impl {
    // All we need to provide here is interior mutability. By also declaring
    // our wrapper thread-safe, we can use it in a static.

    use core::ops::Deref;

    /// A wrapped [`core::cell::Cell`] that implements [`Sync`] so it can be used
    /// in statics.  Methods of the inner `Cell` are available via this wrapper's [`Deref`]
    /// implementation.  The wrapper adds no costs to `Cell`.
    ///
    /// Note: For static storage that works in the test environment,
    /// use the [`contract_local!`](crate::contract_local) macro, e.g.:
    ///
    ///     contract_local! {
    ///         static VAR: ContractCell<u32> = const { ContractCell::new(0) };
    ///     }
    ///
    /// (This macro is a no-op in the guest environment.)
    ///
    pub struct ContractCell<T: ?Sized>(core::cell::Cell<T>);

    // SAFETY: We are single-threaded and don't get interrupts
    unsafe impl<T: ?Sized> Sync for ContractCell<T> {}

    // Override * to remove the `Cell` wrapper, leaving
    // the `core::cell::Cell`
    impl<T: ?Sized> Deref for ContractCell<T> {
        type Target = core::cell::Cell<T>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<T> ContractCell<T> {
        pub const fn new(value: T) -> Self {
            Self(core::cell::Cell::new(value))
        }
    }
}

pub use _impl::*;
