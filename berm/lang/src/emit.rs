//! Logs, for whoever reads this service's logs.
//!
//! A host keeps each log beside the call it was emitted in, whatever that call
//! ends as.

use crate::{
    abi::wire,
    host::{self, CallError},
    sys,
};
use alloc::vec::Vec;

/// Emit `data` under `topics`, which a reader filters by.
///
/// ```ignore
/// berm_lang::emit(&["item", "hn"], r#"{"id":1}"#)?;
/// ```
pub fn emit(topics: &[&str], data: &str) -> Result<(), CallError> {
    let mut request = Vec::new();
    wire::field(&mut request, data.as_bytes());
    for topic in topics {
        wire::field(&mut request, topic.as_bytes());
    }
    host::call(sys::EMIT, &request)?;
    Ok(())
}

/// A typed log: an `#[event]` struct, or one `declare_service!` generated
/// from another service's events. It is emitted under the topic
/// [`Event::NAME`].
pub trait Event {
    const NAME: &'static str;
}

/// What [`crate::emit!`] expands to.
#[doc(hidden)]
pub fn event<T: Event + serde::Serialize>(event: &T) -> Result<usize, CallError> {
    let data = serde_json::to_string(event)
        .map_err(|error| CallError::Failed(alloc::format!("unserializable event: {error}")))?;
    emit(&[T::NAME], &data)?;
    Ok(data.len())
}

/// Emit an `#[event]` struct as a log under its name, and answer the bytes of
/// its data.
///
/// ```ignore
/// berm_lang::emit!(Posted { id: 1 })?;
/// ```
#[macro_export]
macro_rules! emit {
    ($event:expr) => {
        $crate::__event(&$event)
    };
}
