use crate::host::{Fetch, secrets::Grants};
use berm_host::{Alone, Env, Stored, System};

/// One machine, running every program itself.
#[derive(Clone)]
pub struct Local;

impl Env for Local {
    type Fetch = Fetch;
    type Images = Stored;
    type Peers = Alone;
    type Secrets = Grants;
    type Observe = ();
    type Clock = System;
}
