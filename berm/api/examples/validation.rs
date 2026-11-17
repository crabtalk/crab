use anyhow::Result;
use berm_api::schema::{self, Validator};
use serde_json::json;
use std::{hint, time::Instant};

fn main() -> Result<()> {
    let schema = json!({"type":"object","properties":{"id":{"type":"string"},"count":{"type":"integer","minimum":1},"items":{"type":"array","items":{"type":"string"}}},"required":["id","count","items"],"additionalProperties":false});
    let input = json!({"id":"example","count":10,"items":["one","two"]});
    let iterations = 10_000;
    let start = Instant::now();
    for _ in 0..iterations {
        schema::validate(hint::black_box(&schema), hint::black_box(&input))?;
    }
    println!("compile and validate {iterations}: {:?}", start.elapsed());
    let start = Instant::now();
    let validator = Validator::try_from(&schema)?;
    for _ in 0..iterations {
        validator.validate(hint::black_box(&input))?;
    }
    println!(
        "compile once and validate {iterations}: {:?}",
        start.elapsed()
    );
    Ok(())
}
