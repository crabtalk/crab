use crate::audit::{Site, discovery::web_url};
use berm_lang::{Result, caller_storage, host, now, tool, workflow};
use url::Url;

// An abandoned attempt holds the site for at most two minutes. The host
// limits an attempt to 60 seconds, including the time between steps.
const LEASE_MS: u64 = 120_000;

pub(super) struct Run {
    pub id: String,
}

impl Run {
    fn current() -> Result<Self> {
        let run = workflow::run()?.ok_or("This tool requires a workflow run")?;
        Ok(Self { id: run.run })
    }

    pub fn owns(site: &str) -> Result<Self> {
        let run = Self::current()?;
        let Some((expires_at,)) = caller_storage::rows::<(u64,)>(
            "SELECT expires_at FROM runs WHERE site = ?1 AND run = ?2",
            &(site, &run.id),
        )?
        .pop() else {
            return Err("This audit no longer holds the site".into());
        };
        let now = now();
        if expires_at <= now + LEASE_MS / 2 {
            caller_storage::rows::<()>(
                "UPDATE runs SET expires_at = ?3 WHERE site = ?1 AND run = ?2",
                &(site, &run.id, now + LEASE_MS),
            )?;
        }
        Ok(run)
    }

    pub fn current_id() -> Result<String> {
        Ok(Self::current()?.id)
    }
}

/// Start a site audit
#[tool(
    access = "internal",
    budget(cpu = 10000, read = 16, written = 4, memory = 8)
)]
pub(crate) fn audit_start(site: String) -> Result<Site> {
    let mut site = Url::parse(&site)
        .ok()
        .and_then(web_url)
        .ok_or("Provide a public HTTP or HTTPS site URL of at most 512 bytes")?;
    site.set_query(None);
    let site = site.to_string();
    let run = Run::current()?;
    let now = now();
    let acquired = caller_storage::rows::<(String,)>(
        "INSERT INTO runs (site, run, expires_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (site) DO UPDATE SET run = excluded.run, expires_at = excluded.expires_at,
           robots = NULL, sitemap_complete = 0, unlisted = NULL, analysis = NULL
         WHERE runs.run <> excluded.run AND runs.expires_at <= ?4
         RETURNING run",
        &(&site, &run.id, now + LEASE_MS, now),
    )?;
    if acquired.is_empty() {
        let updated = caller_storage::rows::<(String,)>(
            "UPDATE runs SET expires_at = ?3 WHERE site = ?1 AND run = ?2 RETURNING run",
            &(&site, &run.id, now + LEASE_MS),
        )?;
        if updated.is_empty() {
            host::deny();
            return Err("An audit of this site is already running.".into());
        }
    }
    Ok(Site { site })
}
