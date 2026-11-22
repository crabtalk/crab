use crate::{
    deny,
    env::{Env, Secrets},
    host::{self, Inner},
    state::caller,
};
use anyhow::bail;
use axum::http::StatusCode;
use berm::{Callsite, Refused, Syscall, syscall::Abort};
use berm_abi::{self as abi, wire};
use berm_api::{Credential, workflow::run::Failure};
use std::{
    str,
    sync::{Arc, Weak},
};

pub(crate) fn syscall<E: Env>(host: &Weak<Inner<E>>) -> Syscall {
    let host = host.clone();
    Syscall {
        name: abi::SECRET_RESOURCE.into(),
        call: Arc::new(move |at: Callsite, request| {
            let host = host::alive(&host);
            Box::pin(async move {
                let fields = wire::fields(&request)?;
                let [id, destination] = fields.as_slice() else {
                    bail!("Expected a resource ID and destination host")
                };
                let id = str::from_utf8(id)?;
                let destination = str::from_utf8(destination)?;
                abi::secrets::Id::try_from(id).map_err(anyhow::Error::msg)?;
                Credential::check_hosts(&[destination.into()]).map_err(anyhow::Error::msg)?;
                if !at.deps.secret_resources
                    || (at.deps.http.reach(abi::HTTP_FETCH, destination).is_none()
                        && at.deps.http.reach(abi::HTTP_PROXY, destination).is_none())
                {
                    return Err(Refused(
                        "The service cannot bind a runtime secret at that destination".into(),
                    )
                    .into());
                }
                let owner = resource_owner(&host?.secrets, &at, id, destination).await?;
                Ok(owner.map_or_else(Vec::new, |owner| wire::request(&[owner.as_bytes()])))
            })
        }),
    }
}

pub(crate) async fn resolve(
    secrets: &impl Secrets,
    at: &Callsite,
    secret: &str,
    to: &str,
) -> anyhow::Result<Option<String>> {
    if abi::secrets::Id::try_from(secret).is_ok()
        && resource_owner(secrets, at, secret, to).await?.is_none()
    {
        return Ok(None);
    }
    secrets.grant(&at.service, secret, to).await
}

async fn resource_owner<S: Secrets>(
    secrets: &S,
    at: &Callsite,
    id: &str,
    to: &str,
) -> anyhow::Result<Option<String>> {
    let owner = secrets.resource_owner(&at.service, id, to).await?;
    if let Some(owner) = owner.as_deref()
        && owner != S::service_owner(&at.service)
        && !caller::scheduled(at)
        && caller::account(at).as_deref() != Some(owner)
    {
        let refusal = deny(
            StatusCode::FORBIDDEN,
            "The runtime secret belongs to another account",
        )
        .explained("E0089");
        return Err(Abort(Failure::from(&refusal)).into());
    }
    Ok(owner)
}
