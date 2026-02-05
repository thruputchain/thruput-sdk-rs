// For the guest environment, `contract_local!{}` can just be stripped
#[cfg(any(doc, target_family = "wasm"))]
/// Use this macro to wrap static definitions in order to make them work
/// correctly in the test environment.  The statics are placed in thread-local
/// storage so that concurrently-run tests don't bash each other.
///
/// (The macro is a no-op in the guest environment.)
///
/// ## Example
///
///     contract_local! {
///         static VAR: ContractCell<u32> = const { ContractCell::new(0) };
///     }
///
///
#[macro_export]
macro_rules! contract_local {
    { $( $(#[$attr:meta])* $vis:vis static $name:ident: $t:ty = $init:expr; )* } => {

        $( $(#[$attr])* $vis static $name: $t = $init; )*

    }
}

// For the test environment, `contract_local!{}` variables should be thread-local
#[cfg(not(any(doc, target_family = "wasm")))]
#[macro_export]
macro_rules! contract_local {
    { $( $(#[$attr:meta])* $vis:vis static $name:ident: $t:ty = $init:expr; )* } => {

        // This config just silences the compile error when feature "std" is not
        // enabled on a native build.
        #[cfg(feature = "std")]
        std::thread_local! {
            $( $(#[$attr])* $vis static $name: $t = $init; )*
        }

    }
}
