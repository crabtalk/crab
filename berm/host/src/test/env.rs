use crate::{
    env::Env,
    test::{Arranged, Generated, Given, Granted, Manual, Nowhere, Test},
};

impl Env for Test {
    type Fetch = Arranged;
    type Images = Given;
    type Peers = Nowhere;
    type Secrets = Granted;
    type Issuer = Generated;
    type Observe = ();
    type Clock = Manual;
}
