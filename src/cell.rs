#[cfg(any(doc, target_family = "wasm"))]
mod _impl {
    use core::ops::Deref;

    /// A wrapper of a standard-library `Cell` for the SDK.
    /// In addition to `Send`, it also implements `Sync`,
    /// allowing it to be used in a static.
    pub struct Cell<T: ?Sized>(core::cell::Cell<T>);

    // SAFETY: We are single-threaded and don't get interrupted
    unsafe impl<T: ?Sized> Sync for Cell<T> {}

    impl<T: ?Sized> Deref for Cell<T> {
        type Target = core::cell::Cell<T>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<T> Cell<T> {
        pub const fn new(value: T) -> Self {
            Self(core::cell::Cell::new(value))
        }
    }
}

#[cfg(not(any(doc, target_family = "wasm")))]
mod _impl {
    use std::sync::Mutex;

    pub struct Cell<T: ?Sized>(Mutex<T>);

    impl<T> Cell<T>
    where
        T: Copy,
    {
        pub const fn new(value: T) -> Self {
            Cell(Mutex::new(value))
        }

        pub fn get(&self) -> T {
            *self.0.lock().unwrap()
        }

        pub fn update(&self, f: impl FnOnce(T) -> T) {
            let mut value = self.0.lock().unwrap();

            let value_new = f(*value);

            *value = value_new;
        }
    }
}

pub use _impl::*;
