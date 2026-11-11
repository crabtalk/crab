//! Opaque references to host-managed secret resources.
pub const PREFIX: &str = "secret_";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Id<'a>(pub &'a str);
impl<'a> TryFrom<&'a str> for Id<'a> {
    type Error = &'static str;
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        let suffix = value
            .strip_prefix(PREFIX)
            .ok_or("Invalid secret resource ID")?;
        if suffix.len() != 32
            || !suffix
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("Invalid secret resource ID");
        }
        Ok(Self(value))
    }
}
