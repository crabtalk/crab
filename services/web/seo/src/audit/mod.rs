use berm_lang::{Schema, workflow};

mod discovery;
mod page;
mod report;

use crate::audit::{
    discovery::{AuditDiscoverCall, Discovery},
    page::{AuditPageCall, PageScan},
    report::{AuditAnalyzeCall, AuditSaveCall, Report},
};

const MAX_PAGES: usize = 100;
const MAX_URL_BYTES: usize = 512;
const AGENT: &str = "CrabTalkSEO";
const HTTP_TIMEOUT: u64 = 2_500;

#[derive(Schema)]
struct Site {
    /// A public HTTP or HTTPS URL. A path limits the audit to that section.
    site: String,
}

struct Audit;

/// Audit site SEO
#[workflow(budget(cpu = 100000, read = 0, written = 0, memory = 64))]
impl Audit {
    #[step]
    fn discover(input: &Site) -> AuditDiscoverCall {
        AuditDiscoverCall {
            site: input.site.clone(),
        }
    }

    #[step(each, max = 100, price = 100)]
    fn pages(discover: &Discovery) -> Vec<AuditPageCall> {
        discover
            .sample
            .iter()
            .filter(|page| !page.blocked)
            .map(|page| AuditPageCall {
                url: page.url.clone(),
            })
            .collect()
    }

    #[step]
    fn analyze(discover: &Discovery, pages: &[PageScan]) -> AuditAnalyzeCall {
        AuditAnalyzeCall {
            discovery: discover.clone(),
            pages: pages.to_vec(),
        }
    }

    #[step]
    fn save(analyze: &Report) -> AuditSaveCall {
        AuditSaveCall {
            report: analyze.clone(),
        }
    }

    #[output]
    fn done(save: &Report) -> Report {
        save.clone()
    }
}
