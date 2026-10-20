use crate::{Alone, Fetch, Grants, Keys, Stored};
use berm_host::{Env, System};

/// One machine, running every program itself.
#[derive(Clone)]
pub struct Local;

impl Env for Local {
    type Fetch = Fetch;
    type Images = Stored;
    type Peers = Alone;
    type Secrets = Grants;
    type Issuer = Keys;
    type Observe = ();
    type Clock = System;
}
