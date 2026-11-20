use crate::{
    audit::{
        check::CheckRange,
        discovery::{allowed, request, web_url, within},
        run::Run,
    },
    crawlers::Robots,
    report::Code,
};
use berm_lang::{Result, Schema, caller_storage, deadline, http, now, serde, serde_json, tool};
use std::collections::{BTreeMap, BTreeSet};
use url::Url;

const MAX_TARGETS: usize = 100;
const MAX_FINDINGS_BYTES: usize = 256 * 1024;
const PAGE_BATCH: usize = 64;
const SAVE_RETRY_MS: u64 = 120_000;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Schema)]
enum Kind {
    SitemapDisallows,
    PageRedirect,
    PageUnavailable,
    DuplicateTitle,
    DuplicateDescription,
    CanonicalRedirect,
    CanonicalError,
    BrokenLink,
    MissingFromSitemap,
    Metadata(Code),
}

#[derive(Clone, Schema)]
struct Finding {
    kind: Kind,
    page: String,
    related: Option<String>,
    status: Option<u16>,
    detail: Option<String>,
    /// At least one page scan or HTTP observation is from an earlier run.
    stale: bool,
}

impl Finding {
    fn key(&self) -> (Kind, &str, Option<&str>) {
        let related = match self.kind {
            Kind::DuplicateTitle | Kind::DuplicateDescription | Kind::PageRedirect => None,
            _ => self.related.as_deref(),
        };
        (self.kind, &self.page, related)
    }

    fn new(kind: Kind, page: &str, related: Option<&str>, stale: bool) -> Self {
        Self {
            kind,
            page: page.into(),
            related: related.map(str::to_owned),
            status: None,
            detail: None,
            stale,
        }
    }
}

#[derive(Clone, Schema)]
pub(crate) struct Report {
    new: Vec<Finding>,
    fixed: Vec<Finding>,
    findings: Vec<Finding>,
    listed: usize,
    checked: usize,
    audited: usize,
    pending: usize,
    site: String,
    sitemap_complete: bool,
    robots_read: bool,
    unverified_targets: usize,
    findings_truncated: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(crate = "berm_lang::serde")]
struct Analysis {
    report: Report,
    cursor: Option<String>,
    unresolved: Vec<Finding>,
}

#[derive(Default)]
struct Page {
    blocked: bool,
    status: Option<u16>,
    problem: Option<String>,
    redirect: Option<String>,
    checked: bool,
    audited: bool,
    scan_current: bool,
    title: Option<String>,
    description: Option<String>,
    canonical: Option<String>,
    metadata: Vec<Code>,
}

impl Page {
    fn confirmed(&self) -> bool {
        self.checked && self.scan_current
    }
}

#[derive(Default)]
struct Target {
    status: Option<u16>,
    html: bool,
    checked: bool,
}

type PageRow = (
    String,
    u8,
    Option<u16>,
    Option<String>,
    Option<String>,
    u8,
    u8,
    u8,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn pages(site: &str, run: &Run, checked: &[CheckRange]) -> Result<BTreeMap<String, Page>> {
    let mut pages = BTreeMap::new();
    let mut after = String::new();
    loop {
        let batch = caller_storage::rows::<PageRow>(
            "SELECT url, blocked, status, problem, redirect,
                    url NOT IN (SELECT url FROM unlisted_pages WHERE site = ?1), scanned_run IS ?2,
                    hash IS NOT NULL AND hash IS scan_hash AND NOT blocked,
                    scan ->> '$.metadata.title', scan ->> '$.metadata.description',
                    scan ->> '$.metadata.canonical', scan -> '$.metadata.findings'
             FROM pages WHERE site = ?1 AND url > ?3 ORDER BY url LIMIT ?4",
            &(site, &run.id, &after, PAGE_BATCH),
        )?;
        let done = batch.len() < PAGE_BATCH;
        for (
            url,
            blocked,
            status,
            problem,
            redirect,
            listed,
            audited,
            scan_current,
            title,
            description,
            canonical,
            metadata,
        ) in batch
        {
            let metadata = metadata
                .map(|json| serde_json::from_str::<Vec<crate::report::Finding>>(&json))
                .transpose()?
                .unwrap_or_default()
                .into_iter()
                .map(|finding| finding.code)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let canonical = canonical
                .and_then(|url| Url::parse(&url).ok().and_then(web_url))
                .map(Into::into);
            after.clone_from(&url);
            let checked = listed != 0 && checked.iter().any(|range| range.contains(&url));
            pages.insert(
                url,
                Page {
                    blocked: blocked != 0,
                    status,
                    problem,
                    redirect,
                    checked,
                    audited: audited != 0,
                    scan_current: scan_current != 0,
                    title,
                    description,
                    canonical,
                    metadata,
                },
            );
        }
        if done {
            return Ok(pages);
        }
    }
}

fn links(site: &str) -> Result<Vec<(String, String)>> {
    let mut links = Vec::new();
    let mut after = String::new();
    loop {
        let batch = caller_storage::rows::<(String, String)>(
            "SELECT p.url, link.value FROM (
                SELECT url, scan FROM pages
                WHERE site = ?1 AND url > ?2 AND scan IS NOT NULL ORDER BY url LIMIT ?3
             ) p, json_each(p.scan, '$.links') link
             UNION ALL
             SELECT url, '' FROM pages
             WHERE site = ?1 AND url = (
                SELECT url FROM pages WHERE site = ?1 AND url > ?2 AND scan IS NOT NULL
                ORDER BY url LIMIT 1 OFFSET (?3 - 1)
             )",
            &(site, &after, 4),
        )?;
        let mut end = true;
        for (page, link) in batch {
            if link.is_empty() {
                after = page;
                end = false;
            } else {
                links.push((page, link));
            }
        }
        if end {
            return Ok(links);
        }
    }
}

fn duplicates(
    pages: &BTreeMap<String, Page>,
    field: impl Fn(&Page) -> Option<&str>,
) -> impl Iterator<Item = Vec<(&str, &Page)>> {
    let mut groups = BTreeMap::<&str, Vec<_>>::new();
    for (url, page) in pages {
        if let Some(value) = field(page) {
            groups.entry(value).or_default().push((url.as_str(), page));
        }
    }
    groups.into_values().filter(|group| group.len() > 1)
}

fn probes<'a>(
    site: &Url,
    robots: Option<&Robots>,
    pages: &BTreeMap<String, Page>,
    urls: impl Iterator<Item = &'a str>,
) -> Result<(BTreeMap<&'a str, Target>, usize)> {
    let urls: BTreeSet<_> = urls
        .filter(|url| !pages.contains_key(*url))
        .filter(|url| {
            Url::parse(url).is_ok_and(|target| {
                target.origin() == site.origin()
                    && robots.is_some_and(|rules| allowed(rules, &target))
            })
        })
        .collect();
    let mut unverified = urls.len();
    let mut targets = BTreeMap::new();
    let selected: Vec<_> = urls.into_iter().take(MAX_TARGETS).collect();
    for batch in selected.chunks(40) {
        if deadline() < 4_000 {
            break;
        }
        let requests: Vec<_> = batch.iter().map(|target| request(target, 1)).collect();
        for (&target, response) in batch.iter().zip(http::fetch_all(&requests)?) {
            if let Ok(response) = response {
                let html = response
                    .header("content-type")
                    .and_then(|value| value.split(';').next())
                    .map(str::trim)
                    .is_some_and(|mime| {
                        mime.eq_ignore_ascii_case("text/html")
                            || mime.eq_ignore_ascii_case("application/xhtml+xml")
                    });
                targets.insert(
                    target,
                    Target {
                        status: Some(response.status),
                        html,
                        checked: true,
                    },
                );
                unverified -= 1;
            }
        }
    }
    Ok((targets, unverified))
}

fn bounded(findings: impl IntoIterator<Item = Finding>) -> Result<(Vec<Finding>, bool)> {
    let mut remaining = MAX_FINDINGS_BYTES;
    let mut result = Vec::new();
    for finding in findings {
        let size = serde_json::to_vec(&finding)?.len() + 1;
        if size > remaining {
            return Ok((result, true));
        }
        remaining -= size;
        result.push(finding);
    }
    Ok((result, false))
}

impl Analysis {
    fn compare(
        &mut self,
        previous: Option<&(Report, Vec<Finding>)>,
        pages: &BTreeMap<String, Page>,
        targets: &BTreeMap<&str, Target>,
    ) -> Result<()> {
        let Some((previous, unresolved)) = previous else {
            return Ok(());
        };
        let mut past = BTreeMap::new();
        for finding in unresolved.iter().chain(&previous.findings) {
            past.insert(finding.key(), finding);
        }
        let current: BTreeSet<_> = self.report.findings.iter().map(Finding::key).collect();
        self.report.new = self
            .report
            .findings
            .iter()
            .filter(|finding| !past.contains_key(&finding.key()))
            .cloned()
            .collect();

        let mut awaiting = Vec::new();
        for (key, finding) in past {
            if current.contains(&key) {
                continue;
            }
            let Some(page) = pages.get(&finding.page) else {
                continue;
            };
            let related_checked = |scan: bool| {
                finding.related.as_deref().is_some_and(|url| {
                    pages.get(url).map_or_else(
                        || !scan && targets.get(url).is_some_and(|target| target.checked),
                        |page| {
                            if scan {
                                page.confirmed()
                            } else {
                                page.checked && !page.blocked && page.status.is_some()
                            }
                        },
                    )
                })
            };
            let fixed = !self.report.findings_truncated
                && match finding.kind {
                    Kind::SitemapDisallows => {
                        self.report.robots_read && page.checked && !page.blocked
                    }
                    Kind::PageRedirect | Kind::PageUnavailable => {
                        page.checked
                            && !page.blocked
                            && page.status == Some(200)
                            && page.problem.is_none()
                    }
                    Kind::Metadata(_) => page.confirmed(),
                    Kind::DuplicateTitle | Kind::DuplicateDescription => {
                        page.confirmed() && related_checked(true)
                    }
                    Kind::CanonicalRedirect | Kind::CanonicalError | Kind::BrokenLink => {
                        page.confirmed() && related_checked(false)
                    }
                    Kind::MissingFromSitemap => {
                        page.confirmed() && related_checked(false) && self.report.sitemap_complete
                    }
                };
            let mut finding = finding.clone();
            finding.stale = !fixed;
            if fixed {
                self.report.fixed.push(finding);
            } else {
                awaiting.push(finding);
            }
        }
        let (fixed, fixes_truncated) = bounded(std::mem::take(&mut self.report.fixed))?;
        self.report.fixed = fixed;
        let (unresolved, unresolved_truncated) = bounded(awaiting)?;
        self.unresolved = unresolved;
        self.report.findings_truncated |= fixes_truncated || unresolved_truncated;
        Ok(())
    }
}

/// Analyze audit findings
#[tool(
    access = "internal",
    budget(cpu = 100000, read = 1000000, written = 4, memory = 128)
)]
pub(crate) fn audit_analyze(
    site: String,
    checked: Vec<CheckRange>,
    cursor: Option<String>,
) -> Result<AuditSaveCall> {
    let run = Run::owns(&site)?;
    let target = Url::parse(&site).map_err(|_| "Invalid audited site")?;
    let (robots, sitemap_complete) = caller_storage::rows::<(Option<String>, u8)>(
        "SELECT robots, sitemap_complete FROM runs WHERE site = ?1",
        &(&site,),
    )?
    .pop()
    .ok_or("The audit run is missing")?;
    let robots = robots.as_deref().map(Robots::from);
    let sitemap_complete = sitemap_complete != 0;
    let pages = pages(&site, &run, &checked)?;
    let links = links(&site)?;
    let previous: Option<(Report, Vec<Finding>)> = caller_storage::rows::<(String, String)>(
        "SELECT report, unresolved FROM reports WHERE site = ?1",
        &(&site,),
    )?
    .pop()
    .map(|(report, unresolved)| -> Result<_> {
        Ok((
            serde_json::from_str(&report)?,
            serde_json::from_str(&unresolved)?,
        ))
    })
    .transpose()?;

    let prior_targets = previous.iter().flat_map(|(report, unresolved)| {
        report
            .findings
            .iter()
            .chain(unresolved)
            .filter(|finding| {
                matches!(
                    finding.kind,
                    Kind::MissingFromSitemap
                        | Kind::BrokenLink
                        | Kind::CanonicalError
                        | Kind::CanonicalRedirect
                )
            })
            .filter_map(|finding| finding.related.as_deref())
    });
    let (targets, unverified_targets) = probes(
        &target,
        robots.as_ref(),
        &pages,
        pages
            .values()
            .filter_map(|page| page.canonical.as_deref())
            .chain(links.iter().map(|(_, link)| link.as_str()))
            .chain(prior_targets),
    )?;
    let check_target = |url: &str| {
        if let Some(page) = pages.get(url) {
            return Target {
                status: if page.blocked { None } else { page.status },
                html: page.status == Some(200) && page.problem.is_none(),
                checked: page.checked,
            };
        }
        targets
            .get(url)
            .map_or_else(Target::default, |answer| Target {
                status: answer.status,
                html: answer.html,
                checked: answer.checked,
            })
    };

    let mut findings = Vec::new();
    for (url, page) in &pages {
        if robots.is_some() && page.blocked {
            findings.push(Finding::new(Kind::SitemapDisallows, url, None, false));
        } else if matches!(page.status, Some(300..=399)) && page.problem.is_none() {
            let mut finding = Finding::new(
                Kind::PageRedirect,
                url,
                page.redirect.as_deref(),
                !page.checked,
            );
            finding.status = page.status;
            findings.push(finding);
        } else if let Some(problem) = &page.problem {
            let mut finding = Finding::new(Kind::PageUnavailable, url, None, !page.checked);
            finding.status = page.status;
            finding.detail = Some(problem.clone());
            findings.push(finding);
        }
        for code in &page.metadata {
            findings.push(Finding::new(
                Kind::Metadata(*code),
                url,
                None,
                !page.audited,
            ));
        }
        if let Some(canonical) = &page.canonical {
            let answer = check_target(canonical);
            let kind = match answer.status {
                Some(300..=399) => Some(Kind::CanonicalRedirect),
                Some(400..) => Some(Kind::CanonicalError),
                _ => None,
            };
            if let Some(kind) = kind {
                let mut finding =
                    Finding::new(kind, url, Some(canonical), !page.audited || !answer.checked);
                finding.status = answer.status;
                findings.push(finding);
            }
        }
    }
    for (kind, groups) in [
        (
            Kind::DuplicateTitle,
            duplicates(
                &pages,
                (|page: &Page| page.title.as_deref()) as fn(&Page) -> Option<&str>,
            ),
        ),
        (
            Kind::DuplicateDescription,
            duplicates(
                &pages,
                (|page: &Page| page.description.as_deref()) as fn(&Page) -> Option<&str>,
            ),
        ),
    ] {
        for group in groups {
            for (index, (url, page)) in group.iter().enumerate() {
                let (other, other_page) = group[usize::from(index == 0)];
                findings.push(Finding::new(
                    kind,
                    url,
                    Some(other),
                    !page.audited || !other_page.audited,
                ));
            }
        }
    }
    for (url, link) in &links {
        let answer = check_target(link);
        let kind = if matches!(answer.status, Some(400..)) {
            Kind::BrokenLink
        } else if sitemap_complete
            && answer.html
            && matches!(answer.status, Some(200..=299))
            && !pages.contains_key(link)
            && Url::parse(link).is_ok_and(|url| within(&target, &url))
        {
            Kind::MissingFromSitemap
        } else {
            continue;
        };
        let mut finding = Finding::new(
            kind,
            url,
            Some(link),
            !pages[url].audited || !answer.checked,
        );
        finding.status = answer.status;
        findings.push(finding);
    }
    findings.sort_unstable_by(|a, b| a.key().cmp(&b.key()));
    let (findings, findings_truncated) = bounded(findings)?;
    let (listed, pending) = caller_storage::rows::<(usize, usize)>(
        "SELECT count(*), count(CASE WHEN NOT blocked AND status = 200
                                     AND checked_fields IS NOT NULL THEN 1 END)
         FROM pages WHERE site = ?1 AND url NOT IN (SELECT url FROM unlisted_pages WHERE site = ?1)",
        &(&site,),
    )?
    .pop()
    .ok_or("Could not count sitemap pages")?;
    let mut analysis = Analysis {
        report: Report {
            new: Vec::new(),
            fixed: Vec::new(),
            findings,
            listed,
            checked: pages.values().filter(|page| page.checked).count(),
            audited: pages.values().filter(|page| page.audited).count(),
            pending,
            site: site.clone(),
            sitemap_complete,
            robots_read: robots.is_some(),
            unverified_targets,
            findings_truncated,
        },
        cursor,
        unresolved: Vec::new(),
    };
    analysis.compare(previous.as_ref(), &pages, &targets)?;
    caller_storage::rows::<()>(
        "UPDATE runs SET analysis = ?2 WHERE site = ?1",
        &(&site, serde_json::to_string(&analysis)?),
    )?;
    Ok(AuditSaveCall {
        site,
        idempotency_key: run.id,
    })
}

/// Save the audit report
#[tool(
    access = "internal",
    idempotency = "2m",
    budget(cpu = 100000, read = 64, written = 8, memory = 64)
)]
pub(crate) fn audit_save(site: String, idempotency_key: String) -> Result<Report> {
    let run = Run::current_id()?;
    if idempotency_key != run {
        return Err("Audit save key does not match the workflow run".into());
    }
    let now = now();
    let cutoff = now.saturating_sub(SAVE_RETRY_MS);
    if let Some((Some(report),)) = caller_storage::rows::<(Option<String>,)>(
        "SELECT CASE WHEN run = ?2 THEN report
                    WHEN json_extract(receipts, '$.\"' || ?2 || '\".at') >= ?3
                    THEN json_extract(receipts, '$.\"' || ?2 || '\".report') END
         FROM reports WHERE site = ?1",
        &(&site, &run, cutoff),
    )?
    .pop()
    {
        return Ok(serde_json::from_str(&report)?);
    }
    let (analysis,) = caller_storage::rows::<(String,)>(
        "SELECT analysis FROM runs WHERE site = ?1 AND run = ?2",
        &(&site, &run),
    )?
    .pop()
    .ok_or("This audit no longer holds the site")?;
    let Analysis {
        report,
        cursor,
        unresolved,
    } = serde_json::from_str(&analysis)?;
    caller_storage::rows::<()>(
        "INSERT INTO reports (site, run, report, unresolved, saved_at, cursor)
         VALUES (?1, ?2, ?3, ?4, ?5, ?7)
         ON CONFLICT (site) DO UPDATE SET run = excluded.run, report = excluded.report,
             unresolved = excluded.unresolved, saved_at = excluded.saved_at,
             cursor = coalesce(excluded.cursor, reports.cursor),
             receipts = (
                 SELECT json_group_object(key, json(value)) FROM (
                     SELECT key, value FROM json_each(reports.receipts) WHERE value ->> '$.at' >= ?6
                     UNION ALL
                     SELECT reports.run, json_object('at', reports.saved_at, 'report', json(reports.report))
                     WHERE reports.saved_at >= ?6
                 )
             )",
        &(
            &site,
            &run,
            serde_json::to_string(&report)?,
            serde_json::to_string(&unresolved)?,
            now,
            cutoff,
            cursor,
        ),
    )?;
    caller_storage::rows::<()>(
        "DELETE FROM runs WHERE site = ?1 AND run = ?2",
        &(&site, &run),
    )?;
    Ok(report)
}
