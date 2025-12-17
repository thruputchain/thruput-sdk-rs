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
