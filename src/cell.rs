// Test environment: just alias `core::cell::Cell`.
// Statics will work if placed in `contract_local!{}`
#[cfg(not(any(doc, target_family = "wasm")))]
mod _impl {
    pub use core::cell::Cell as ContractCell;
}

// Guest environment: wrap
#[cfg(any(doc, target_family = "wasm"))]
mod _impl {
    // All we need to provide here is interior mutability. By also declaring
    // our wrapper thread-safe, we can use it in a static.

    use core::ops::Deref;

    /// Note: For static storage that works for guests and for tests,
    /// use, e.g.:
    ///
    ///     contract_local! {
    ///         static VAR: ContractCell<u32> = const { ContractCell::new(0) };
    ///     }
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
