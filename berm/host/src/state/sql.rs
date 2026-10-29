//! `berm.sql`, over the database of the service that keeps one.

use crate::{
    env::Env,
    host::{self, Inner},
};
use anyhow::bail;
use berm::{
    Callsite, Meter, Syscall,
    syscall::sql::{self, Statement},
};
use std::sync::Weak;

/// `berm.sql` for `host`'s services: a query chain reads, any other writes.
pub(crate) fn syscall<E: Env>(host: &Weak<Inner<E>>) -> Syscall {
    let host = host.clone();
    sql::programs(move |at: Callsite, statement: Statement| {
        let kept = host::alive(&host).map(|host| at.database(|| host.kept(&at.program)));
        async move {
            let Some(database) = kept? else {
                bail!("this service keeps no database");
            };
            let database = database?;
            let answer = match at.query {
                true => database.read(at.chain, statement).await?,
                false => database.exec(at.chain, statement).await?,
            };
            at.meter(Meter {
                read: answer.read,
                written: answer.written,
                sql_cpu: answer.cpu,
                ..Meter::default()
            });
            Ok(answer.json)
        }
    })
}
