use super::keypath::Keypath;
use core::marker::PhantomData;

// Can generate a keypath-containing seed for this type
pub trait KeypathSeeded {
    type Seed;

    fn seed(keypath: &Keypath) -> Self::Seed;
}

pub struct KeypathSeed<T> {
    pub keypath: Keypath,
    pub marker: PhantomData<T>,
}

impl<T> KeypathSeed<T> {
    pub fn new(keypath: &Keypath) -> Self {
        Self {
            keypath: keypath.clone(),
            marker: PhantomData,
        }
    }
}

mod deserialize_impl {
    use super::KeypathSeed;
    use super::{Keypath, KeypathSeeded};
    use serde::de::DeserializeSeed;
    use serde::{Deserialize, Deserializer};

    impl<'de, T> KeypathSeeded for T
    where
        T: Deserialize<'de>,
    {
        type Seed = KeypathSeed<T>;

        fn seed(keypath: &Keypath) -> Self::Seed {
            KeypathSeed::new(keypath)
        }
    }

    impl<'de, T> DeserializeSeed<'de> for KeypathSeed<T>
    where
        T: Deserialize<'de>,
    {
        type Value = T;

        fn deserialize<D>(
            self,
            deserializer: D,
        ) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            <T as Deserialize<'de>>::deserialize(deserializer)
        }
    }
}
