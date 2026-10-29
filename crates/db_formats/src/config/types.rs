//! Value types shared by the generated config structs.
//!
//! protobuf's JSON mapping writes 64-bit integers as strings and enums as value names
//! (or as numbers when the value is not in the schema); these types accept exactly that.

use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

macro_rules! int64 {
    ($name:ident, $inner:ty, $what:literal) => {
        #[doc = concat!("A protobuf ", $what, ": JSON string (protobuf mapping) or JSON number.")]
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub $inner);

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.0.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct V;
                impl Visitor<'_> for V {
                    type Value = $name;
                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        f.write_str(concat!("a ", $what, " as string or number"))
                    }
                    fn visit_str<E: de::Error>(self, v: &str) -> Result<$name, E> {
                        v.parse().map($name).map_err(E::custom)
                    }
                    fn visit_i64<E: de::Error>(self, v: i64) -> Result<$name, E> {
                        <$inner>::try_from(v).map($name).map_err(E::custom)
                    }
                    fn visit_u64<E: de::Error>(self, v: u64) -> Result<$name, E> {
                        <$inner>::try_from(v).map($name).map_err(E::custom)
                    }
                }
                d.deserialize_any(V)
            }
        }
    };
}

int64!(I64, i64, "int64");
int64!(U64, u64, "uint64");

/// Defines a protobuf enum: known variants plus `NotInSchema(i32)` for numbers not in the schema.
/// Deserializes from the value name (unknown names are an error) or from a number.
#[macro_export]
macro_rules! config_enum {
    ($(#[$m:meta])* $name:ident { $($var:ident = $num:literal => $s:literal,)* }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $($var,)*
            /// A number that is not a value of this enum in the recovered schema.
            NotInSchema(i32),
        }

        #[allow(unreachable_patterns)]
        impl $name {
            /// The protobuf number.
            pub fn number(self) -> i32 {
                match self { $(Self::$var => $num,)* Self::NotInSchema(n) => n }
            }
            /// The variant for a protobuf number (`NotInSchema` if the schema has none).
            pub fn from_number(n: i32) -> Self {
                match n { $($num => Self::$var,)* n => Self::NotInSchema(n) }
            }
            /// The protobuf value name, if known.
            pub fn name(self) -> Option<&'static str> {
                match self { $(Self::$var => Some($s),)* Self::NotInSchema(_) => None }
            }
            /// The variant for a protobuf value name.
            pub fn from_name(s: &str) -> Option<Self> {
                match s { $($s => Some(Self::$var),)* _ => None }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::from_number(0)
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                match self.name() {
                    Some(n) => s.serialize_str(n),
                    None => s.serialize_i32(self.number()),
                }
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct V;
                impl ::serde::de::Visitor<'_> for V {
                    type Value = $name;
                    fn expecting(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                        f.write_str(concat!("a ", stringify!($name), " name or number"))
                    }
                    fn visit_str<E: ::serde::de::Error>(self, v: &str) -> Result<$name, E> {
                        $name::from_name(v).ok_or_else(|| E::custom(format!(concat!("unknown ", stringify!($name), " `{}`"), v)))
                    }
                    fn visit_i64<E: ::serde::de::Error>(self, v: i64) -> Result<$name, E> {
                        i32::try_from(v).map($name::from_number).map_err(E::custom)
                    }
                    fn visit_u64<E: ::serde::de::Error>(self, v: u64) -> Result<$name, E> {
                        i32::try_from(v).map($name::from_number).map_err(E::custom)
                    }
                }
                d.deserialize_any(V)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    crate::config_enum! {
        /// Test enum.
        Career {
            None = 0 => "EN_CAREER_NONE",
            Fighter = 1 => "EN_CAREER_FIGHTER",
        }
    }

    #[test]
    fn int64_from_string_or_number() {
        assert_eq!(serde_json::from_str::<I64>("\"-12\"").unwrap(), I64(-12));
        assert_eq!(serde_json::from_str::<I64>("7").unwrap(), I64(7));
        assert_eq!(serde_json::from_str::<U64>("\"18446744073709551615\"").unwrap(), U64(u64::MAX));
        assert!(serde_json::from_str::<U64>("\"-1\"").is_err());
        assert_eq!(serde_json::to_string(&I64(5)).unwrap(), "\"5\"");
    }

    #[test]
    fn enum_from_name_number_and_unknown() {
        assert_eq!(serde_json::from_str::<Career>("\"EN_CAREER_FIGHTER\"").unwrap(), Career::Fighter);
        assert_eq!(serde_json::from_str::<Career>("1").unwrap(), Career::Fighter);
        assert_eq!(serde_json::from_str::<Career>("9").unwrap(), Career::NotInSchema(9));
        assert!(serde_json::from_str::<Career>("\"EN_CAREER_WIZARD\"").is_err());
        assert_eq!(Career::default(), Career::None);
        assert_eq!(serde_json::to_string(&Career::NotInSchema(9)).unwrap(), "9");
        assert_eq!(serde_json::to_string(&Career::Fighter).unwrap(), "\"EN_CAREER_FIGHTER\"");
    }
}
