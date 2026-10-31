use syn::{
    Ident, LitBool, LitStr, Macro, Meta, Path, Token, braced,
    parse::{Parse, ParseStream},
};

pub(crate) struct Config {
    pub spec: LitStr,
    pub spec_env: Option<LitStr>,
    pub client: Path,
    pub response: Option<Path>,
    pub base_url: Option<LitStr>,
    pub tools: bool,
    pub tools_cfg: Option<Meta>,
    pub operations: Vec<(Ident, LitStr)>,
}

impl Parse for Config {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let (mut spec, mut client, mut base_url, mut tools, mut operations) =
            (None, None, None, None, None);
        let mut response = None;
        let mut tools_cfg = None;
        let mut spec_env = None;
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "spec" if spec.is_none() => {
                    if input.peek(LitStr) {
                        spec = Some(input.parse()?);
                    } else {
                        let source: Macro = input.parse()?;
                        if !source.path.is_ident("env") {
                            return Err(syn::Error::new(
                                key.span(),
                                "expected a path literal or env!(\"NAME\")",
                            ));
                        }
                        let name: LitStr = source.parse_body()?;
                        let value = std::env::var(name.value())
                            .map_err(|error| syn::Error::new(name.span(), error))?;
                        spec = Some(LitStr::new(&value, name.span()));
                        spec_env = Some(name);
                    }
                }
                "client" if client.is_none() => client = Some(input.parse()?),
                "response" if response.is_none() => response = Some(input.parse()?),
                "base_url" if base_url.is_none() => base_url = Some(input.parse()?),
                "tools" if tools.is_none() => {
                    if input.peek(LitBool) {
                        tools = Some(input.parse::<LitBool>()?.value);
                    } else {
                        let cfg: Meta = input.parse()?;
                        if !cfg.path().is_ident("cfg") || !matches!(cfg, Meta::List(_)) {
                            return Err(syn::Error::new(
                                key.span(),
                                "expected true, false or cfg(...)",
                            ));
                        }
                        tools_cfg = Some(cfg);
                        tools = Some(true);
                    }
                }
                "operations" if operations.is_none() => {
                    let content;
                    braced!(content in input);
                    let mut selected = Vec::new();
                    while !content.is_empty() {
                        selected.push((content.parse()?, {
                            content.parse::<Token![:]>()?;
                            content.parse()?
                        }));
                        if !content.is_empty() {
                            content.parse::<Token![,]>()?;
                        }
                    }
                    operations = Some(selected);
                }
                _ => return Err(syn::Error::new(key.span(), "unknown or duplicate option")),
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        let span = input.span();
        let config = Self {
            spec: spec.ok_or_else(|| syn::Error::new(span, "missing spec"))?,
            spec_env,
            client: client.ok_or_else(|| syn::Error::new(span, "missing client"))?,
            base_url,
            response,
            tools: tools.unwrap_or(true),
            tools_cfg,
            operations: operations.ok_or_else(|| syn::Error::new(span, "missing operations"))?,
        };
        if config.operations.is_empty() {
            return Err(syn::Error::new(span, "select at least one operation"));
        }
        Ok(config)
    }
}
