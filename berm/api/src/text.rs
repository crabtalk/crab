//! An enum's text form: the name serde gives each of its variants.

use serde::{
    Serialize,
    de::{
        DeserializeOwned,
        value::{Error, StrDeserializer},
    },
};
use std::fmt;

/// Give each enum `Display`, `FromStr` and `TryFrom<String>` in the names its
/// serde derive gives its variants.
#[macro_export]
macro_rules! text_enum {
    ($($ty:ty),+ $(,)?) => {$(
        impl ::std::fmt::Display for $ty {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                $crate::text::write(self, f)
            }
        }

        impl ::std::str::FromStr for $ty {
            type Err = ::std::string::String;

            fn from_str(text: &str) -> ::std::result::Result<Self, Self::Err> {
                $crate::text::read(text)
            }
        }

        impl ::std::convert::TryFrom<::std::string::String> for $ty {
            type Error = ::std::string::String;

            fn try_from(text: ::std::string::String) -> ::std::result::Result<Self, Self::Error> {
                $crate::text::read(&text)
            }
        }
    )+};
}

/// Write `value` as serde names it.
pub fn write<T: Serialize>(value: &T, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    value.serialize(f)
}

/// The value serde names `text`.
pub fn read<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    T::deserialize(StrDeserializer::<Error>::new(text)).map_err(|error| error.to_string())
}
