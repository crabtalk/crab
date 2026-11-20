use crate::{
    audit::{
        discovery::{Discovery, allowed, request, within},
        page::PageScan,
    },
    crawlers::Robots,
    report::Code,
};
use berm_lang::{
    Result, Schema, http, rows, serde_json, sql, tool,
    workflow::{self, Trigger},
};
use std::collections::{BTreeMap, BTreeSet};
use url::Url;

const MAX_TARGETS: usize = 100;
const MAX_FINDINGS_BYTES: usize = 96 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Schema)]
enum Kind {
    SitemapDisallows,
    PageUnavailable,
    DuplicateTitle,
    DuplicateDescription,
    CanonicalRedirect,
    CanonicalError,
    BrokenLink,
    MissingFromSitemap,
    Metadata(Code),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Schema)]
struct Finding {
    kind: Kind,
    page: String,
    related: Option<String>,
}

#[derive(Clone, Schema)]
pub(crate) struct Report {
    site: String,
    new: Vec<Finding>,
    fixed: Vec<Finding>,
    findings: Vec<Finding>,
    sitemap_complete: bool,
    robots_read: bool,
    pages_capped: bool,
    pages_audited: usize,
    sample: Vec<String>,
    links_sampled: bool,
    targets_capped: bool,
    unverified_targets: usize,
    findings_truncated: bool,
    history_updated: bool,
}

#[derive(Schema)]
struct Previous {
    run: String,
    sample: Vec<String>,
    findings: Vec<Finding>,
    new: Vec<Finding>,
    fixed: Vec<Finding>,
}

#[derive(Clone, Copy)]
struct Response {
    status: u16,
    html: bool,
}

impl Response {
    fn successful(self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Analyze site SEO
///
/// Compares page metadata and checks canonical and internal link targets.
#[tool(
    query,
    access = "internal",
    budget(cpu = 100000, read = 1, written = 1, memory = 64)
)]
pub(crate) fn audit_analyze(discovery: Discovery, pages: Vec<PageScan>) -> Result<Report> {
    let site = Url::parse(&discovery.site).map_err(|_| "Invalid audited site")?;
    let robots = Robots::from(discovery.robots.as_str());
    let mut findings = BTreeSet::new();
    let mut record = |kind, page: &str, related: Option<&str>| {
        findings.insert(Finding {
            kind,
            page: page.to_owned(),
            related: related.map(str::to_owned),
        });
    };
    if discovery.robots_read {
        for page in discovery.sample.iter().filter(|page| page.blocked) {
            record(Kind::SitemapDisallows, &page.url, None);
        }
    }
    for page in &pages {
        if page.problem.is_some() {
            record(Kind::PageUnavailable, &page.url, None);
        }
        for code in &page.metadata {
            record(Kind::Metadata(*code), &page.url, None);
        }
    }

    let duplicates = |field: fn(&PageScan) -> Option<&str>| {
        let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for page in &pages {
            if let Some(value) = field(page) {
                groups.entry(value).or_default().push(page.url.as_str());
            }
        }
        groups.into_values().filter(|urls| urls.len() > 1)
    };
    for (kind, groups) in [
        (
            Kind::DuplicateTitle,
            duplicates(|page| page.title.as_deref()),
        ),
        (
            Kind::DuplicateDescription,
            duplicates(|page| page.description.as_deref()),
        ),
    ] {
        for urls in groups {
            for (index, url) in urls.iter().enumerate() {
                let other = urls[usize::from(index == 0)];
                record(kind, url, Some(other));
            }
        }
    }

    let mut responses: BTreeMap<&str, Response> = pages
        .iter()
        .filter_map(|page| {
            Some((
                page.url.as_str(),
                Response {
                    status: page.status?,
                    html: page.problem.is_none(),
                },
            ))
        })
        .collect();
    let targets: BTreeSet<_> = pages
        .iter()
        .flat_map(|page| page.canonical.iter().chain(&page.links).map(String::as_str))
        .filter(|target| !responses.contains_key(target))
        .filter(|target| {
            Url::parse(target).is_ok_and(|url| {
                discovery.robots_read && url.origin() == site.origin() && allowed(&robots, &url)
            })
        })
        .collect();
    let targets_capped = targets.len() > MAX_TARGETS;
    let selected: Vec<_> = targets.into_iter().take(MAX_TARGETS).collect();
    let mut unverified_targets = 0;
    for batch in selected.chunks(16) {
        let requests: Vec<_> = batch.iter().map(|target| request(target, 1)).collect();
        for (target, answer) in batch.iter().zip(http::fetch_all(&requests)?) {
            match answer {
                Ok(answer) => {
                    let html = answer
                        .header("content-type")
                        .unwrap_or_default()
                        .split(';')
                        .next()
                        .is_some_and(|mime| {
                            let mime = mime.trim();
                            mime.eq_ignore_ascii_case("text/html")
                                || mime.eq_ignore_ascii_case("application/xhtml+xml")
                        });
                    responses.insert(
                        target,
                        Response {
                            status: answer.status,
                            html,
                        },
                    );
                }
                Err(_) => unverified_targets += 1,
            }
        }
    }

    let listed: BTreeSet<_> = discovery.listed.iter().map(String::as_str).collect();
    for page in &pages {
        if let Some(target) = &page.canonical
            && let Some(answer) = responses.get(target.as_str())
        {
            let kind = match answer.status {
                300..400 => Some(Kind::CanonicalRedirect),
                400.. => Some(Kind::CanonicalError),
                _ => None,
            };
            if let Some(kind) = kind {
                record(kind, &page.url, Some(target));
            }
        }
        for target in &page.links {
            let Some(answer) = responses.get(target.as_str()) else {
                continue;
            };
            if answer.status >= 400 {
                record(Kind::BrokenLink, &page.url, Some(target));
            } else if discovery.sitemap_complete
                && answer.successful()
                && answer.html
                && !listed.contains(target.as_str())
                && Url::parse(target).is_ok_and(|url| within(&site, &url))
            {
                record(Kind::MissingFromSitemap, &page.url, Some(target));
            }
        }
    }

    let mut remaining = MAX_FINDINGS_BYTES;
    let mut findings_truncated = false;
    let mut bounded = Vec::new();
    for finding in findings {
        let bytes = serde_json::to_vec(&finding)?.len() + 1;
        if bytes > remaining {
            findings_truncated = true;
            break;
        }
        remaining -= bytes;
        bounded.push(finding);
    }
    let sample = discovery
        .sample
        .into_iter()
        .map(|page| page.url)
        .collect::<Vec<_>>();
    Ok(Report {
        site: discovery.site,
        new: Vec::new(),
        fixed: Vec::new(),
        findings: bounded,
        sitemap_complete: discovery.sitemap_complete,
        robots_read: discovery.robots_read,
        pages_capped: discovery.listed.len() > sample.len(),
        pages_audited: pages.len(),
        sample,
        links_sampled: pages.iter().any(|page| page.links_truncated),
        targets_capped,
        unverified_targets,
        findings_truncated,
        history_updated: false,
    })
}

/// Compare and save audit findings
///
/// Stores the previous report for the workflow's starting account and site.
#[tool(
    access = "internal",
    budget(cpu = 100000, read = 8, written = 2, memory = 64)
)]
pub(crate) fn audit_save(mut report: Report) -> Result<Report> {
    let Some(run) = workflow::run()? else {
        return Ok(report);
    };
    let Trigger::Manual { account, user, .. } = run.trigger else {
        return Ok(report);
    };
    let user = serde_json::to_string(&user)?;
    let previous = rows::<(String,)>(
        "SELECT snapshot FROM audits WHERE account = ?1 AND user = ?2 AND site = ?3",
        &(&account, &user, &report.site),
    )?
    .into_iter()
    .next()
    .map(|(snapshot,)| serde_json::from_str::<Previous>(&snapshot))
    .transpose()?;
    if let Some(previous) = &previous
        && previous.run == run.run
    {
        report.new = previous.new.clone();
        report.fixed = previous.fixed.clone();
        report.history_updated = true;
        return Ok(report);
    }
    let current: BTreeSet<_> = report.findings.iter().collect();
    let past: BTreeSet<_> = previous
        .iter()
        .flat_map(|previous| &previous.findings)
        .collect();
    report.new = current
        .difference(&past)
        .map(|finding| (**finding).clone())
        .collect();

    if report.sitemap_complete
        && report.robots_read
        && report.unverified_targets == 0
        && !report.targets_capped
        && !report.findings_truncated
        && report
            .findings
            .iter()
            .all(|finding| finding.kind != Kind::PageUnavailable)
    {
        if previous
            .as_ref()
            .is_some_and(|previous| previous.sample == report.sample)
        {
            report.fixed = past
                .difference(&current)
                .filter(|finding| {
                    !report.links_sampled
                        || !matches!(finding.kind, Kind::BrokenLink | Kind::MissingFromSitemap)
                })
                .map(|finding| (**finding).clone())
                .collect();
        }
        let snapshot = serde_json::to_string(&Previous {
            run: run.run,
            sample: report.sample.clone(),
            findings: report.findings.clone(),
            new: report.new.clone(),
            fixed: report.fixed.clone(),
        })?;
        sql(
            "INSERT INTO audits (account, user, site, snapshot) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (account, user, site) DO UPDATE SET snapshot = excluded.snapshot",
            &serde_json::to_string(&(&account, &user, &report.site, snapshot))?,
        )?;
        report.history_updated = true;
    }
    Ok(report)
}
