//! Supabase over HTTPS through `berm.http.fetch`: PostgREST tables and
//! functions, and Storage objects, as the key in the secret `SUPABASE_KEY`.
//!
//! The kit declares no host. A program using it declares its project's host
//! under `berm.http.fetch`, e.g. `"berm.http.fetch" = ["abcd.supabase.co"]`.

#![no_std]

extern crate alloc;

use alloc::{format, string::String, vec::Vec};
use berm_lang::{
    Result, http,
    serde::{Deserialize, Serialize, de::DeserializeOwned},
    serde_json,
};

mod rest;
mod storage;

pub use rest::Query;
pub use storage::Bucket;

berm_lang::kit!(secrets = ["SUPABASE_KEY"]);

const APIKEY: &str = "{secret:SUPABASE_KEY}";
const AUTHORIZATION: &str = "Bearer {secret:SUPABASE_KEY}";
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
        read(&send(
            "POST",
            &url,
            &[("Content-Type", "application/json")],
            &json(args)?,
        )?)
    }

    /// The Storage bucket `bucket`.
    pub fn storage(&self, bucket: &str) -> Bucket {
        Bucket::new(format!("{}{STORAGE}/{}", self.url, http::encode(bucket)))
    }
}

/// An error body from PostgREST (`message`, `code`) or Storage (`message`,
/// `error`).
#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct Failure {
    #[serde(default)]
    message: String,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// The body of a 2xx answer to the request.
fn send(method: &str, url: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<Vec<u8>> {
    let mut all = Vec::from([("apikey", APIKEY), ("Authorization", AUTHORIZATION)]);
    all.extend_from_slice(headers);
    let response = http::fetch(method, url, &all, body)?;
    if (200..300).contains(&response.status) {
        return Ok(response.body);
    }
    let status = response.status;
    Err(match serde_json::from_slice::<Failure>(&response.body) {
        Ok(Failure {
            message,
            code,
            error,
        }) => match code.or(error) {
            Some(code) => format!("Supabase answered {status}: {message} ({code})"),
            None => format!("Supabase answered {status}: {message}"),
        },
        Err(_) => format!("Supabase answered {status}"),
    }
    .into())
}

fn json(value: &impl Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|_| String::from("Supabase was sent what is not JSON").into())
}

fn read<T: DeserializeOwned>(body: &[u8]) -> Result<T> {
    serde_json::from_slice(body)
        .map_err(|_| String::from("Supabase answered what this kit cannot read").into())
}
