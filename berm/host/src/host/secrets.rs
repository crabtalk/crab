use crate::{
    env::{Env, Secrets},
    host::{self, Inner},
};
use anyhow::bail;
use berm::{Callsite, Refused, Syscall};
use berm_abi::{self as abi, wire};
use berm_api::Credential;
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
                let owner = host?
                    .secrets
                    .resource_owner(&at.service, id, destination)
                    .await?;
                Ok(owner.map_or_else(Vec::new, |owner| wire::request(&[owner.as_bytes()])))
            })
        }),
    }
}
