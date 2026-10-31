//! `berm.var` — one setting of the asking service's `[vars]`.
//!
//! berm reads the request and answers a var the manifest does not declare
//! with a refusal. The value a deployed copy holds is the host's, and arrives
//! through the closure below; a copy holding none reads the default.

use crate::{Callsite, Refused, Syscall, abi, wire};
use anyhow::Result;
use std::{future::Future, sync::Arc};

/// Serve `berm.var` through `read`, handed the asking site and the var's name
/// and answering the value its copy holds, `None` when it holds none.
pub fn services<R, Found>(read: R) -> Syscall
where
    R: Fn(Callsite, String) -> Found + Send + Sync + 'static,
    Found: Future<Output = Result<Option<String>>> + Send + 'static,
{
    let read = Arc::new(read);
    Syscall {
        name: abi::VAR.to_owned(),
        call: Arc::new(move |at: Callsite, request: Vec<u8>| {
            let read = read.clone();
            Box::pin(async move {
                let fields = abi::wire::fields(&request)?;
                let name = wire::text(&fields, 0, "var")?.to_owned();
                let Some(declared) = at.deps.vars.get(&name) else {
                    return Err(Refused(format!("{name} is not declared in [vars]")).into());
                };
                let default = declared.default.clone();
                Ok(read(at, name).await?.unwrap_or(default).into_bytes())
            })
        }),
    }
}
