#[cfg_attr(feature = "debugging", track_caller)]
pub fn ensure<E>(pred: bool, err: E)
where
    E: crate::Exn,
{
    if !pred {
        crate::throw(err)
    }
}

#[macro_export]
macro_rules! hash_ident {
    { $vis:vis static $name:ident: SHA256Hash; } => {
        $vis static $name: ::sdk::SHA256Hash =
            ::sdk::SHA256Hash::new(::sdk::sha256_array_ident!($name));
    };
    { $vis:vis static $name:ident: ContractAddress; } => {
        $vis static $name: ::sdk::ContractAddress =
            ::sdk::ContractAddress::new(::sdk::sha256_array_ident!($name));
    };
    { $vis:vis static $name:ident: Address; } => {
        $vis static $name: ::sdk::ContractAddress =
            ::sdk::ContractAddress::new(::sdk::sha256_array_ident!($name));
    };
}
