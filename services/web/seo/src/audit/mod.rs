use crate::audit::{
    check::AuditCheckCall,
    discovery::{AuditDiscoverCall, CheckPlan},
    page::{AuditPageCall, AuditSelectCall, Selection},
    report::{AuditAnalyzeCall, AuditSaveCall, Report},
    run::AuditStartCall,
};
use berm_lang::{Schema, workflow};

mod check;
mod discovery;
mod page;
mod report;
mod run;

const MAX_PAGES: usize = 100;
const MAX_CHECKED: usize = 1_000;
const CHECK_BATCH: usize = 40;
const MAX_URL_BYTES: usize = 512;
const AGENT: &str = "CrabTalkSEO";

#[derive(Schema)]
pub(crate) struct Input {
    /// Absolute HTTP or HTTPS URL. A path limits the audit to that section.
    site: String,
    /// Sitemap URLs known to have changed. Pending URLs in this list go first.
    pages: Option<Vec<String>>,
    /// Maximum pages to audit. Defaults to 100 and is capped at 100.
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

    #[step(each, max = 100, price = 100)]
    fn audit(select: &Selection) -> Vec<AuditPageCall> {
        select
            .urls
            .iter()
            .map(|url| AuditPageCall {
                site: select.site.clone(),
                url: url.clone(),
            })
            .collect()
    }

    #[step]
    fn analyze(select: &Selection, audit: &[bool]) -> AuditAnalyzeCall {
        let _ = audit;
        AuditAnalyzeCall {
            site: select.site.clone(),
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
