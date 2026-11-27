use crate::audit::{
    check::{AuditCheckCall, CheckRange},
    discovery::{AuditDiscoverCall, CheckPlan},
    page::{AuditBatchCall, AuditSelectCall, Selection},
    report::{AuditAnalyzeCall, AuditSaveCall, Report},
    run::AuditStartCall,
};
use berm_lang::{Schema, workflow};
use std::collections::BTreeSet;

mod check;
mod discovery;
mod markdown;
mod page;
mod report;
mod run;

const MAX_PAGES: usize = 1_000;
const AUDIT_BATCH: usize = 10;
const MAX_CHECKED: usize = 1_000;
const CHECK_BATCH: usize = 40;
const MAX_URL_BYTES: usize = 512;
const AGENT: &str = "CrabTalkSEO";
const CHECK_TIMEOUT: u64 = 1_800;
const FINISH_BUDGET_MS: u64 = 12_000;

#[derive(Schema)]
pub(crate) struct Input {
    /// Absolute HTTP or HTTPS URL. A path limits the audit to that section.
    site: String,
    /// Sitemap URLs known to have changed. Pending URLs in this list go first.
    pages: Option<Vec<String>>,
    /// Maximum pages to audit. Defaults to 1,000 and is capped at 1,000.
    max: Option<usize>,
}

#[derive(Clone, Schema)]
pub(crate) struct Site {
    site: String,
}

struct Audit;

/// Audit site SEO
#[workflow(budget(cpu = 100000, read = 0, written = 0, memory = 64))]
impl Audit {
    #[step]
    fn start(input: &Input) -> AuditStartCall {
        AuditStartCall {
            site: input.site.clone(),
        }
    }

    #[step]
    fn discover(start: &Site) -> AuditDiscoverCall {
        AuditDiscoverCall {
            site: start.site.clone(),
        }
    }

    #[step(each, max = 25)]
    fn check(discover: &CheckPlan) -> Vec<AuditCheckCall> {
        discover
            .urls
            .chunks(CHECK_BATCH)
            .map(|urls| AuditCheckCall {
                site: discover.site.clone(),
                urls: urls.to_vec(),
            })
            .collect()
    }

    #[step]
    fn select(input: &Input, discover: &CheckPlan, check: &[bool]) -> AuditSelectCall {
        let _ = check;
        AuditSelectCall {
            site: discover.site.clone(),
            pages: input.pages.clone().unwrap_or_default(),
            max: input.max.unwrap_or(MAX_PAGES).min(MAX_PAGES),
        }
    }

    #[step(each, max = 100)]
    fn audit(select: &Selection, discover: &CheckPlan, check: &[bool]) -> Vec<AuditBatchCall> {
        let checked: BTreeSet<_> = discover
            .urls
            .chunks(CHECK_BATCH)
            .zip(check)
            .filter(|(_, completed)| **completed)
            .flat_map(|(urls, _)| urls.iter().map(String::as_str))
            .collect();
        select
            .urls
            .chunks(AUDIT_BATCH)
            .map(|urls| AuditBatchCall {
                site: select.site.clone(),
                urls: urls.to_vec(),
                checked: urls
                    .iter()
                    .filter(|url| checked.contains(url.as_str()))
                    .cloned()
                    .collect(),
            })
            .collect()
    }

    #[step]
    fn analyze(discover: &CheckPlan, check: &[bool], audit: &[bool]) -> AuditAnalyzeCall {
        let _ = audit;
        let mut checked = Vec::new();
        let mut cursor = None;
        let mut contiguous = true;
        for (urls, &completed) in discover.urls.chunks(CHECK_BATCH).zip(check) {
            contiguous &= completed;
            if completed {
                let last = urls.last().expect("nonempty check batch").clone();
                if contiguous {
                    cursor = Some(last.clone());
                }
                checked.push(CheckRange {
                    first: urls[0].clone(),
                    last,
                });
            }
        }
        AuditAnalyzeCall {
            site: discover.site.clone(),
            checked,
            cursor,
        }
    }

    #[step]
    fn save(analyze: &AuditSaveCall) -> AuditSaveCall {
        AuditSaveCall {
            site: analyze.site.clone(),
            idempotency_key: analyze.idempotency_key.clone(),
        }
    }

    #[output]
    fn done(save: &Report) -> Report {
        save.clone()
    }
}
