//! `#[program]` — turns a module of plain functions into a loadable program.
//!
//! The exports an image needs are ceremony: a description the host reads at
//! registration, and dispatch from a tool index back to a function. None of it
//! is the author's problem, so none of it is in their source.
//!
//! `#[derive(Schema)]`, `#[event]` and `#[error_code]` go on types anywhere in
//! the program's crate.

use crate::config::Config;
use proc_macro::TokenStream;
use syn::{DeriveInput, ItemMod, parse_macro_input};

mod config;
mod declare;
mod kit;
mod program;
mod schema;
mod syscalls;
mod utils;

/// Declare the syscalls a program may call, emitting a stub for each.
///
/// ```ignore
/// berm_lang::syscalls! {
///     namespace = "berm";
///     mod fs {
///         /// Read a file whole.
///         fn read(path: &str) -> Vec<u8>;
///     }
/// }
/// ```
///
/// [`hosts!`] takes the same declaration and emits what a host registers under.
/// Each side writes it in its own crate, and drift is caught on the first call:
/// a renamed call hashes to a number nothing is registered for.
#[proc_macro]
pub fn syscalls(input: TokenStream) -> TokenStream {
    parse_macro_input!(input as syscalls::Declaration)
        .expand(syscalls::Side::Guest)
        .into()
}

/// Typed calls, types and events for another program, from its IDL.
///
/// ```ignore
/// berm_lang::declare_program!(hn);
///
/// let lanes = hn::calls::lanes()?;
/// ```
///
/// Reads `idls/hn.json`, the IDL of the program deployed as `hn`, from the crate's
/// directory or the nearest ancestor that has one. A call takes the tool's
/// arguments in the IDL's key order, an optional one as an `Option`.
#[proc_macro]
pub fn declare_program(input: TokenStream) -> TokenStream {
    let name = parse_macro_input!(input as syn::Ident);
    declare::expand(&name)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// The other side of [`syscalls!`]: one constructor per name, taking the
/// implementation and returning the `Syscall` that serves it.
#[proc_macro]
pub fn hosts(input: TokenStream) -> TokenStream {
    parse_macro_input!(input as syscalls::Declaration)
        .expand(syscalls::Side::Host)
        .into()
}

/// Declare a program from a module of tool functions.
///
/// ```ignore
/// #[program]
/// mod tools {
///     use berm_lang::Result;
///
///     /// Greets `name`.
///     #[public]
///     pub fn greet(
///         /// Who to greet.
///         name: String,
///     ) -> Result<String> {
///         Ok(format!("hello, {name}"))
///     }
/// }
/// ```
///
/// Every `pub fn` in the module becomes a tool. Its doc comment is the
/// description the model reads. Its parameters are the properties of its
/// arguments, each a `Schema` type, and a parameter's doc comment is its
/// description. It returns `berm_lang::Result<T>`: `T` is its reply, written as
/// JSON, and `()` replies nothing.
///
/// **A tool is internal unless marked `#[public]`.** An internal tool is only
/// called by the host, for the program's own wakes, crons and Stripe events. A
/// `#[public]` tool can be called by any account the host authenticates and
/// any program allowed to reach this one, with arguments the caller chooses.
/// `berm_lang::caller()` says who called. `#[callers(services)]` limits a
/// public tool to programs, `#[callers(accounts)]` to callers from outside.
/// `#[query]` marks it read-only: the host refuses its writes, logs and wakes.
/// `#[deprecated]` on a tool marks it so in the manifest.
///
/// `#[cron]` marks a tool its host calls when the cron expression its deploy
/// or its owner sets fires. A cron tool takes no parameters and is not
/// `#[public]`.
///
/// `#[stripe]` marks the tool its host calls with each Stripe event its
/// daemon takes for the program: the event object is the argument object, so
/// the tool's parameters are the event's fields it reads. A program has at
/// most one, and it is not `#[public]`.
///
/// `uses = [crabkit_auth]` names crates that expanded [`kit!`]: the manifest
/// carries each kit's declarations and migrations as the kit's own.
#[proc_macro_attribute]
pub fn program(args: TokenStream, item: TokenStream) -> TokenStream {
    let config = parse_macro_input!(args as Config);
    let module = parse_macro_input!(item as ItemMod);
    program::expand(&config, module).into()
}

/// Declare a library crate a kit: what it reaches for and the migrations of
/// the tables it keeps, for each program naming it in `#[program(uses)]`.
///
/// ```ignore
/// berm_lang::kit!(
///     syscalls = { "berm.http.fetch" = ["api.example.com"] },
///     secrets = ["example"],
///     database = "migrations",
/// );
/// ```
///
/// Takes `#[program]`'s `programs`, `syscalls`, `secrets` and `database`, once
/// at the crate's root. The kit is named for its crate.
#[proc_macro]
pub fn kit(input: TokenStream) -> TokenStream {
    kit::expand(&parse_macro_input!(input as config::Declarations)).into()
}

/// A type's JSON Schema, and the serde impls that read and write it.
///
/// A struct with named fields is an object titled by its name: a field's doc
/// comment is its description, and an `Option` field may be absent. An enum
/// of unit variants is a string, each variant in `snake_case`.
#[proc_macro_derive(Schema)]
pub fn derive_schema(input: TokenStream) -> TokenStream {
    schema::derive(&parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// A typed log, emitted with `berm_lang::emit!` under the struct's name and
/// listed in the program's manifest. Its doc comment is what a reader of the
/// logs is told it means; `#[deprecated]` marks it so in the manifest.
#[proc_macro_attribute]
pub fn event(args: TokenStream, item: TokenStream) -> TokenStream {
    let _ = parse_macro_input!(args as syn::parse::Nothing);
    schema::event(parse_macro_input!(item as syn::ItemStruct))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// An enum of a program's failures. Each variant takes `#[msg("…")]`, the
/// message a tool failing with it answers, and converts into
/// `berm_lang::Error`.
#[proc_macro_attribute]
pub fn error_code(args: TokenStream, item: TokenStream) -> TokenStream {
    let _ = parse_macro_input!(args as syn::parse::Nothing);
    schema::error_code(parse_macro_input!(item as syn::ItemEnum))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
