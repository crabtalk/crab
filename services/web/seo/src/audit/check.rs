use crate::audit::{
    AGENT, CHECK_TIMEOUT, FINISH_BUDGET_MS,
    discovery::{request, web_url},
    markdown::SiteMarkdown,
    page::PageScan,
    run::Run,
};
use berm_lang::{Result, Schema, caller_storage, deadline, http, serde_json, tool};
use url::Url;
use webpage::page::{MAX_HTML, Page};

const MAX_VALIDATOR_BYTES: usize = 512;

type Candidate = (
    String,
    u8,
    Option<u16>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<u8>,
    u8,
);

#[derive(Schema)]
pub(crate) struct CheckRange {
    pub first: String,
    pub last: String,
}

impl CheckRange {
    pub fn contains(&self, url: &str) -> bool {
        let first = self.first.as_str();
        let last = self.last.as_str();
        if first <= last {
            first <= url && url <= last
        } else {
            first <= url || url <= last
        }
    }
}

#[derive(Default, PartialEq)]
struct Observation {
    status: Option<u16>,
    problem: Option<String>,
    redirect: Option<String>,
    etag: Option<String>,
    last_modified: Option<String>,
    hash: Option<String>,
    // HTML validators describe a different representation.
    markdown: Option<bool>,
}

impl Observation {
    fn inspect(
        url: &str,
        prior: &Self,
        response: Result<http::Response, berm_lang::CallError>,
    ) -> Result<(Self, Option<PageScan>)> {
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                return Ok((
                    Self {
                        problem: Some(error.message().into()),
                        ..Self::default()
                    },
                    None,
                ));
            }
        };
        let mut check = Self {
            status: Some(response.status),
            ..Self::default()
        };
        let validator = |name| {
            response
                .header(name)
                .filter(|value| value.len() <= MAX_VALIDATOR_BYTES && !value.contains("{secret:"))
                .map(str::to_owned)
        };
        let mut fields = None;
        match response.status {
            304 if prior.hash.is_some() => {
                check.status = Some(200);
                check.etag = validator("etag").or_else(|| prior.etag.clone());
                check.last_modified =
                    validator("last-modified").or_else(|| prior.last_modified.clone());
                check.hash.clone_from(&prior.hash);
            }
            304 => check.problem = Some("HTTP 304 with no stored HTML check".into()),
            300..400 => {
                let target = Url::parse(url).map_err(|_| "Invalid sitemap page URL")?;
                check.redirect = response
                    .header("location")
                    .and_then(|location| target.join(location).ok())
                    .and_then(web_url)
                    .map(Into::into);
            }
            200 => {
                check.etag = validator("etag");
                check.last_modified = validator("last-modified");
                let target = Url::parse(url).map_err(|_| "Invalid sitemap page URL")?;
                match Page::try_from((target, response)) {
                    Ok(page) => {
                        let scan = PageScan::inspect(&page);
                        check.hash = Some(scan.hash()?);
                        fields = Some(scan);
                    }
                    Err(error) => {
                        check.problem = Some(error.message);
                        check.etag = None;
                        check.last_modified = None;
                    }
                }
            }
            _ => check.problem = Some(format!("The page returned HTTP {}", response.status)),
        }
        Ok((check, fields))
    }

    fn store(&self, site: &str, url: &str, prior: &Self, fields: Option<PageScan>) -> Result<()> {
        if self == prior {
            return Ok(());
        }
        caller_storage::rows::<()>(
            "UPDATE pages SET status = ?3, problem = ?4, redirect = ?5,
                etag = ?6, last_modified = ?7,
                checked_fields = CASE WHEN hash IS ?8 THEN checked_fields
                                      WHEN scan_hash IS NOT ?8 THEN ?9 END,
                hash = ?8, markdown = ?10
             WHERE site = ?1 AND url = ?2",
            &(
                site,
                url,
                &self.status,
                &self.problem,
                &self.redirect,
                &self.etag,
                &self.last_modified,
                &self.hash,
                fields.as_ref().map(serde_json::to_string).transpose()?,
                self.markdown,
            ),
        )?;
        Ok(())
    }
}

/// Check for page changes
#[tool(
    access = "internal",
    budget(cpu = 100000, read = 2000, written = 100, memory = 128)
)]
pub(crate) fn audit_check(site: String, urls: Vec<String>) -> Result<bool> {
    Run::owns(&site)?;
    if deadline() <= FINISH_BUDGET_MS {
        return Ok(false);
    }
    let rows = caller_storage::rows::<Candidate>(
        "SELECT p.url, p.blocked, p.status, p.problem, p.redirect, p.etag, p.last_modified, p.hash, p.markdown, p.scan_hash IS NOT NULL
         FROM json_each(?2) j CROSS JOIN pages p ON p.site = ?1 AND p.url = j.value",
        &(&site, &urls),
    )?;
    let mut outbound = Vec::new();
    for (url, blocked, status, problem, redirect, etag, last_modified, hash, markdown, scanned) in
        rows
    {
        let prior = Observation {
            status,
            problem,
            redirect,
            etag,
            last_modified,
            hash,
            markdown: markdown.map(|value| value != 0),
        };
        if blocked == 0 {
            outbound.push((url, prior, scanned != 0));
        } else {
            Observation::default().store(&site, &url, &prior, None)?;
        }
    }

    if !outbound.is_empty() {
        let headers: Vec<_> = outbound
            .iter()
            .map(|(_, prior, _)| {
                let mut headers = vec![("User-Agent", AGENT)];
                if let Some(etag) = &prior.etag {
                    headers.push(("If-None-Match", etag));
                }
                if let Some(last_modified) = &prior.last_modified {
                    headers.push(("If-Modified-Since", last_modified));
                }
                headers
            })
            .collect();
        let requests: Vec<_> = outbound
            .iter()
            .zip(&headers)
            .map(|((url, _, _), headers)| http::Request {
                headers,
                timeout: Some(CHECK_TIMEOUT),
                ..request(url, MAX_HTML + 1)
            })
            .collect();
        let mut observations = Vec::with_capacity(outbound.len());
        for ((url, prior, _), answer) in outbound.iter().zip(http::fetch_all(&requests)?) {
            observations.push(Observation::inspect(url, prior, answer)?);
        }
        let mut indices = Vec::new();
        let mut pages = Vec::new();
        for (index, ((url, _, scanned), (check, _))) in
            outbound.iter().zip(&observations).enumerate()
        {
            if *scanned && check.hash.is_some() {
                indices.push(index);
                pages.push(url.as_str());
            }
        }
        if !pages.is_empty()
            && let Some(markdown) = SiteMarkdown::read(&site)?
        {
            for (index, coverage) in indices.into_iter().zip(markdown.coverage(&pages)?) {
                observations[index].0.markdown = coverage;
            }
        }
        for ((url, prior, _), (check, fields)) in outbound.iter().zip(observations) {
            check.store(&site, url, prior, fields)?;
        }
    }
    Ok(true)
}
