//! JSON Schema for the types a tool reads and writes, built at compile time.
//!
//! A type's schema is a [`Shape`] its [`Schema`] impl names as a const, so a
//! tool's argument may be a type from any module or crate. The manifest is
//! rendered from shapes by [`render`] into the image's `.berm.abi` section,
//! which a host reads without running anything.

use alloc::{collections::BTreeMap, string::String, vec::Vec};

/// A type with a JSON Schema.
/// `#[derive(Schema)]` implements it, and the serde impls that read and write
/// the type as that schema says.
pub trait Schema {
    const SHAPE: Shape;
}

/// One JSON Schema. Every string in it is already JSON-escaped.
pub enum Shape {
    /// Any JSON value, including null and nested objects.
    Any,
    /// A complete, valid JSON Schema literal.
    Json(&'static str),
    String,
    Boolean,
    Number,
    Integer,
    /// An integer with `minimum` 0.
    Unsigned,
    Array {
        items: &'static Shape,
    },
    /// An object whose keys are the caller's; only the values have a schema.
    Map {
        values: &'static Shape,
    },
    /// Nullable as a value; omitted from an object's `required` as a field.
    Optional {
        inner: &'static Shape,
    },
    /// `title` empty is an object without one.
    Object {
        title: &'static str,
        fields: &'static [Field],
    },
    /// A string that is one of `values`.
    Enum {
        title: &'static str,
        values: &'static [&'static str],
    },
    /// Externally tagged enum variants, each with an optional payload.
    Union {
        title: &'static str,
        variants: &'static [Variant],
    },
}

pub struct Variant {
    pub name: &'static str,
    pub description: &'static str,
    pub payload: Option<&'static Shape>,
}

/// One property of a [`Shape::Object`].
pub struct Field {
    pub name: &'static str,
    /// A JSON string, its quotes included; empty when the field has none.
    pub description: &'static str,
    pub shape: &'static Shape,
}

/// One run of a manifest: JSON text, or a schema rendered in its place.
pub enum Piece {
    Text(&'static str),
    Shape(&'static Shape),
}

/// The bytes [`render`] writes for `pieces`.
pub const fn len(pieces: &[Piece]) -> usize {
    let mut sink = Sink {
        buffer: &mut [],
        at: 0,
        counting: true,
    };
    pieces_into(&mut sink, pieces);
    sink.at
}

/// `pieces` as one JSON text, `N` being their [`len`].
pub const fn render<const N: usize>(pieces: &[Piece]) -> [u8; N] {
    let mut buffer = [0; N];
    let mut sink = Sink {
        buffer: &mut buffer,
        at: 0,
        counting: false,
    };
    pieces_into(&mut sink, pieces);
    buffer
}

struct Sink<'a> {
    buffer: &'a mut [u8],
    at: usize,
    /// Advance without writing, which is how [`len`] measures.
    counting: bool,
}

impl Sink<'_> {
    const fn put(&mut self, text: &str) {
        let bytes = text.as_bytes();
        if !self.counting {
            let mut at = 0;
            while at < bytes.len() {
                self.buffer[self.at + at] = bytes[at];
                at += 1;
            }
        }
        self.at += bytes.len();
    }

    const fn quoted(&mut self, text: &str) {
        self.put("\"");
        self.put(text);
        self.put("\"");
    }
}

const fn pieces_into(sink: &mut Sink, pieces: &[Piece]) {
    let mut at = 0;
    while at < pieces.len() {
        match &pieces[at] {
            Piece::Text(text) => sink.put(text),
            Piece::Shape(shape) => shape_into(sink, shape, ""),
        }
        at += 1;
    }
}

const fn shape_into(sink: &mut Sink, shape: &Shape, description: &str) {
    match shape {
        Shape::Any => {
            sink.put("{");
            if !description.is_empty() {
                sink.put(r#""description":"#);
                sink.put(description);
            }
            sink.put("}");
            return;
        }
        Shape::Json(json) => {
            if description.is_empty() {
                sink.put(json);
                return;
            }
            sink.put(r#"{"allOf":["#);
            sink.put(json);
            sink.put("]");
        }
        Shape::Optional { inner } => {
            sink.put(r#"{"anyOf":["#);
            shape_into(sink, inner, "");
            sink.put(r#",{"type":"null"}]"#);
        }
        Shape::String => sink.put(r#"{"type":"string""#),
        Shape::Boolean => sink.put(r#"{"type":"boolean""#),
        Shape::Number => sink.put(r#"{"type":"number""#),
        Shape::Integer => sink.put(r#"{"type":"integer""#),
        Shape::Unsigned => sink.put(r#"{"type":"integer","minimum":0"#),
        Shape::Array { items } => {
            sink.put(r#"{"type":"array","items":"#);
            shape_into(sink, items, "");
        }
        Shape::Map { values } => {
            sink.put(r#"{"type":"object","additionalProperties":"#);
            shape_into(sink, values, "");
        }
        Shape::Object { title, fields } => {
            sink.put(r#"{"type":"object""#);
            if !title.is_empty() {
                sink.put(r#","title":"#);
                sink.quoted(title);
            }
            sink.put(r#","properties":{"#);
            let mut at = 0;
            while at < fields.len() {
                if at > 0 {
                    sink.put(",");
                }
                sink.quoted(fields[at].name);
                sink.put(":");
                let shape = match fields[at].shape {
                    Shape::Optional { inner } => inner,
                    shape => shape,
                };
                shape_into(sink, shape, fields[at].description);
                at += 1;
            }
            sink.put(r#"},"required":["#);
            let mut first = true;
            let mut at = 0;
            while at < fields.len() {
                if !matches!(fields[at].shape, Shape::Optional { .. }) {
                    if !first {
                        sink.put(",");
                    }
                    sink.quoted(fields[at].name);
                    first = false;
                }
                at += 1;
            }
            sink.put("]");
        }
        Shape::Enum { title, values } => {
            sink.put(r#"{"type":"string","title":"#);
            sink.quoted(title);
            sink.put(r#","enum":["#);
            let mut at = 0;
            while at < values.len() {
                if at > 0 {
                    sink.put(",");
                }
                sink.quoted(values[at]);
                at += 1;
            }
            sink.put("]");
        }
        Shape::Union { title, variants } => {
            sink.put(r#"{"title":"#);
            sink.quoted(title);
            sink.put(r#","oneOf":["#);
            let mut at = 0;
            while at < variants.len() {
                if at > 0 {
                    sink.put(",");
                }
                let variant = &variants[at];
                match variant.payload {
                    None => {
                        sink.put(r#"{"type":"string","enum":["#);
                        sink.quoted(variant.name);
                        sink.put("]");
                    }
                    Some(payload) => {
                        sink.put(r#"{"type":"object","properties":{"#);
                        sink.quoted(variant.name);
                        sink.put(":");
                        shape_into(sink, payload, "");
                        sink.put(r#"},"required":["#);
                        sink.quoted(variant.name);
                        sink.put(r#"],"additionalProperties":false"#);
                    }
                }
                if !variant.description.is_empty() {
                    sink.put(r#","description":"#);
                    sink.put(variant.description);
                }
                sink.put("}");
                at += 1;
            }
            sink.put("]");
        }
    }
    if !description.is_empty() {
        sink.put(r#","description":"#);
        sink.put(description);
    }
    sink.put("}");
}

macro_rules! leaf {
    ($shape:ident: $($ty:ty),*) => {
        $(impl Schema for $ty {
            const SHAPE: Shape = Shape::$shape;
        })*
    };
}

leaf!(String: String);
leaf!(Boolean: bool);
leaf!(Number: f32, f64);
leaf!(Integer: i8, i16, i32, i64, isize);
leaf!(Unsigned: u8, u16, u32, u64, usize);

impl<T: Schema> Schema for Vec<T> {
    const SHAPE: Shape = Shape::Array { items: &T::SHAPE };
}

impl<T: Schema> Schema for Option<T> {
    const SHAPE: Shape = Shape::Optional { inner: &T::SHAPE };
}

impl<T: Schema> Schema for BTreeMap<String, T> {
    const SHAPE: Shape = Shape::Map { values: &T::SHAPE };
}

impl Schema for crate::serde_json::Value {
    const SHAPE: Shape = Shape::Any;
}
