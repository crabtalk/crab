//! How a syscall request is laid out.
//!
//! Every request is a sequence of length-prefixed byte fields, a single-field
//! one included. A reply that carries more than one blob uses the same layout.

use alloc::vec::Vec;
use core::fmt;

/// Append one length-prefixed field.
pub fn field(request: &mut Vec<u8>, bytes: &[u8]) {
    request.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    request.extend_from_slice(bytes);
}

/// Build a request from its fields.
pub fn request(fields: &[&[u8]]) -> Vec<u8> {
    let mut request = Vec::new();
    for bytes in fields {
        field(&mut request, bytes);
    }
    request
}

/// Split framed bytes back into their fields.
pub fn fields(mut framed: &[u8]) -> Result<Vec<&[u8]>, Malformed> {
    let mut fields = Vec::new();
    while !framed.is_empty() {
        let (header, rest) = framed.split_at_checked(4).ok_or(Malformed)?;
        let length = u32::from_le_bytes(header.try_into().map_err(|_| Malformed)?) as usize;
        let (field, rest) = rest.split_at_checked(length).ok_or(Malformed)?;
        fields.push(field);
        framed = rest;
    }
    Ok(fields)
}

/// Framed bytes whose last field's length header or bytes run past their end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Malformed;

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a field runs past the end of the framed bytes")
    }
}

impl core::error::Error for Malformed {}
