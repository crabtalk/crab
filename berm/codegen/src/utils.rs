use syn::{Expr, ExprLit, Lit};

/// Join a doc comment into one line — the description is a JSON string.
pub(crate) fn docs(attrs: &[syn::Attribute]) -> String {
    let mut description = String::new();
    for attribute in attrs {
        if attribute.path().is_ident("doc")
            && let syn::Meta::NameValue(pair) = &attribute.meta
            && let Expr::Lit(ExprLit {
                lit: Lit::Str(line),
                ..
            }) = &pair.value
        {
            if !description.is_empty() {
                description.push(' ');
            }
            description.push_str(line.value().trim());
        }
    }
    description.trim().to_owned()
}

/// Minimal JSON string escaping — doc comments are prose and can carry
/// anything, and a stray quote would produce a description the host cannot
/// parse at all.
pub(crate) fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if (c as u32) < 0x20 => escaped.push_str(&format!("\\u{:04x}", c as u32)),
            c => escaped.push(c),
        }
    }
    escaped
}
