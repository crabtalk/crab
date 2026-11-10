//! Outbound HTTP destinations declared by a service or linked library.

use berm_abi as abi;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Http {
    pub fetch: Vec<String>,
    pub proxy: Vec<String>,
}

impl Http {
    pub fn is_empty(&self) -> bool {
        self.fetch.is_empty() && self.proxy.is_empty()
    }

    /// Matching ignores ASCII case. `*.example.com` excludes `example.com`.
    /// A name wins over any matching pattern.
    pub fn reach(&self, syscall: &str, host: &str) -> Option<Reach> {
        let declared = match syscall {
            abi::HTTP_FETCH | abi::HTTP_FETCH_ALL | abi::HTTP_FETCH_ALL_LITERAL_BODY => &self.fetch,
            abi::HTTP_PROXY | abi::HTTP_PROXY_ALL => &self.proxy,
            _ => return None,
        };
        if declared.iter().any(|item| item.eq_ignore_ascii_case(host)) {
            return Some(Reach::Named);
        }
        declared
            .iter()
            .any(|item| match item.strip_prefix('*') {
                Some("") => true,
                Some(suffix) => {
                    suffix.starts_with('.')
                        && host.len().checked_sub(suffix.len()).is_some_and(|at| {
                            host.as_bytes()[at..].eq_ignore_ascii_case(suffix.as_bytes())
                        })
                }
                None => false,
            })
            .then_some(Reach::Pattern)
    }
}

/// How a manifest declares a host it reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    Named,
    /// Only through `*` or `*.domain`.
    Pattern,
}
