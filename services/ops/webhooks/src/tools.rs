use crate::delivery::{Deliveries, Delivery, Receipt};
use alloc::string::String;
use berm_lang::{Result, serde_json::Value, tool};

/// Receive
///
/// Store a signed event envelope. A repeated source and id returns the original receipt.
#[tool(
    webhook,
    access = "internal",
    budget(cpu = 100000, read = 20000, written = 10, memory = 16)
)]
pub fn receive(source: String, id: String, payload: Value) -> Result<Receipt> {
    Delivery::receive(source, id, payload)
}

/// List events
///
/// List event metadata after a cursor; fetch payloads individually with get.
#[tool(
    query,
    catalog = "webhooks.events",
    budget(cpu = 100000, read = 20000, written = 10, memory = 16)
)]
pub fn events(
    after: Option<u64>,
    pending_only: Option<bool>,
    limit: Option<u64>,
) -> Result<Deliveries> {
    Delivery::events(after.unwrap_or(0), pending_only.unwrap_or(true), limit)
}

/// Read event
///
/// Read one stored event and its payload.
#[tool(
    query,
    catalog = "webhooks.get",
    budget(cpu = 100000, read = 20000, written = 10, memory = 16)
)]
pub fn get(seq: u64) -> Result<Option<Delivery>> {
    Delivery::get(seq)
}

/// Acknowledge event
///
/// Mark an event processed; repeated acknowledgments return false.
#[tool(
    catalog = "webhooks.acknowledge",
    budget(cpu = 100000, read = 20000, written = 10, memory = 16)
)]
pub fn acknowledge(seq: u64) -> Result<bool> {
    Delivery::acknowledge(seq)
}

/// Delete event
///
/// Delete an event and its deduplication record.
#[tool(
    catalog = "webhooks.delete",
    budget(cpu = 100000, read = 20000, written = 10, memory = 16)
)]
pub fn delete(seq: u64) -> Result<bool> {
    Delivery::delete(seq)
}

/// Remove events older than thirty days, acknowledged or not
///
/// Remove events older than thirty days, acknowledged or not.
#[tool(
    cron,
    access = "internal",
    budget(cpu = 100000, read = 20000, written = 10000, memory = 16)
)]
pub fn prune() -> Result<()> {
    Delivery::prune()
}
