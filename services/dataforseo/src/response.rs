use alloc::format;
use berm_lang::{Result, serde_json::Value};

const COMPLETE: u64 = 20000;
const CREATED: u64 = 20100;
const PARTIAL: u64 = 40106;

pub(crate) fn check(response: Value) -> Result<Value> {
    if !matches!(response["status_code"].as_u64(), Some(COMPLETE | CREATED)) {
        return Err(format!("DataForSEO response failure: {response}").into());
    }
    let tasks = response["tasks"]
        .as_array()
        .ok_or("DataForSEO response has no task list")?;
    if tasks.iter().any(|task| {
        !matches!(
            task["status_code"].as_u64(),
            Some(COMPLETE | CREATED | PARTIAL)
        )
    }) {
        return Err(format!("DataForSEO task failure: {response}").into());
    }
    Ok(response)
}
