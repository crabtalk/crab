#![allow(dead_code)]
use berm_lang::{Branch, Result, Schema, tool};
#[derive(Schema)]
pub struct Input {
    pub count: u64,
}
#[derive(Schema)]
pub struct Yes {
    pub count: u64,
}
#[derive(Schema)]
pub struct No {
    pub count: u64,
}
#[derive(Schema, Branch)]
pub enum Route {
    Yes(Yes),
    No(No),
}
/// Echo a value.
#[tool]
pub fn echo(value: u64) -> Result<u64> {
    Ok(value)
}
