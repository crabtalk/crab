//! `berm.get` and `berm.set` — a service's own bytes, surviving its invocations.
//!
//! Neither door takes a service. The keyspace is whichever one is asking, read
//! off the [`Callsite`], so another service's keys are not refused — a guest
//! has no way to name them.
//!
//! The host takes a write through the closure below, or leaves it to the
//! chain's journal, which hands it back in [`crate::Changes`]. A read sees the
//! chain's journal first, then asks the host. Each read is metered as one row
//! read and each write as one row written.

use crate::{Callsite, Meter, Refused, Syscall, abi, wire};
use anyhow::{Result, bail};
use std::{future::Future, sync::Arc};

/// Serve both doors, reading through `read`, handed the asking site and the
/// key, and handing each write to `write`, which answers `false` to leave it
/// to the chain's journal.
///
/// `read` answers `None` for a key never written. The guest is suspended
/// until it resolves.
pub fn services<R, Found, W, Taken>(read: R, write: W) -> Vec<Syscall>
where
    R: Fn(Callsite, String) -> Found + Send + Sync + 'static,
    Found: Future<Output = Result<Option<Vec<u8>>>> + Send + 'static,
    W: Fn(Callsite, String, Vec<u8>) -> Taken + Send + Sync + 'static,
    Taken: Future<Output = Result<bool>> + Send + 'static,
{
    values(abi::GET, abi::SET, true, read, write)
}

/// Serve a keyspace whose writes are handled by the supplied store.
pub fn caller<R, Found, W, Taken>(read: R, write: W) -> Vec<Syscall>
where
    R: Fn(Callsite, String) -> Found + Send + Sync + 'static,
    Found: Future<Output = Result<Option<Vec<u8>>>> + Send + 'static,
    W: Fn(Callsite, String, Vec<u8>) -> Taken + Send + Sync + 'static,
    Taken: Future<Output = Result<bool>> + Send + 'static,
{
    values(abi::CALLER_GET, abi::CALLER_SET, false, read, write)
}

fn values<R, Found, W, Taken>(
    get_name: &'static str,
    set_name: &'static str,
    journal: bool,
    read: R,
    write: W,
) -> Vec<Syscall>
where
    R: Fn(Callsite, String) -> Found + Send + Sync + 'static,
    Found: Future<Output = Result<Option<Vec<u8>>>> + Send + 'static,
    W: Fn(Callsite, String, Vec<u8>) -> Taken + Send + Sync + 'static,
    Taken: Future<Output = Result<bool>> + Send + 'static,
{
    let (read, write) = (Arc::new(read), Arc::new(write));
    let get = move |at: Callsite, request: Vec<u8>| {
        let read = read.clone();
        async move {
            let fields = abi::wire::fields(&request)?;
            let key = wire::text(&fields, 0, "key")?;
            at.rows_left()?;
            let held = journal.then(|| at.journal.read(&at.service, key)).flatten();
            let value = match held {
                Some(held) => Some(held),
                None => read(at.clone(), key.to_owned()).await?,
            };
            at.meter(Meter {
                read: 1,
                ..Meter::default()
            });
            // One field for a value, none for an absent key. Framed rather
            // than bare, so a stored empty value is not read as one that was
            // never written.
            Ok(match value {
                Some(value) => abi::wire::request(&[&value]),
                None => Vec::new(),
            })
        }
    };
    let set = move |at: Callsite, request: Vec<u8>| {
        let write = write.clone();
        async move {
            let fields = abi::wire::fields(&request)?;
            let key = wire::text(&fields, 0, "key")?;
            let Some(value) = fields.get(1) else {
                bail!("request has no value");
            };
            if at.query {
                return Err(Refused(format!("a query cannot write; {key:?} was not set")).into());
            }
            at.rows_left()?;
            if !write(at.clone(), key.to_owned(), value.to_vec()).await? && journal {
                at.journal.write(&at.service, key, value)?;
            }
            at.meter(Meter {
                written: 1,
                ..Meter::default()
            });
            Ok(Vec::new())
        }
    };
    vec![
        Syscall {
            name: get_name.to_owned(),
            call: Arc::new(move |at: Callsite, request: Vec<u8>| Box::pin(get(at, request))),
        },
        Syscall {
            name: set_name.to_owned(),
            call: Arc::new(move |at: Callsite, request: Vec<u8>| Box::pin(set(at, request))),
        },
    ]
}
