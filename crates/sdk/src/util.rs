#[cfg_attr(not(target_arch = "riscv32"), track_caller)]
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
        $vis static $name: $crate::SHA256Hash =
            $crate::SHA256Hash::new($crate::sha256_array_ident!($name));
    };
    { $vis:vis static $name:ident: ContractAddress; } => {
        $vis static $name: $crate::ContractAddress =
            $crate::ContractAddress::new($crate::sha256_array_ident!($name));
    };
    { $vis:vis static $name:ident: Address; } => {
        $vis static $name: $crate::ContractAddress =
            $crate::ContractAddress::new($crate::sha256_array_ident!($name));
    };
}
