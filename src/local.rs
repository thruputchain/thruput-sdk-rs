// For the guest environment, `contract_local!{}` can just be stripped
#[cfg(any(doc, target_family = "wasm"))]
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

        std::thread_local! {
            $( $(#[$attr])* $vis static $name: $t = $init; )*
        }

    }
}
