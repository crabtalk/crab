//! Serde helpers shared by the wire types.

/// Give an enum its text form: `AsRef<str>` and `Display` to write it,
/// `FromStr` and `TryFrom<String>` to read it. The literals are the enum's
/// vocabulary on the wire and in the database, and `#[serde(rename_all = …)]`
/// on the enum must spell them the same way.
macro_rules! text_enum {
    ($ty:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        impl AsRef<str> for $ty {
            fn as_ref(&self) -> &str {
                match self { $(Self::$variant => $text),+ }
            }
        }

        impl std::fmt::Display for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_ref())
            }
        }

        impl std::str::FromStr for $ty {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($text => Ok(Self::$variant),)+
                    other => Err(format!(concat!("unknown ", stringify!($ty), ": {}"), other)),
                }
            }
        }

        impl TryFrom<String> for $ty {
            type Error = String;

            fn try_from(s: String) -> Result<Self, Self::Error> {
                s.parse()
            }
        }
    };
}
pub(crate) use text_enum;
