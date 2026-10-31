use crate::{Batch, Delivery, Email, Sent};
use alloc::{format, string::String, vec::Vec};
use berm_lang::{Result, http, serde::de::DeserializeOwned, serde_json};

fn request<T: DeserializeOwned>(
    method: &str,
    path: &[&str],
    body: &[u8],
    key: Option<&str>,
) -> Result<T> {
    let mut url = url::Url::parse("https://api.resend.com").expect("constant origin");
    url.path_segments_mut().expect("HTTP URL").extend(path);
    let mut headers = Vec::from([
        ("Authorization", "Bearer {secret:RESEND_API_KEY}"),
        ("Content-Type", "application/json"),
    ]);
    if let Some(key) = key {
        headers.push(("Idempotency-Key", key));
    }
    let response = http::fetch(method, url.as_str(), &headers, body)?;
    if !response.is_success() {
        let failure = response.json::<serde_json::Value>().ok();
        let name = failure
            .as_ref()
            .and_then(|v| v["name"].as_str())
            .unwrap_or("request_failed");
        let message = failure
            .as_ref()
            .and_then(|v| v["message"].as_str())
            .unwrap_or("provider request failed");
        let retry = response
            .header("retry-after")
            .map(|v| format!("; retry-after={v}"))
            .unwrap_or_default();
        return Err(format!("Resend {} ({name}): {message}{retry}", response.status).into());
    }
    response
        .json()
        .map_err(|_| String::from("Resend returned an unreadable response").into())
}

/// Send an email. Reuse the key with the same payload when retrying within 24 hours.
pub fn send(email: &Email, idempotency_key: &str) -> Result<Sent> {
    let body = serde_json::to_vec(email).map_err(|e| format!("email JSON: {e}"))?;
    request("POST", &["emails"], &body, Some(idempotency_key))
}

/// Send up to 100 emails in one request, with one idempotency key for the batch.
pub fn send_batch(emails: &[Email], idempotency_key: &str) -> Result<Batch> {
    let body = serde_json::to_vec(emails).map_err(|e| format!("email JSON: {e}"))?;
    request("POST", &["emails", "batch"], &body, Some(idempotency_key))
}

/// Read an email's latest provider status.
pub fn retrieve(id: &str) -> Result<Delivery> {
    request("GET", &["emails", id], &[], None)
}
