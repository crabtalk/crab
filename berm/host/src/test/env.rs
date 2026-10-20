use crate::{
    env::Env,
    test::{Arranged, Given, Granted, Manual, Nowhere, Test},
};

impl Env for Test {
    type Fetch = Arranged;
    type Images = Given;
    type Peers = Nowhere;
    type Secrets = Granted;
    type Observe = ();
    type Clock = Manual;
}
