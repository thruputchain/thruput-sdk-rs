macro_rules! _serialize_unit_impl_base {
    ($ty_name:ident<$($param:ident),*>) => {
        impl<$($param),*> ::serde::Serialize for $ty_name<$($param),*> {
            fn serialize<_S>(
                &self,
                serializer: _S,
            ) -> ::core::result::Result<_S::Ok, _S::Error>
            where
                _S: ::serde::ser::Serializer,
            {
                serializer.serialize_unit()
            }
        }
    };
}

macro_rules! serialize_unit_impl {
    ($ty_name:ident) => {
        _serialize_unit_impl_base!($ty_name<>);
    };
    ($ty_name:ident<$($param:ident),+>) => {
        _serialize_unit_impl_base!($ty_name<$($param),+>);
    };
}

macro_rules! _deserialize_keypath_impl_base {
    ( $ty_name:ident<$($param:ident),*>, $descr:tt ) => {

        mod deserialize_impl {
            use super::$ty_name;
            use crate::storage::{Keypath, KeypathSeed, KeypathSeeded, LoadKeypath};
            use serde::de::{DeserializeSeed, Error};
            use serde::Deserializer;

            impl<$($param),*> KeypathSeeded for $ty_name<$($param),*> {
                type Seed = KeypathSeed<$ty_name<$($param),*>>;

                fn seed(keypath: &Keypath) -> Self::Seed {
                    KeypathSeed::new(keypath)
                }
            }

            impl<'de, $($param),*> DeserializeSeed<'de> for KeypathSeed<$ty_name<$($param),*>> {
                type Value = $ty_name<$($param),*>;

                fn deserialize<_D>(
                    self,
                    _deserializer: _D,
                ) -> Result<Self::Value, _D::Error>
                where
                    _D: Deserializer<'de>,
                {
                    // Consume nothing
                    <Self::Value as LoadKeypath>::load_keypath(&self.keypath).map_err(|e| {
                                _D::Error::custom(e)
                            })
                }
            }
        }

    };
}

macro_rules! deserialize_keypath_impl {
    ( $ty_name:ident, $descr:tt ) => {
        _deserialize_keypath_impl_base!($ty_name<>, $descr);
    };
    ( $ty_name:ident<$($param:ident),+>, $descr:tt ) => {
        _deserialize_keypath_impl_base!($ty_name<$($param),+>, $descr);
    };
}
