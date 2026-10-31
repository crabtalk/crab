use crate::{
    env::{Alone, Env},
    test::{Arranged, Given, Granted, Manual, Test},
};

impl Env for Test {
    type Fetch = Arranged;
    type Images = Given;
    type Peers = Alone;
    type Secrets = Granted;
    type Observe = ();
    type Clock = Manual;
}
