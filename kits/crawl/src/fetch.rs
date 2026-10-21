//! GETs over `berm.http.fetch_all`, sent a batch at a time, following
//! redirects within a site.

use alloc::{format, string::String, vec::Vec};
use berm_lang::http::{self, Request, Response};
use url::Url;

/// Redirects followed before a request fails.
const REDIRECTS: usize = 5;

/// One GET on `domain`'s site.
pub struct Get<'a> {
    pub url: Url,
    pub domain: &'a str,
    pub headers: Vec<(&'a str, &'a str)>,
    /// The most bytes of the answer's body read.
    pub limit: usize,
}

/// GET each of `gets` as `agent`, each request given up when nothing answered
/// within `timeout` milliseconds, and answer each in its place. The requests
/// are sent at once, then the redirects their answers name, a batch a round,
/// following a redirect to a host of its site only: the domain, a subdomain
/// of it, or a domain it is a subdomain of.
pub fn get_all(gets: &[Get<'_>], agent: &str, timeout: u64) -> Vec<Result<Response, String>> {
    let mut urls: Vec<Url> = gets.iter().map(|get| get.url.clone()).collect();
    let mut answers: Vec<Option<Result<Response, String>>> = gets.iter().map(|_| None).collect();
    for _ in 0..=REDIRECTS {
        let pending: Vec<usize> = (0..gets.len())
            .filter(|&at| answers[at].is_none())
            .collect();
        if pending.is_empty() {
            break;
        }
        let sent: Vec<Vec<(&str, &str)>> = pending
            .iter()
            .map(|&at| {
                let mut sent = alloc::vec![("User-Agent", agent)];
                sent.extend_from_slice(&gets[at].headers);
                sent
            })
            .collect();
        let requests: Vec<Request<'_>> = pending
            .iter()
            .zip(&sent)
            .map(|(&at, headers)| Request {
                method: "GET",
                url: urls[at].as_str(),
                headers,
                body: b"",
                timeout: Some(timeout),
                limit: Some(gets[at].limit),
            })
            .collect();
        let answered = match http::fetch_all(&requests) {
            Ok(answered) => answered,
            Err(error) => {
                for &at in &pending {
                    answers[at] = Some(Err(format!("{}: {}", urls[at], error.message())));
                }
                break;
            }
        };
        for (at, answer) in pending.into_iter().zip(answered) {
            let url = &urls[at];
            let response = match answer {
                Ok(response) => response,
                Err(error) => {
                    answers[at] = Some(Err(format!("{url}: {}", error.message())));
                    continue;
                }
            };
            if !matches!(response.status, 301 | 302 | 303 | 307 | 308) {
                answers[at] = Some(Ok(response));
                continue;
            }
            let Some(location) = header(&response, "location") else {
                answers[at] = Some(Err(format!("{url}: redirect with no Location")));
                continue;
            };
            match url.join(location) {
                Ok(next)
                    if next
                        .host_str()
                        .is_some_and(|host| crate::same_site(host, gets[at].domain)) =>
                {
                    urls[at] = next;
                }
                Ok(next) => {
                    answers[at] = Some(Err(format!("{url}: redirect off the site, to {next}")));
                }
                Err(error) => {
                    answers[at] = Some(Err(format!("{url}: redirect to {location}: {error}")));
                }
            }
        }
    }
    answers
        .into_iter()
        .zip(urls)
        .map(|(answer, url)| {
            answer.unwrap_or_else(|| Err(format!("{url}: more than {REDIRECTS} redirects")))
        })
        .collect()
}

/// The value of `response`'s header `name`, ASCII case ignored.
pub fn header<'a>(response: &'a Response, name: &str) -> Option<&'a str> {
    response
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}
