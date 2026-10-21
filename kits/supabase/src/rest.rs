//! A PostgREST query on one table: its columns, filters, order and range, and
//! what is done with the rows they select.

use crate::request;
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use berm_lang::{
    Result, http,
    serde::{Serialize, de::DeserializeOwned},
};

const JSON: (&str, &str) = ("Content-Type", "application/json");
const REPRESENTATION: (&str, &str) = ("Prefer", "return=representation");

/// A query on one table, built up and then run by [`Query::get`],
/// [`Query::insert`], [`Query::update`] or [`Query::delete`].
///
/// Values are sent as written, after PostgREST's `operator.`: a value holding
/// `,`, `.`, `:` or `()` inside `in.(…)` or `or=(…)` needs PostgREST's double
/// quotes.
pub struct Query {
    url: String,
    columns: Option<String>,
    params: Vec<(String, String)>,
    order: Vec<String>,
}

impl Query {
    pub(crate) fn new(url: String) -> Self {
        Self {
            url,
            columns: None,
            params: Vec::new(),
            order: Vec::new(),
        }
    }

    /// The columns each answered row carries, in PostgREST's `select` syntax,
    /// e.g. `"id,title"`. Every column when not set.
    pub fn select(mut self, columns: &str) -> Self {
        self.columns = Some(columns.into());
        self
    }

    /// Rows where `column` satisfies the PostgREST operator `operator`
    /// against `value`, e.g. `filter("id", "in", "(1,2)")` or
    /// `filter("done", "not.is", "null")`.
    pub fn filter(mut self, column: &str, operator: &str, value: &str) -> Self {
        self.params
            .push((column.into(), format!("{operator}.{value}")));
        self
    }

    /// Rows where `column` equals `value`.
    pub fn eq(self, column: &str, value: &str) -> Self {
        self.filter(column, "eq", value)
    }

    /// Rows where `column` does not equal `value`.
    pub fn neq(self, column: &str, value: &str) -> Self {
        self.filter(column, "neq", value)
    }

    /// Rows where `column` is greater than `value`.
    pub fn gt(self, column: &str, value: &str) -> Self {
        self.filter(column, "gt", value)
    }

    /// Rows where `column` is at least `value`.
    pub fn gte(self, column: &str, value: &str) -> Self {
        self.filter(column, "gte", value)
    }

    /// Rows where `column` is less than `value`.
    pub fn lt(self, column: &str, value: &str) -> Self {
        self.filter(column, "lt", value)
    }

    /// Rows where `column` is at most `value`.
    pub fn lte(self, column: &str, value: &str) -> Self {
        self.filter(column, "lte", value)
    }

    /// Rows where `column` matches the case-sensitive pattern `pattern`, `*`
    /// or `%` matching any run of characters.
    pub fn like(self, column: &str, pattern: &str) -> Self {
        self.filter(column, "like", pattern)
    }

    /// [`Query::like`], case-insensitive.
    pub fn ilike(self, column: &str, pattern: &str) -> Self {
        self.filter(column, "ilike", pattern)
    }

    /// Rows where `column` is `value`: `null`, `true`, `false` or `unknown`.
    pub fn is(self, column: &str, value: &str) -> Self {
        self.filter(column, "is", value)
    }

    /// Rows ordered by `column`, after any order set before it.
    pub fn order(mut self, column: &str, ascending: bool) -> Self {
        let direction = if ascending { "asc" } else { "desc" };
        self.order.push(format!("{column}.{direction}"));
        self
    }

    /// At most `count` rows.
    pub fn limit(mut self, count: u64) -> Self {
        self.params.push(("limit".into(), count.to_string()));
        self
    }

    /// Rows after the first `count`.
    pub fn offset(mut self, count: u64) -> Self {
        self.params.push(("offset".into(), count.to_string()));
        self
    }

    /// The rows the query selects.
    pub fn get<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        request::read(&request::send("GET", &self.url(true), &[], &[])?)
    }

    /// Inserts `rows`, one object or an array of them, and answers the rows
    /// inserted. Filters, order and range are not sent.
    pub fn insert<T: DeserializeOwned>(&self, rows: &impl Serialize) -> Result<Vec<T>> {
        request::read(&request::send(
            "POST",
            &self.url(false),
            &[JSON, REPRESENTATION],
            &request::json(rows)?,
        )?)
    }

    /// Sets `values`, an object of columns, on the rows the query selects, and
    /// answers them as updated.
    pub fn update<T: DeserializeOwned>(&self, values: &impl Serialize) -> Result<Vec<T>> {
        request::read(&request::send(
            "PATCH",
            &self.url(true),
            &[JSON, REPRESENTATION],
            &request::json(values)?,
        )?)
    }

    /// Deletes the rows the query selects, and answers them.
    pub fn delete<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        request::read(&request::send(
            "DELETE",
            &self.url(true),
            &[REPRESENTATION],
            &[],
        )?)
    }

    /// The table's URL with `select`, and with the filters, order and range
    /// when `filtered`.
    fn url(&self, filtered: bool) -> String {
        let order = self.order.join(",");
        let mut fields: Vec<(&str, &str)> = Vec::new();
        if let Some(columns) = &self.columns {
            fields.push(("select", columns));
        }
        if filtered {
            fields.extend(self.params.iter().map(|(k, v)| (k.as_str(), v.as_str())));
            if !order.is_empty() {
                fields.push(("order", &order));
            }
        }
        let query = http::form(&fields);
        if query.is_empty() {
            self.url.clone()
        } else {
            format!("{}?{query}", self.url)
        }
    }
}
