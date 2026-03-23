//! Macro for compile-time length-checking of VecM constructions.
//!
//! # Examples
//!
//! ```
//!     # #[macro_use] extern crate wallet;
//!     # use xdrs::VecM;
//!     # fn main() {
//!     let _: VecM<u32, 3> = vecM![0, 1];    // Ok!
//!     # }
//! ```
//!
//! ```compile_fail
//!     # #[macro_use] extern crate wallet;
//!     # use xdrs::VecM;
//!     # fn main() {
//!     let _: VecM<u32, 2> = vecM![0, 1, 2]; // Compile error!
//!     # }
//! ```

pub use xdrs::VecM;

struct ValidLen<const N: usize, const M: u32>;

impl<const N: usize, const M: u32> ValidLen<N, M> {
    pub const OK: () = assert!(N < (M as usize),
        "Tried to make a VecM from an array that is too long");
}

#[allow(non_snake_case)]
#[macro_export]
macro_rules! vecM {
    [ $($elt:expr),* ] => {
        $crate::vecM([$($elt),*])
    };
}

#[allow(non_snake_case)]
pub fn vecM<const N: usize, const M: u32, T>(v: [T; N]) -> VecM<T, M> {
    let _ = ValidLen::<N, M>::OK;

    // This should never fail
    Vec::from(v).try_into().unwrap()
}
