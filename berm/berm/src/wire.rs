//! Reading a syscall request's fields, framed as
//! [`abi::wire`](crate::abi::wire) frames them.

use anyhow::{Result, bail};

/// Read a field as UTF-8.
pub fn text<'a>(fields: &[&'a [u8]], at: usize, what: &str) -> Result<&'a str> {
    let Some(field) = fields.get(at) else {
        bail!("request has no {what}");
    };
    Ok(std::str::from_utf8(field)?)
}
