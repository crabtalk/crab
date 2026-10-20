//! GET over `berm.http.fetch`, following redirects within a site.

use alloc::{format, string::String};
use berm_lang::http::{self, Response};
use url::Url;

/// Redirects followed before a request fails.
const REDIRECTS: usize = 5;

/// GET `url` as `agent` with `headers`, following a redirect to a host of
/// `domain`'s site: `domain`, a subdomain of it, or a domain it is a
/// subdomain of.
pub fn get(
    url: &Url,
    domain: &str,
    agent: &str,
    headers: &[(&str, &str)],
) -> Result<Response, String> {
    let mut url = url.clone();
    for _ in 0..=REDIRECTS {
        let mut sent = alloc::vec![("User-Agent", agent)];
        sent.extend_from_slice(headers);
        let response = http::fetch("GET", url.as_str(), &sent, b"")
            .map_err(|error| format!("{url}: {}", error.message()))?;
        if !matches!(response.status, 301 | 302 | 303 | 307 | 308) {
            return Ok(response);
        }
        let location = header(&response, "location")
            .ok_or_else(|| format!("{url}: redirect with no Location"))?;
        let next = url
            .join(location)
            .map_err(|error| format!("{url}: redirect to {location}: {error}"))?;
        if !next.host_str().is_some_and(|host| same_site(host, domain)) {
            return Err(format!("{url}: redirect off the site, to {next}"));
        }
        url = next;
    }
    Err(format!("{url}: more than {REDIRECTS} redirects"))
}

/// Whether `host` belongs to `domain`'s site.
pub fn same_site(host: &str, domain: &str) -> bool {
    let within = |inner: &str, outer: &str| {
        outer.contains('.')
            && (inner == outer
                || inner
                    .strip_suffix(outer)
                    .is_some_and(|prefix| prefix.ends_with('.')))
    };
    within(host, domain) || within(domain, host)
}

/// The value of `response`'s header `name`, ASCII case ignored.
pub fn header<'a>(response: &'a Response, name: &str) -> Option<&'a str> {
    response
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}
