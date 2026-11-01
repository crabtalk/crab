//! A Neon database, reached at its project's SQL-over-HTTP endpoint.

use crate::{Answer, Statement};
use alloc::{format, string::String, vec::Vec};
use berm_lang::{
    Error, Result, http,
    serde::{Deserialize, Serialize, de::DeserializeOwned},
    serde_json,
};
use core::str::FromStr;

const CONNECTION_STRING: &str = "Neon-Connection-String";
const RAW_TEXT_OUTPUT: &str = "Neon-Raw-Text-Output";
const ARRAY_MODE: &str = "Neon-Array-Mode";

/// A client for one Neon database, parsed from its connection string:
///
/// ```ignore
/// let neon: Neon = "postgresql://app:{secret:NEON_PASSWORD}@ep-x.us-east-2.aws.neon.tech/app".parse()?;
/// let titles = neon.rows::<(String,)>("SELECT title FROM items WHERE id = $1", &(42,))?;
/// ```
pub struct Neon {
    /// Sent as `Neon-Connection-String`, `{secret:NEON_PASSWORD}` as written.
    pub connection: String,
    /// Neon's SQL-over-HTTP endpoint for the connection string's host.
    pub endpoint: String,
}

/// A transaction's statements, as Neon's endpoint reads them.
#[derive(Serialize)]
#[serde(crate = "berm_lang::serde")]
struct Queries<'a> {
    queries: &'a [Statement],
}

/// A transaction's answers, one per statement in order.
#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct Results {
    results: Vec<Answer>,
}

/// Neon's error body.
#[derive(Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct Failure {
    message: String,
    /// The Postgres SQLSTATE, empty when the failure is not Postgres'.
    #[serde(default)]
    code: Option<String>,
}

impl FromStr for Neon {
    type Err = Error;

    /// A `postgres://` or `postgresql://` connection string whose host has
    /// more than one label.
    fn from_str(connection: &str) -> Result<Self> {
        let invalid = || Error::from("not a Neon connection string");
        let rest = connection
            .strip_prefix("postgresql://")
            .or_else(|| connection.strip_prefix("postgres://"))
            .ok_or_else(invalid)?;
        let authority = rest.split(['/', '?']).next().unwrap_or_default();
        let host = authority.rsplit('@').next().unwrap_or_default();
        let host = host.split(':').next().unwrap_or_default();
        let (_, rest) = host.split_once('.').ok_or_else(invalid)?;
        if rest.is_empty() {
            return Err(invalid());
        }
        Ok(Self {
            connection: connection.into(),
            endpoint: format!("https://api.{rest}/sql"),
        })
    }
}

impl Neon {
    /// Run `query` with `params` bound to its `$n` placeholders, and answer
    /// its rows, each a tuple of its columns in order.
    pub fn rows<T: DeserializeOwned>(
        &self,
        query: &str,
        params: &impl Serialize,
    ) -> Result<Vec<T>> {
        self.send::<Answer>(&Statement::new(query, params)?)?.rows()
    }

    /// Run `query` with `params` bound to its `$n` placeholders, and answer
    /// the rows it affected.
    pub fn execute(&self, query: &str, params: &impl Serialize) -> Result<u64> {
        Ok(self
            .send::<Answer>(&Statement::new(query, params)?)?
            .row_count
            .unwrap_or_default())
    }

    /// Run `statements` in one transaction in one request, and answer each
    /// one's result in order. One failing rolls back all of them.
    pub fn transaction(&self, statements: &[Statement]) -> Result<Vec<Answer>> {
        Ok(self
            .send::<Results>(&Queries {
                queries: statements,
            })?
            .results)
    }

    fn send<T: DeserializeOwned>(&self, body: &impl Serialize) -> Result<T> {
        let body = serde_json::to_vec(body)
            .map_err(|error| format!("unserializable Neon request: {error}"))?;
        let response = http::fetch(
            "POST",
            &self.endpoint,
            &[
                (CONNECTION_STRING, &self.connection),
                (RAW_TEXT_OUTPUT, "true"),
                (ARRAY_MODE, "true"),
                ("Content-Type", "application/json"),
            ],
            &body,
        )?;
        if !response.is_success() {
            let status = response.status;
            let message = match response.json::<Failure>() {
                Ok(Failure {
                    message,
                    code: Some(code),
                }) if !code.is_empty() => format!("{message} ({code})"),
                Ok(failure) => failure.message,
                Err(_) => String::from_utf8_lossy(&response.body).into_owned(),
            };
            return Err(format!("Neon answered {status}: {message}").into());
        }
        response
            .json()
            .map_err(|error| format!("Neon answered what this kit cannot read: {error}").into())
    }
}
