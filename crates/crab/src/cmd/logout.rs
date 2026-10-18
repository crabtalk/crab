//! `crab logout` — forget the session a context holds.

use crate::context::{self, Contexts};
use anyhow::{Result, bail};

/// Drop the token of context `name`, or of the one in use.
pub fn run(name: Option<&str>) -> Result<()> {
    let mut contexts = Contexts::load()?;
    let name = name
        .or(contexts.current.as_deref())
        .unwrap_or(context::DEFAULT)
        .to_owned();
    let Some(context) = contexts.contexts.get_mut(&name) else {
        bail!("{name:?} holds no session");
    };
    context.token = None;
    contexts.save()?;
    eprintln!("signed out of {name}");
    Ok(())
}
