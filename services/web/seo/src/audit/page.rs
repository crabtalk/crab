use crate::{
    audit::{
        MAX_PAGES,
        discovery::{web_url, within},
        run::Run,
    },
    report::Report,
    structured::Block,
};
use berm_lang::{Result, Schema, caller_storage, now, serde, serde_json, tool};
use scraper::Selector;
use std::{collections::BTreeSet, io};
use url::Url;
use webpage::page::Page;

#[derive(serde::Serialize)]
#[serde(crate = "berm_lang::serde")]
pub(super) struct PageScan {
    metadata: Report,
    links: BTreeSet<String>,
    titles: Vec<String>,
    headings: Vec<String>,
    image_alts: Vec<Option<String>>,
    json_ld: Vec<(bool, serde_json::Value)>,
}

impl PageScan {
    pub fn inspect(page: &Page) -> Self {
        let selector = Selector::parse("a[href]").expect("static selector");
        let links = page
            .document
            .select(&selector)
            .filter_map(|element| {
                let link = page.base.join(element.value().attr("href")?).ok()?;
                let link = web_url(link)?;
                (link.origin() == page.url.origin() && link != page.url).then(|| link.into())
            })
            .collect();
        let selector = Selector::parse("h1, img").expect("static selector");
        let mut headings = Vec::new();
        let mut image_alts = Vec::new();
        for element in page.document.select(&selector) {
            if element.value().name() == "h1" {
                headings.push(element.text().collect::<String>().trim().to_owned());
            } else {
                image_alts.push(element.value().attr("alt").map(str::to_owned));
            }
        }
        let titles = page
            .titles()
            .map(|element| element.text().collect::<String>().trim().to_owned())
            .collect();
        let json_ld = Block::sources(page)
            .map(|source| match serde_json::from_str(&source) {
                Ok(value) => (true, value),
                Err(_) => (false, serde_json::Value::String(source.trim().to_owned())),
            })
            .collect();
        Self {
            metadata: Report::inspect(page),
            links,
            titles,
            headings,
            image_alts,
            json_ld,
        }
    }

    pub fn hash(&self) -> Result<String> {
        let report = &self.metadata;
        let mut hash = FieldHash(0xcbf29ce484222325);
        serde_json::to_writer(
            &mut hash,
            &(
                &self.titles,
                &report.descriptions,
                &self.headings,
                &report.canonicals,
                &report.robots_directives,
                &report.hreflang,
                &self.json_ld,
                &report.social.properties,
                &self.image_alts,
                &self.links,
                &report.language,
            ),
        )?;
        Ok(format!("{:016x}", hash.0))
    }
}

// Fingerprints are persisted across builds, so the algorithm is fixed.
struct FieldHash(u64);

impl io::Write for FieldHash {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Schema)]
pub(crate) struct Selection {
    pub site: String,
    pub urls: Vec<String>,
}

/// Select pending sitemap pages
#[tool(
    access = "internal",
    budget(cpu = 100000, read = 1000000, written = 4, memory = 32)
)]
pub(crate) fn audit_select(site: String, pages: Vec<String>, max: usize) -> Result<Selection> {
    let run = Run::owns(&site)?;
    let target = Url::parse(&site).map_err(|_| "Invalid audited site")?;
    let max = max.min(MAX_PAGES);
    let mut seen = BTreeSet::new();
    let pages: Vec<_> = pages
        .into_iter()
        .filter_map(|url| Url::parse(&url).ok().and_then(web_url))
        .filter(|url| within(&target, url) && seen.insert(url.clone()))
        .map(String::from)
        .collect();
    let urls = caller_storage::rows::<(String,)>(
        "WITH hints AS (
            SELECT value AS url, key AS position FROM json_each(?3)
         ), pending AS MATERIALIZED (
            SELECT p.url, p.scan_hash, p.scanned_at, hints.position
            FROM pages p LEFT JOIN hints USING (url)
            WHERE p.site = ?1 AND p.listed_run = ?2 AND NOT p.blocked
                AND p.status = 200 AND p.checked_fields IS NOT NULL
                AND (SELECT robots IS NOT NULL FROM runs WHERE site = ?1)
         ), incoming AS MATERIALIZED (
            SELECT link.value AS url, count(*) AS links
            FROM pages p, json_each(coalesce(p.checked_fields, p.scan), '$.links') link
            JOIN pending target ON target.url = link.value AND target.scan_hash IS NOT NULL
            WHERE p.site = ?1 AND p.listed_run = ?2 AND NOT p.blocked AND p.status = 200
            GROUP BY link.value
         )
         SELECT p.url FROM pending p LEFT JOIN incoming i USING (url)
         ORDER BY
           CASE WHEN p.position IS NOT NULL THEN 0 WHEN p.scan_hash IS NULL THEN 1
                WHEN i.links IS NOT NULL THEN 2 ELSE 3 END,
           p.position, i.links DESC, p.scanned_at, p.url
         LIMIT ?4",
        &(&site, &run.id, serde_json::to_string(&pages)?, max),
    )?
    .into_iter()
    .map(|(url,)| url)
    .collect();
    Ok(Selection { site, urls })
}

/// Audit a pending HTML page
#[tool(
    access = "internal",
    budget(cpu = 50000, read = 16, written = 4, memory = 16)
)]
pub(crate) fn audit_page(site: String, url: String) -> Result<bool> {
    let run = Run::owns(&site)?;
    let changed = caller_storage::rows::<(String,)>(
        "UPDATE pages SET scan = checked_fields, scan_hash = hash,
             scanned_at = ?4, scanned_run = ?3, checked_fields = NULL
         WHERE site = ?1 AND url = ?2 AND NOT blocked AND status = 200
             AND checked_fields IS NOT NULL
         RETURNING url",
        &(&site, &url, &run.id, now()),
    )?;
    if changed.is_empty()
        && caller_storage::rows::<(String,)>(
            "SELECT url FROM pages WHERE site = ?1 AND url = ?2 AND scanned_run = ?3",
            &(&site, &url, &run.id),
        )?
        .is_empty()
    {
        return Err("This page has no pending HTML check".into());
    }
    Ok(true)
}
