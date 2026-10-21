//! Supabase over HTTPS through `berm.http.fetch`: PostgREST tables and
//! functions, and Storage objects, as the key in the secret `SUPABASE_KEY`.
//!
//! The kit declares no host. A program using it declares its project's host
//! under `berm.http.fetch`, e.g. `"berm.http.fetch" = ["abcd.supabase.co"]`.

#![no_std]

extern crate alloc;

use alloc::{format, string::String};
use berm_lang::{
    Result, http,
    serde::{Serialize, de::DeserializeOwned},
};

mod request;
mod rest;
mod storage;

pub use rest::Query;
pub use storage::Bucket;

berm_lang::kit!(secrets = ["SUPABASE_KEY"]);

const REST: &str = "/rest/v1";
const STORAGE: &str = "/storage/v1/object";

/// A Supabase project, by its URL.
pub struct Supabase {
    /// The project's URL, e.g. `https://abcd.supabase.co`, with no trailing `/`.
    pub url: String,
}

impl Supabase {
    /// The project at `url`, e.g. `https://abcd.supabase.co`.
    pub fn new(url: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').into(),
        }
    }

    /// A query on the table or view `table`.
    pub fn from(&self, table: &str) -> Query {
        Query::new(format!("{}{REST}/{}", self.url, http::encode(table)))
    }

    /// Calls the Postgres function `function` with `args`, a JSON object of its
    /// named arguments, and answers what it returned.
    pub fn rpc<T: DeserializeOwned>(&self, function: &str, args: &impl Serialize) -> Result<T> {
        let url = format!("{}{REST}/rpc/{}", self.url, http::encode(function));
        request::read(&request::send(
            "POST",
            &url,
            &[("Content-Type", "application/json")],
            &request::json(args)?,
        )?)
    }

    /// The Storage bucket `bucket`.
    pub fn storage(&self, bucket: &str) -> Bucket {
        Bucket::new(format!("{}{STORAGE}/{}", self.url, http::encode(bucket)))
    }
}
