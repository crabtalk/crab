use crate::types::{Batch, Delivery, Email, Sent};
use alloc::vec::Vec;
use berm_lang::{Result, http::Client, serde::de::DeserializeOwned, serde_json};
use url::Url;

const API: &str = "https://api.resend.com";
const HEADERS: [(&str, &str); 2] = [
    ("Authorization", "Bearer {secret:RESEND_API_KEY}"),
    ("Content-Type", "application/json"),
];

fn request<T: DeserializeOwned>(
    method: &str,
    path: &[&str],
    body: &[u8],
    key: Option<&str>,
) -> Result<T> {
    let mut url = Url::parse(API).expect("constant origin");
    url.path_segments_mut().expect("HTTP URL").extend(path);
    let mut headers = Vec::from(HEADERS);
    if let Some(key) = key {
        headers.push(("Idempotency-Key", key));
    }
    Client {
        name: "Resend",
        headers: &headers,
    }
    .json(method, url.as_str(), body)
}

/// Send an email. Reuse the key with the same payload when retrying within 24 hours.
pub fn send(email: &Email, idempotency_key: &str) -> Result<Sent> {
    let body = serde_json::to_vec(email)?;
    request("POST", &["emails"], &body, Some(idempotency_key))
}

/// Send up to 100 emails in one request, with one idempotency key for the batch.
pub fn send_batch(emails: &[Email], idempotency_key: &str) -> Result<Batch> {
    let body = serde_json::to_vec(emails)?;
    request("POST", &["emails", "batch"], &body, Some(idempotency_key))
}

/// Read an email's latest provider status.
pub fn retrieve(id: &str) -> Result<Delivery> {
    request("GET", &["emails", id], &[], None)
}
