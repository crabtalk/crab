//! The arguments of `#[program(...)]` and `kit!(...)`.

use crate::utils;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Token, parse::Parse};

/// Default size of the argument buffer, and the most a result may be, in
/// bytes.
///
/// The buffer is paid on every invocation, not once: it lives in `.bss`, which
/// the host zeroes each time it instantiates the guest. Measured against the
/// reference guest, 64 KiB costs a few microseconds per call over 16 KiB, and
/// 4 KiB buys nothing back. `buffer = N` overrides it for a program that needs
/// room.
const DEFAULT_BUFFER: usize = 16 * 1024;

pub(crate) struct Config {
    pub(crate) buffer: usize,
    pub(crate) readme: String,
    /// Kept only to re-emit as an `include_str!`, so editing the file
    /// rebuilds the program.
    pub(crate) readme_file: Option<syn::LitStr>,
    /// The kits whose declarations and migrations the manifest carries, each
    /// the path of a crate `kit!` expanded in.
    pub(crate) uses: Vec<syn::Path>,
    pub(crate) declarations: Declarations,
}

/// What a program or a kit reaches for once it runs, and the database it
/// keeps.
#[derive(Default)]
pub(crate) struct Declarations {
    /// Programs it calls by name, as the author declares them.
    pub(crate) programs: Vec<String>,
    /// Syscalls it makes beyond the ABI's own, each beside what it reaches
    /// through that call.
    pub(crate) syscalls: Vec<(String, Vec<String>)>,
    /// Secrets it names in `{secret:NAME}`.
    pub(crate) secrets: Vec<String>,
    /// The database's migrations directory, relative to the crate, and each
    /// `*.sql` file in it by name.
    pub(crate) database: Option<(syn::LitStr, Vec<(String, String)>)>,
}

/// The arguments of `kit!(...)`.
pub(crate) struct Kit {
    pub(crate) declarations: Declarations,
}

/// The file at `path`, relative to the crate, for argument `key`.
fn read(key: &syn::Ident, path: &syn::LitStr) -> syn::Result<String> {
    let root = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| syn::Error::new(key.span(), format!("{key} needs CARGO_MANIFEST_DIR")))?;
    let full = std::path::Path::new(&root).join(path.value());
    std::fs::read_to_string(&full)
        .map_err(|e| syn::Error::new(path.span(), format!("{}: {e}", full.display())))
}

impl Declarations {
    /// Read the value of `key` when it is a declaration, answering whether it
    /// was one.
    fn parse_key(&mut self, key: &syn::Ident, input: syn::parse::ParseStream) -> syn::Result<bool> {
        match key.to_string().as_str() {
            // Declared, never inferred: a target the program computes at
            // the call is invisible to anything reading the image, so a
            // scan of this crate would report a list that is quietly short.
            // A directory of `*.sql` migrations, applied in name order.
            // Read here for the same reason `readme` is.
            "database" => {
                let path: syn::LitStr = input.parse()?;
                let root = std::env::var("CARGO_MANIFEST_DIR").map_err(|_| {
                    syn::Error::new(key.span(), "database needs CARGO_MANIFEST_DIR")
                })?;
                let dir = std::path::Path::new(&root).join(path.value());
                let unreadable = |e: std::io::Error| {
                    syn::Error::new(path.span(), format!("{}: {e}", dir.display()))
                };
                let mut migrations = Vec::new();
                for entry in std::fs::read_dir(&dir).map_err(unreadable)? {
                    let file = entry.map_err(unreadable)?.path();
                    if file.extension().is_none_or(|extension| extension != "sql") {
                        continue;
                    }
                    let name = file
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or_else(|| {
                            syn::Error::new(path.span(), "a migration name is not UTF-8")
                        })?
                        .to_owned();
                    let sql = std::fs::read_to_string(&file).map_err(unreadable)?;
                    migrations.push((name, sql));
                }
                migrations.sort();
                self.database = Some((path, migrations));
            }
            "programs" => self.programs = strings(input)?,
            "secrets" => self.secrets = strings(input)?,
            // `{ "berm.get", "berm.http.fetch" = ["example.com"] }`: each
            // call, and what it reaches through that call.
            "syscalls" => {
                let entries;
                syn::braced!(entries in input);
                while !entries.is_empty() {
                    let name: syn::LitStr = entries.parse()?;
                    let reached = match entries.peek(Token![=]) {
                        true => {
                            entries.parse::<Token![=]>()?;
                            strings(&entries)?
                        }
                        false => Vec::new(),
                    };
                    if self.syscalls.iter().any(|(seen, _)| *seen == name.value()) {
                        return Err(syn::Error::new(
                            name.span(),
                            "syscall declared twice; list everything it reaches in one entry",
                        ));
                    }
                    self.syscalls.push((name.value(), reached));
                    if !entries.is_empty() {
                        entries.parse::<Token![,]>()?;
                    }
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Done reading: one keeping a database calls `berm.sql`, and says so like
    /// any other syscall.
    fn finish(mut self) -> Self {
        if self.database.is_some() && !self.syscalls.iter().any(|(name, _)| name == "berm.sql") {
            self.syscalls.push(("berm.sql".to_owned(), Vec::new()));
        }
        self
    }

    /// The manifest's `deps` object.
    pub(crate) fn deps(&self) -> String {
        let list = |items: &[String]| {
            items
                .iter()
                .map(|item| format!(r#""{}""#, utils::escape(item)))
                .collect::<Vec<_>>()
                .join(",")
        };
        let syscalls = self
            .syscalls
            .iter()
            .map(|(name, reached)| format!(r#""{}":[{}]"#, utils::escape(name), list(reached)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"programs":[{}],"syscalls":{{{syscalls}}},"secrets":[{}]}}"#,
            list(&self.programs),
            list(&self.secrets),
        )
    }

    /// `,"database":{...}` when it keeps a database, else nothing.
    pub(crate) fn database(&self) -> String {
        let Some((_, migrations)) = &self.database else {
            return String::new();
        };
        format!(
            r#","database":{{"migrations":[{}]}}"#,
            migrations
                .iter()
                .map(|(name, sql)| format!(
                    r#"{{"name":"{}","sql":"{}"}}"#,
                    utils::escape(name),
                    utils::escape(sql)
                ))
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    /// An `include_str!` of each migration the macro read, which cargo cannot
    /// see, so editing one rebuilds the crate.
    pub(crate) fn rebuild(&self) -> TokenStream {
        let includes = self.database.iter().flat_map(|(dir, migrations)| {
            migrations.iter().map(move |(name, _)| {
                let path = format!("{}/{name}", dir.value());
                quote! {
                    const _: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", #path));
                }
            })
        });
        quote!(#(#includes)*)
    }
}

impl Parse for Config {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut buffer = DEFAULT_BUFFER;
        let mut readme = String::new();
        let mut readme_file = None;
        let mut uses: Vec<syn::Path> = Vec::new();
        let mut declarations = Declarations::default();

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "buffer" => {
                    let size: syn::LitInt = input.parse()?;
                    buffer = size.base10_parse()?;
                }
                // A path rather than the text, because a README runs to
                // pages and `include_str!` cannot help here: a proc macro
                // sees the unexpanded call, never the file. So the macro
                // reads it, and the expansion carries an `include_str!` of
                // its own purely so cargo rebuilds when the file changes.
                "readme" => {
                    let path: syn::LitStr = input.parse()?;
                    readme = read(&key, &path)?;
                    readme_file = Some(path);
                }
                "uses" => {
                    let items;
                    syn::bracketed!(items in input);
                    for path in items.parse_terminated(syn::Path::parse_mod_style, Token![,])? {
                        let spelled = quote!(#path).to_string();
                        if uses.iter().any(|seen| quote!(#seen).to_string() == spelled) {
                            return Err(syn::Error::new_spanned(path, "kit used twice"));
                        }
                        uses.push(path);
                    }
                }
                _ if declarations.parse_key(&key, input)? => {}
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "unknown argument: {other} (expected `buffer`, `readme`, `uses`, `database`, `programs`, `syscalls` or `secrets`)"
                        ),
                    ));
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Config {
            buffer,
            readme,
            readme_file,
            uses,
            declarations: declarations.finish(),
        })
    }
}

impl Parse for Kit {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut declarations = Declarations::default();
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            if !declarations.parse_key(&key, input)? {
                return Err(syn::Error::new(
                    key.span(),
                    format!(
                        "unknown argument: {key} (expected `database`, `programs`, `syscalls` or `secrets`)"
                    ),
                ));
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Kit {
            declarations: declarations.finish(),
        })
    }
}

/// A bracketed list of string literals.
fn strings(input: syn::parse::ParseStream) -> syn::Result<Vec<String>> {
    let items;
    syn::bracketed!(items in input);
    Ok(items
        .parse_terminated(<syn::LitStr as Parse>::parse, Token![,])?
        .iter()
        .map(syn::LitStr::value)
        .collect())
}
