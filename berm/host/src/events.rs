use crate::{Denied, Env, Host, deny};
use axum::http::StatusCode;
use berm_abi::events::Page;
use berm_api::{
    Output,
    events::Source,
    host::{self, Caller},
};
use serde_json::Value;

impl<E: Env> Host<E> {
    /// Read a committed event page through the caller's ordinary authorization boundary.
    pub async fn event_page(
        &self,
        service: &str,
        caller: Option<&Caller>,
        name: &str,
        filters: Value,
        cursor: Option<&str>,
    ) -> Result<Page<Value>, Denied> {
        let idl = self.idl(service)?;
        let tool = idl
            .tools
            .iter()
            .find(|tool| tool.events.as_deref() == Some(name))
            .ok_or_else(|| deny(StatusCode::NOT_FOUND, "No such event source"))?;
        let source = Source::try_from(tool)?;
        let args = source
            .arguments(filters, cursor)
            .map_err(|error| deny(StatusCode::BAD_REQUEST, error.to_string()))?;
        let (headers, output) = self.call(service, caller, &source.tool, args, true).await?;
        let body = match output {
            Output::Done(body) => body,
            Output::Failed(error) => {
                let status = if headers.contains_key(host::DENIED) {
                    StatusCode::FORBIDDEN
                } else {
                    StatusCode::BAD_REQUEST
                };
                return Err(deny(status, error));
            }
        };
        source
            .page(&body, cursor)
            .map_err(|error| deny(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
    }
}
