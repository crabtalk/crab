use crate::{
    env::{Alone, Env},
    fixture::Http,
    test::{Given, Granted, Manual, Test},
};

impl Env for Test {
    type Fetch = Http;
    type Images = Given;
    type Peers = Alone;
    type Secrets = Granted;
    type Observe = ();
    type Clock = Manual;
}
