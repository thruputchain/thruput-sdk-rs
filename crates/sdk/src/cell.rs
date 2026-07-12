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
#[cfg(all(
    any(doc, target_family = "wasm", target_arch = "riscv32"),
    feature = "alloc"
))]
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

#[cfg(feature = "alloc")]
#[allow(unused_imports)]
pub use _ref_impl::*;

/// Uniform access to a [`contract_local!`](crate::contract_local)
/// [`ContractRefCell`] static: implemented by the guest static itself and by
/// the test env's `LocalKey`, so `CELL.with_ref(|c| ...)` works in both.
///
/// Crate-private: contracts should not build on the SDK's internal cells.
///
/// `alloc`-only, because the SDK's one by-reference cell is the `alloc`
/// scratch pool ([`call::with_call_scratch`](crate::call)). The no-alloc guest
/// keeps its scratch on the stack — a `MaybeUninit` buffer is free to build,
/// so there is nothing to cache — and so needs no cell, no borrow flag, and no
/// re-entrancy fallback at all.
#[cfg(feature = "alloc")]
pub(crate) trait ContractLocalRef<T> {
    fn with_ref<R>(
        &'static self,
        f: impl FnOnce(&core::cell::RefCell<T>) -> R,
    ) -> R;
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
