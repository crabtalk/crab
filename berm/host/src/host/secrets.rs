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
use berm_api::{Credential, host as protocol, workflow::run::Failure};
use rusqlite::{Connection, params};
use std::{
    str,
    sync::{Arc, Weak},
};

pub(crate) fn syscalls<E: Env>(host: &Weak<Inner<E>>) -> [Syscall; 2] {
    [abi::SECRET_RESOURCE, abi::SECRET_BIND].map(|name| {
        let host = host.clone();
        let bind = name == abi::SECRET_BIND;
        Syscall {
            name: name.into(),
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
                        let message =
                            "The service has no runtime secret permission at that destination";
                        return Err(if bind {
                            refused(message)
                        } else {
                            Refused(message.into()).into()
                        });
                    }
                    if bind && (at.query || caller::scheduled(&at)) {
                        return Err(refused(
                            "Bind the runtime secret during an authenticated write call",
                        ));
                    }
                    let host = host?;
                    let owner = host.resource_owner(&at, id, destination, bind).await?;
                    Ok(owner.map_or_else(Vec::new, |owner| wire::request(&[owner.as_bytes()])))
                })
            }),
        }
    })
}

impl<E: Env> Inner<E> {
    pub(crate) async fn secret(
        &self,
        at: &Callsite,
        secret: &str,
        to: &str,
    ) -> anyhow::Result<Option<String>> {
        if abi::secrets::Id::try_from(secret).is_ok()
            && self.resource_owner(at, secret, to, false).await?.is_none()
        {
            return Ok(None);
        }
        self.secrets.grant(&at.service, secret, to).await
    }

    async fn resource_owner(
        &self,
        at: &Callsite,
        id: &str,
        to: &str,
        bind: bool,
    ) -> anyhow::Result<Option<String>> {
        let Some(owner) = self.secrets.resource_owner(&at.service, id, to).await? else {
            return Ok(None);
        };
        let scheduled = caller::scheduled(at);
        if !scheduled && caller::authenticated_account().as_deref() != Some(owner.as_str()) {
            return Err(refused("The runtime secret belongs to another account"));
        }
        if !scheduled && !bind {
            return Ok(Some(owner));
        }

        let (scope, name) = host::split(&at.service);
        let storage_id = at
            .storage_id
            .as_deref()
            .ok_or_else(|| refused("The runtime secret binding needs a current deployment"))?;
        let parts = [scope, name, storage_id, id, to].map(str::to_owned);
        let account = owner.clone();
        let exists = |conn: &Connection, parts: &[String; 5], account: &str| {
            let [scope, name, storage_id, id, to] = parts;
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM secret_bindings
                 WHERE scope = ?1 AND name = ?2 AND storage_id = ?3
                   AND secret = ?4 AND host = ?5 AND account = ?6)",
                params![scope, name, storage_id, id, to, account],
                |row| row.get::<_, bool>(0),
            )
        };
        let matches = if bind {
            self.store
                .run(move |conn| {
                    let [scope, name, storage_id, id, to] = &parts;
                    conn.execute(
                    "INSERT INTO secret_bindings (scope, name, storage_id, secret, host, account)
                     SELECT scope, name, storage_id, ?4, ?5, ?6 FROM services
                     WHERE scope = ?1 AND name = ?2 AND storage_id = ?3
                     ON CONFLICT DO NOTHING",
                    params![scope, name, storage_id, id, to, account],
                )?;
                    exists(conn, &parts, &account)
                })
                .await?
        } else {
            self.store
                .read(move |conn| exists(conn, &parts, &account))
                .await?
        };
        if !matches {
            return Err(refused(
                "The runtime secret is not bound by its account for this service and destination",
            ));
        }
        Ok(Some(owner))
    }
}

fn refused(message: &str) -> anyhow::Error {
    let refusal = deny(StatusCode::FORBIDDEN, message).explained(protocol::SECRET_ACCESS);
    Abort(Failure::from(&refusal)).into()
}
