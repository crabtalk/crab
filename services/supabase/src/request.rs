//! Requests to the project as the key in the secret `SUPABASE_KEY`, and the
//! JSON they carry.

use alloc::{format, string::String, vec::Vec};
use berm_lang::{
    Result, http,
    serde::{Deserialize, Serialize, de::DeserializeOwned},
    serde_json,
};

const APIKEY: &str = "{secret:SUPABASE_KEY}";
const AUTHORIZATION: &str = "Bearer {secret:SUPABASE_KEY}";

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
pub fn send(method: &str, url: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<Vec<u8>> {
    let mut all = Vec::from([("apikey", APIKEY), ("Authorization", AUTHORIZATION)]);
    all.extend_from_slice(headers);
    let response = http::fetch(method, url, &all, body)?;
    if response.is_success() {
        return Ok(response.body);
    }
    let status = response.status;
    Err(match response.json::<Failure>() {
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

pub fn json(value: &impl Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|_| String::from("Supabase was sent what is not JSON").into())
}

pub fn read<T: DeserializeOwned>(body: &[u8]) -> Result<T> {
    serde_json::from_slice(body)
        .map_err(|_| String::from("Supabase answered what this kit cannot read").into())
}
