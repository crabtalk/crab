use berm_api::ToolSpec;
use serde_json::{Map, Value};
use syn::{Attribute, Expr, ExprLit, Lit, meta::ParseNestedMeta};

#[derive(Default)]
pub(crate) struct Declaration(pub Map<String, Value>);

impl Declaration {
    pub fn parse(&mut self, meta: ParseNestedMeta, workflow: bool) -> syn::Result<()> {
        let key = meta
            .path
            .get_ident()
            .map(ToString::to_string)
            .unwrap_or_default();
        if self.0.contains_key(&key) {
            return Err(meta.error("duplicate declaration"));
        }
        let value = match key.as_str() {
            "access" | "catalog" | "example" if key != "catalog" || !workflow => Value::from(meta.value()?.parse::<syn::LitStr>()?.value()),
            "price" | "max_cost" | "checkpoint_bytes" if workflow || key == "price" => Value::from(meta.value()?.parse::<syn::LitInt>()?.base10_parse::<u64>()?),
            "idempotency" | "retain" if workflow == (key == "retain") => {
                let literal = meta.value()?.parse::<syn::LitStr>()?;
                let duration = literal.value().parse::<humantime::Duration>().map_err(|error| syn::Error::new(literal.span(), error))?;
                let millis = u64::try_from(duration.as_millis()).map_err(|error| syn::Error::new(literal.span(), error))?;
                if millis == 0 { return Err(meta.error("a duration is at least one millisecond")); }
                if key == "idempotency" { return self.insert("idempotency_ms", Value::from(millis), &meta); }
                Value::from(duration.to_string())
            }
            "budget" => {
                let mut fields = Map::new();
                meta.parse_nested_meta(|field| {
                    let name = field.path.get_ident().map(ToString::to_string).unwrap_or_default();
                    if !["cpu", "read", "written", "memory"].contains(&name.as_str()) || fields.contains_key(&name) { return Err(field.error("expected a unique CPU, read, written or memory budget")); }
                    fields.insert(name, Value::from(field.value()?.parse::<syn::LitInt>()?.base10_parse::<u64>()?));
                    Ok(())
                })?;
                Value::Object(fields)
            }
            _ => return Err(meta.error("unknown declaration; use access, price, budget, catalog, idempotency or example on a tool, or access, price, budget, max_cost, checkpoint_bytes, retain or example on a workflow")),
        };
        self.insert(&key, value, &meta)
    }

    fn insert(&mut self, key: &str, value: Value, meta: &ParseNestedMeta) -> syn::Result<()> {
        if self.0.insert(key.into(), value).is_some() {
            return Err(meta.error("duplicate declaration"));
        }
        Ok(())
    }
}

pub(crate) fn docs(attrs: &[Attribute], at: impl quote::ToTokens) -> syn::Result<(String, String)> {
    let lines: Vec<_> = attrs
        .iter()
        .filter_map(|attr| {
            if attr.path().is_ident("doc")
                && let syn::Meta::NameValue(pair) = &attr.meta
                && let Expr::Lit(ExprLit {
                    lit: Lit::Str(line),
                    ..
                }) = &pair.value
            {
                Some(line.value().trim().to_owned())
            } else {
                None
            }
        })
        .collect();
    let title = lines.first().cloned().unwrap_or_default();
    ToolSpec::check_title(&title).map_err(|error| syn::Error::new_spanned(at, error))?;
    Ok((title, lines[1..].join(" ").trim().to_owned()))
}
