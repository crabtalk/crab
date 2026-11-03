use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use berm_lang::{Result, Schema, http};
use url::Url;

const MAX_MONITORS: u64 = 20;
const BATCH: u64 = 5;
const TIMEOUT: u64 = 5000;
const MINUTE: u64 = 60_000;
const RETENTION: u64 = 30 * 86_400_000;

#[derive(Schema)]
pub struct Monitor {
    pub name: String,
    pub url: String,
    pub interval_ms: u64,
    pub expected_status: Option<u16>,
    pub checked_at: Option<u64>,
    pub state: String,
    pub failures: u64,
    pub status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Schema)]
pub struct Status {
    pub name: String,
    pub state: String,
    pub checked_at: Option<u64>,
    pub latency_ms: Option<u64>,
}

#[derive(Schema)]
pub struct Check {
    pub seq: u64,
    pub checked_at: u64,
    pub ok: bool,
    pub status: Option<u16>,
    pub latency_ms: u64,
    pub error: Option<String>,
}

#[derive(Schema)]
pub struct History {
    pub items: Vec<Check>,
    pub next: u64,
    pub has_more: bool,
}

impl Monitor {
    pub fn watch(
        name: String,
        given: String,
        interval_minutes: Option<u64>,
        expected_status: Option<u16>,
    ) -> Result<Self> {
        if name.trim().is_empty() || name.len() > 64 {
            return Err("A monitor name must be 1–64 bytes".into());
        }
        let mut url = Url::parse(&given).map_err(|_| "Provide an absolute HTTP or HTTPS URL")?;
        if given.len() > 4096
            || url.as_str().len() > 4096
            || !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(
                "Use an HTTP or HTTPS URL up to 4096 bytes without embedded credentials".into(),
            );
        }
        url.set_fragment(None);
        let minutes = interval_minutes.unwrap_or(5);
        if !(5..=1440).contains(&minutes) {
            return Err("The check interval must be 5–1440 minutes".into());
        }
        if expected_status.is_some_and(|status| !(100..=599).contains(&status)) {
            return Err("Expected status must be an HTTP status from 100 to 599".into());
        }
        let previous = Self::get(&name)?;
        if previous.is_none() {
            let (count,) =
                berm_lang::rows::<(u64,)>("SELECT COUNT(*) FROM uptime_monitors", &[0u8; 0])?
                    .pop()
                    .unwrap_or_default();
            if count >= MAX_MONITORS {
                return Err("This copy supports twenty monitors".into());
            }
        }
        let changed = previous.as_ref().is_none_or(|monitor| {
            monitor.url != url.as_str() || monitor.expected_status != expected_status
        });
        if changed {
            berm_lang::rows::<()>("INSERT INTO uptime_monitors (name, url, interval_ms, expected_status, due_at) VALUES (?, ?, ?, ?, ?)
                ON CONFLICT (name) DO UPDATE SET url = excluded.url, interval_ms = excluded.interval_ms, expected_status = excluded.expected_status, due_at = excluded.due_at,
                checked_at = NULL, state = 'unknown', failures = 0, status = NULL, latency_ms = NULL, error = NULL", &(&name, url.as_str(), minutes * MINUTE, expected_status, berm_lang::now()))?;
            berm_lang::rows::<()>("DELETE FROM uptime_checks WHERE name = ?", &(&name,))?;
        } else {
            berm_lang::rows::<()>(
                "UPDATE uptime_monitors SET interval_ms = ? WHERE name = ?",
                &(minutes * MINUTE, &name),
            )?;
        }
        Self::get(&name)?.ok_or_else(|| "The monitor was not saved".into())
    }

    pub fn get(name: &str) -> Result<Option<Self>> {
        Ok(berm_lang::rows::<(String, u64, Option<u16>, Option<u64>, String, u64, Option<u16>, Option<u64>, Option<String>)>(
            "SELECT url, interval_ms, expected_status, checked_at, state, failures, status, latency_ms, error FROM uptime_monitors WHERE name = ?", &(name,))?
            .pop().map(|(url, interval_ms, expected_status, checked_at, state, failures, status, latency_ms, error)| Self { name: name.into(), url, interval_ms, expected_status, checked_at, state, failures, status, latency_ms, error }))
    }

    pub fn all() -> Result<Vec<Self>> {
        berm_lang::rows::<(String,)>("SELECT name FROM uptime_monitors ORDER BY name", &[0u8; 0])?
            .into_iter()
            .map(|(name,)| Self::get(&name)?.ok_or_else(|| "The monitor no longer exists".into()))
            .collect()
    }

    pub fn unwatch(name: &str) -> Result<bool> {
        berm_lang::rows::<()>("DELETE FROM uptime_checks WHERE name = ?", &(name,))?;
        Ok(!berm_lang::rows::<(String,)>(
            "DELETE FROM uptime_monitors WHERE name = ? RETURNING name",
            &(name,),
        )?
        .is_empty())
    }

    pub fn check(name: &str) -> Result<Self> {
        let monitor = Self::get(name)?.ok_or("No such monitor")?;
        monitor.observe()
    }

    pub fn tick() -> Result<u64> {
        let due = berm_lang::rows::<(String,)>(
            "SELECT name FROM uptime_monitors WHERE due_at <= ? ORDER BY due_at, name LIMIT ?",
            &(berm_lang::now(), BATCH),
        )?;
        let monitors = due
            .into_iter()
            .map(|(name,)| Self::get(&name)?.ok_or_else(|| "No such monitor".into()))
            .collect::<Result<Vec<_>>>()?;
        let count = monitors.len() as u64;
        for monitor in monitors {
            monitor.observe()?;
        }
        Ok(count)
    }

    fn observe(self) -> Result<Self> {
        let monitor = self;
        let started = berm_lang::now();
        let answer = http::fetch_all(&[http::Request {
            method: "GET",
            url: &monitor.url,
            headers: &[],
            body: &[],
            timeout: Some(TIMEOUT),
            limit: Some(1024),
        }])?
        .pop()
        .ok_or("The host returned no check result")?;
        let finished = berm_lang::now();
        let latency_ms = finished.saturating_sub(started);
        let (ok, status, error) = match answer {
            Ok(response) => {
                let ok = monitor.expected_status.map_or_else(
                    || response.is_success(),
                    |expected| expected == response.status,
                );
                (
                    ok,
                    Some(response.status),
                    (!ok).then(|| alloc::format!("HTTP {}", response.status)),
                )
            }
            Err(error) => (
                false,
                None,
                Some(error.to_string().chars().take(256).collect()),
            ),
        };
        let failures = if ok {
            0
        } else {
            monitor.failures.saturating_add(1)
        };
        let state = if ok {
            "up"
        } else if failures >= 2 {
            "down"
        } else {
            &monitor.state
        };
        berm_lang::rows::<()>(
            "INSERT INTO uptime_checks (name, checked_at, ok, status, latency_ms, error) VALUES (?, ?, ?, ?, ?, ?)",
            &(&monitor.name, finished, ok, status, latency_ms, &error),
        )?;
        berm_lang::rows::<()>(
            "UPDATE uptime_monitors SET checked_at = ?, state = ?, failures = ?, status = ?, latency_ms = ?, error = ?, due_at = ? WHERE name = ?",
            &(
                finished,
                state,
                failures,
                status,
                latency_ms,
                &error,
                finished.saturating_add(monitor.interval_ms),
                &monitor.name,
            ),
        )?;
        berm_lang::rows::<()>(
            "DELETE FROM uptime_checks WHERE name = ?1 AND (checked_at < ?2 OR seq NOT IN (SELECT seq FROM uptime_checks WHERE name = ?1 ORDER BY seq DESC LIMIT 1000))",
            &(&monitor.name, finished.saturating_sub(RETENTION)),
        )?;
        Self::get(&monitor.name)?.ok_or_else(|| "No such monitor".into())
    }

    pub fn history(name: &str, after: u64, limit: Option<u64>) -> Result<History> {
        let limit = limit.unwrap_or(20).clamp(1, 100);
        let mut found = berm_lang::rows::<(u64, u64, u8, Option<u16>, u64, Option<String>)>(
            "SELECT seq, checked_at, ok, status, latency_ms, error FROM uptime_checks WHERE name = ? AND seq > ? ORDER BY seq LIMIT ?",
            &(name, after, limit + 1),
        )?;
        let has_more = found.len() as u64 > limit;
        found.truncate(limit as usize);
        let items: Vec<_> = found
            .into_iter()
            .map(|(seq, checked_at, ok, status, latency_ms, error)| Check {
                seq,
                checked_at,
                ok: ok != 0,
                status,
                latency_ms,
                error,
            })
            .collect();
        Ok(History {
            next: items.last().map_or(after, |check| check.seq),
            items,
            has_more,
        })
    }

    pub fn statuses() -> Result<Vec<Status>> {
        let now = berm_lang::now();
        Ok(Self::all()?
            .into_iter()
            .map(|monitor| {
                let stale = monitor.checked_at.is_some_and(|at| {
                    now.saturating_sub(at) > monitor.interval_ms.saturating_mul(2)
                });
                Status {
                    name: monitor.name,
                    state: if stale { "stale".into() } else { monitor.state },
                    checked_at: monitor.checked_at,
                    latency_ms: monitor.latency_ms,
                }
            })
            .collect())
    }
}
