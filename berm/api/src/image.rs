//! A service's wasm module, and the custom sections its manifest is read from.

use anyhow::{Context, Result, bail};
use object::{Object, ObjectSection};

/// A wasm module's bytes.
pub struct Image<'a>(pub &'a [u8]);

impl<'a> Image<'a> {
    /// The payload of its custom section `name`, `None` when it has none.
    pub fn section(&self, name: &str) -> Result<Option<&'a [u8]>> {
        let file = object::File::parse(self.0).context("service is not a readable image")?;
        let Some(section) = file.section_by_name(name) else {
            return Ok(None);
        };
        let data = section
            .data()
            .with_context(|| format!("service's {name} section is unreadable"))?;
        Ok(Some(data))
    }

    /// Its bytes with `payload` appended as the custom section `name`.
    /// Refused when it holds a section of that name already.
    pub fn with_section(&self, name: &str, payload: &[u8]) -> Result<Vec<u8>> {
        if self.section(name)?.is_some() {
            bail!("service already holds a {name} section");
        }
        let mut body = Vec::with_capacity(name.len() + payload.len() + 5);
        leb128(&mut body, name.len());
        body.extend_from_slice(name.as_bytes());
        body.extend_from_slice(payload);

        let mut image = Vec::with_capacity(self.0.len() + body.len() + 6);
        image.extend_from_slice(self.0);
        // Section id 0: a custom section.
        image.push(0);
        leb128(&mut image, body.len());
        image.extend_from_slice(&body);
        Ok(image)
    }
}

/// Append `value` as an unsigned LEB128.
fn leb128(into: &mut Vec<u8>, mut value: usize) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            into.push(byte);
            return;
        }
        into.push(byte | 0x80);
    }
}
