use crate::audit::{
    AGENT, CHECK_BATCH,
    discovery::{request, web_url},
    page::PageScan,
    run::Run,
};
use berm_lang::{Result, caller_storage, deadline, http, now, serde, serde_json, tool};
use url::Url;
use webpage::page::{MAX_HTML, Page};

const CHECK_TIMEOUT: u64 = 1_800;
const MAX_VALIDATOR_BYTES: usize = 512;
const FINISH_BUDGET_MS: u64 = 12_000;

type Candidate = (String, u8, Option<String>, Option<String>, Option<String>);

#[derive(Default, serde::Serialize)]
#[serde(crate = "berm_lang::serde")]
struct Check {
    status: Option<u16>,
    problem: Option<String>,
    redirect: Option<String>,
    etag: Option<String>,
    last_modified: Option<String>,
    hash: Option<String>,
    fields: Option<PageScan>,
    unchanged: bool,
}

impl Check {
    fn inspect(
        url: &str,
        prior: &Option<String>,
        response: Result<http::Response, berm_lang::CallError>,
    ) -> Result<Self> {
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                return Ok(Self {
                    problem: Some(error.message().into()),
                    ..Self::default()
                });
            }
        };
        let mut check = Self {
            status: Some(response.status),
            ..Self::default()
        };
        let target = Url::parse(url).map_err(|_| "Invalid sitemap page URL")?;
        let validator = |name| {
            response
                .header(name)
                .filter(|value| value.len() <= MAX_VALIDATOR_BYTES && !value.contains("{secret:"))
                .map(str::to_owned)
        };
        check.etag = validator("etag");
        check.last_modified = validator("last-modified");
        match response.status {
            304 if prior.is_some() => {
                check.status = Some(200);
                check.unchanged = true;
            }
            304 => check.problem = Some("HTTP 304 with no stored HTML check".into()),
            300..400 => {
                check.redirect = response
                    .header("location")
                    .and_then(|location| target.join(location).ok())
                    .and_then(web_url)
                    .map(Into::into);
            }
            200 => match Page::try_from((target, response)) {
                Ok(page) => {
                    let fields = PageScan::inspect(&page);
                    check.hash = Some(fields.hash()?);
                    check.fields = Some(fields);
                }
                Err(error) => check.problem = Some(error.message),
            },
            _ => check.problem = Some(format!("The page returned HTTP {}", response.status)),
        }
        Ok(check)
    }
}

/// Check for page changes
#[tool(
    access = "internal",
    budget(cpu = 100000, read = 2000, written = 100, memory = 128)
)]
pub(crate) fn audit_check(site: String, urls: Vec<String>) -> Result<bool> {
    let run = Run::owns(&site)?;
    for urls in urls.chunks(CHECK_BATCH) {
        if deadline() <= FINISH_BUDGET_MS {
            break;
        }
        check_batch(&site, &run, urls)?;
    }
    Ok(true)
}

fn check_batch(site: &str, run: &Run, urls: &[String]) -> Result<()> {
    let rows = caller_storage::rows::<Candidate>(
        "SELECT p.url, p.blocked, p.etag, p.last_modified, p.hash
         FROM json_each(?2) j CROSS JOIN pages p ON p.site = ?1 AND p.url = j.value
         WHERE p.checked_run IS NOT ?3",
        &(site, serde_json::to_string(urls)?, &run.id),
    )?;
    let mut updates = Vec::with_capacity(rows.len());
    let mut requests = Vec::new();
    let mut outbound = Vec::new();
    let mut headers = Vec::new();
    for (url, blocked, etag, last_modified, prior) in &rows {
        if *blocked != 0 {
            updates.push((url, Check::default()));
            continue;
        }
        let mut values = vec![("User-Agent", AGENT)];
        if prior.is_some() {
            if let Some(value) = etag {
                values.push(("If-None-Match", value));
            }
            if let Some(value) = last_modified {
                values.push(("If-Modified-Since", value));
            }
        }
        headers.push(values);
        outbound.push((url, prior));
    }
    for ((url, _), headers) in outbound.iter().zip(&headers) {
        requests.push(http::Request {
            headers,
            timeout: Some(CHECK_TIMEOUT),
            ..request(url, MAX_HTML + 1)
        });
    }
    if !requests.is_empty() {
        for ((url, prior), answer) in outbound.into_iter().zip(http::fetch_all(&requests)?) {
            updates.push((url, Check::inspect(url, prior, answer)?));
        }
    }
    let checked_at = now();
    for (url, check) in updates {
        caller_storage::rows::<()>(
            "UPDATE pages SET
                checked_at = ?4, checked_run = ?3, status = ?5 ->> '$.status',
                problem = ?5 ->> '$.problem', redirect = ?5 ->> '$.redirect',
                etag = CASE WHEN ?5 ->> '$.unchanged'
                            THEN coalesce(?5 ->> '$.etag', etag)
                            WHEN ?5 ->> '$.hash' IS NOT NULL THEN ?5 ->> '$.etag' END,
                last_modified = CASE WHEN ?5 ->> '$.unchanged'
                            THEN coalesce(?5 ->> '$.last_modified', last_modified)
                            WHEN ?5 ->> '$.hash' IS NOT NULL THEN ?5 ->> '$.last_modified' END,
                checked_fields = CASE WHEN ?5 ->> '$.unchanged' THEN checked_fields
                            WHEN (?5 ->> '$.hash') IS NOT scan_hash THEN ?5 ->> '$.fields' END,
                hash = CASE WHEN ?5 ->> '$.unchanged' THEN hash ELSE ?5 ->> '$.hash' END
             WHERE site = ?1 AND url = ?2",
            &(
                &site,
                url,
                &run.id,
                checked_at,
                serde_json::to_string(&check)?,
            ),
        )?;
    }
    Ok(())
}
