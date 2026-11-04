use crate::delivery::{Deliveries, Delivery, Receipt};
use alloc::string::String;
use berm_lang::{Result, serde_json::Value, tool};

/// Store a signed event envelope. A repeated source and id returns the original receipt.
#[tool(webhook)]
pub fn receive(source: String, id: String, payload: Value) -> Result<Receipt> {
    Delivery::receive(source, id, payload)
}

/// List event metadata after a cursor; fetch payloads individually with get.
#[tool(query)]
pub fn events(
    after: Option<u64>,
    pending_only: Option<bool>,
    limit: Option<u64>,
) -> Result<Deliveries> {
    Delivery::events(after.unwrap_or(0), pending_only.unwrap_or(true), limit)
}

/// Read one stored event and its payload.
#[tool(query)]
pub fn get(seq: u64) -> Result<Option<Delivery>> {
    Delivery::get(seq)
}

/// Mark an event processed; repeated acknowledgments return false.
#[tool]
pub fn acknowledge(seq: u64) -> Result<bool> {
    Delivery::acknowledge(seq)
}

/// Delete an event and its deduplication record.
#[tool]
pub fn delete(seq: u64) -> Result<bool> {
    Delivery::delete(seq)
}

/// Remove events older than thirty days, acknowledged or not.
#[tool(cron)]
pub fn prune() -> Result<()> {
    Delivery::prune()
}
