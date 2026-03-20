#[macro_export]
macro_rules! storage_key_named {
    ( $name:literal ) => {
        $crate::StorageKey::new($crate::static_key!($name))
    };
}
