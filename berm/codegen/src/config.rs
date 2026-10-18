//! The arguments of `#[program(...)]`.

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
    /// Programs this one calls by name, as the author declares them.
    pub(crate) programs: Vec<String>,
    /// Syscalls it makes beyond the ABI's own, each beside what it reaches
    /// through that call.
    pub(crate) syscalls: Vec<(String, Vec<String>)>,
    /// Secrets it names in `{secret:NAME}`.
    pub(crate) secrets: Vec<String>,
    /// Kept only to re-emit as an `include_str!`, so editing the file
    /// rebuilds the program.
    pub(crate) readme_file: Option<syn::LitStr>,
    /// The database's migrations directory, relative to the crate, and each
    /// `*.sql` file in it by name.
    pub(crate) database: Option<(syn::LitStr, Vec<(String, String)>)>,
}

/// The file at `path`, relative to the crate, for argument `key`.
fn read(key: &syn::Ident, path: &syn::LitStr) -> syn::Result<String> {
    let root = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| syn::Error::new(key.span(), format!("{key} needs CARGO_MANIFEST_DIR")))?;
    let full = std::path::Path::new(&root).join(path.value());
    std::fs::read_to_string(&full)
        .map_err(|e| syn::Error::new(path.span(), format!("{}: {e}", full.display())))
}

impl Parse for Config {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut buffer = DEFAULT_BUFFER;
        let mut readme = String::new();
        let mut readme_file = None;
        let mut programs = Vec::new();
        let mut syscalls = Vec::new();
        let mut secrets = Vec::new();
        let mut database = None;

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
                    database = Some((path, migrations));
                }
                "programs" => programs = strings(input)?,
                "secrets" => secrets = strings(input)?,
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
                        if syscalls.iter().any(|(seen, _)| *seen == name.value()) {
                            return Err(syn::Error::new(
                                name.span(),
                                "syscall declared twice; list everything it reaches in one entry",
                            ));
                        }
                        syscalls.push((name.value(), reached));
                        if !entries.is_empty() {
                            entries.parse::<Token![,]>()?;
                        }
                    }
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "unknown argument: {other} (expected `buffer`, `readme`, `database`, `programs`, `syscalls` or `secrets`)"
                        ),
                    ));
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        // A program keeping a database calls `berm.sql`, and says so like any
        // other syscall.
        if database.is_some() && !syscalls.iter().any(|(name, _)| name == "berm.sql") {
            syscalls.push(("berm.sql".to_owned(), Vec::new()));
        }

        Ok(Config {
            buffer,
            readme,
            readme_file,
            database,
            programs,
            syscalls,
            secrets,
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
