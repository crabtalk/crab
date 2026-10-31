use crate::{Evaluation, Event, Flags, Region};
use alloc::{format, string::String};
use berm_lang::{
    Result, http,
    serde::Serialize,
    serde_json::{self, Value, json},
};

/// PostHog Cloud, using host-managed project and personal API credentials.
pub struct PostHog {
    region: Region,
}
impl PostHog {
    pub fn new(region: Region) -> Self {
        Self { region }
    }
    fn host(&self, private: bool) -> &'static str {
        match (self.region, private) {
            (Region::Us, false) => "https://us.i.posthog.com",
            (Region::Eu, false) => "https://eu.i.posthog.com",
            (Region::Us, true) => "https://us.posthog.com",
            (Region::Eu, true) => "https://eu.posthog.com",
        }
    }
    fn request(&self, path: &str, body: &impl Serialize, private: bool) -> Result<http::Response> {
        let body = serde_json::to_vec(body).map_err(|e| format!("PostHog request JSON: {e}"))?;
        let mut headers = alloc::vec![
            ("Content-Type", "application/json"),
            ("User-Agent", "crabtalk-posthog")
        ];
        if private {
            headers.push(("Authorization", "Bearer {secret:POSTHOG_PERSONAL_API_KEY}"));
        }
        let response = http::fetch(
            "POST",
            &format!("{}{path}", self.host(private)),
            &headers,
            &body,
        )?;
        if !response.is_success() {
            let value = response.json::<Value>().ok();
            let message = value
                .as_ref()
                .and_then(|v| v["detail"].as_str().or_else(|| v["error"].as_str()))
                .unwrap_or("provider request failed");
            let retry = response
                .header("retry-after")
                .map(|v| format!("; retry-after={v}"))
                .unwrap_or_default();
            return Err(format!("PostHog {}: {message}{retry}", response.status).into());
        }
        Ok(response)
    }
    /// Submit one event. Success means accepted, not immediately queryable.
    pub fn capture(&self, event: &Event) -> Result<()> {
        check_event(event)?;
        let mut body =
            serde_json::to_value(event).map_err(|e| format!("PostHog event JSON: {e}"))?;
        body["api_key"] = json!("{secret:POSTHOG_PROJECT_TOKEN}");
        self.request("/i/v0/e/", &body, false)?;
        Ok(())
    }
    /// Submit a batch. Calls make one attempt; stable event UUIDs help deduplicate retries.
    pub fn capture_batch(&self, events: &[Event]) -> Result<()> {
        if events.is_empty() {
            return Err("Provide at least one event".into());
        }
        for event in events {
            check_event(event)?;
        }
        self.request(
            "/batch/",
            &json!({"api_key":"{secret:POSTHOG_PROJECT_TOKEN}","batch":events}),
            false,
        )?;
        Ok(())
    }
    /// Evaluate flags remotely, preserving partial evaluation status. Does not log exposure.
    pub fn flags(&self, evaluation: &Evaluation) -> Result<Flags> {
        check_identity(&evaluation.distinct_id)?;
        let mut body =
            serde_json::to_value(evaluation).map_err(|e| format!("PostHog flags JSON: {e}"))?;
        body["api_key"] = json!("{secret:POSTHOG_PROJECT_TOKEN}");
        let value: Value = self
            .request("/flags?v=2", &body, false)?
            .json()
            .map_err(|_| "PostHog returned unreadable flags")?;
        let flags = serde_json::from_value(
            value
                .get("flags")
                .cloned()
                .ok_or("PostHog response has no flags")?,
        )
        .map_err(|_| "PostHog returned invalid flags")?;
        let errors_while_computing_flags = value["errorsWhileComputingFlags"]
            .as_bool()
            .ok_or("PostHog response has no evaluation status")?;
        Ok(Flags {
            flags,
            errors_while_computing_flags,
            request_id: value["requestId"].as_str().map(String::from),
        })
    }
    /// Run a blocking HogQL query with a personal API key scoped to query:read.
    pub fn query(&self, project_id: u64, sql: &str, name: &str) -> Result<Value> {
        if project_id == 0 || sql.trim().is_empty() || name.trim().is_empty() {
            return Err("Provide a project ID, HogQL query and descriptive name".into());
        }
        let body = json!({
            "query": {"kind": "HogQLQuery", "query": sql},
            "name": name,
            "async": false,
            "refresh": "blocking",
        });
        self.request(&format!("/api/projects/{project_id}/query/"), &body, true)?
            .json()
            .map_err(|_| "PostHog returned an unreadable query response".into())
    }
}
fn check_identity(id: &str) -> Result<()> {
    if id.trim().is_empty() || id.chars().count() > 200 {
        return Err("distinct_id must contain 1 to 200 characters".into());
    }
    Ok(())
}
fn check_event(event: &Event) -> Result<()> {
    check_identity(&event.distinct_id)?;
    if event.event.trim().is_empty() {
        return Err("An event name is required".into());
    }
    Ok(())
}
