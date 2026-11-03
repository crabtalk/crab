use crate::types::AnalyticsError;
use alloc::{format, string::String, vec::Vec};
use berm_lang::{Result, Schema};
use url::{Host, Url};

pub const DAY: u64 = 86_400_000;
pub const KEPT: u64 = 30 * DAY;
pub const MAX_PAGE: usize = 1024;
const MAX_SITES: u64 = 20;

#[derive(Schema)]
pub struct Site {
    pub id: String,
    pub domain: String,
    pub name: String,
    pub created: u64,
}

impl Site {
    pub fn add(domain: String, name: String) -> Result<Self> {
        let domain = match Host::parse(domain.trim()).map_err(|_| AnalyticsError::BadDomain)? {
            Host::Domain(domain)
                if domain.contains('.')
                    && domain.len() <= 253
                    && domain.split('.').all(|part| {
                        !part.is_empty()
                            && part.len() <= 63
                            && !part.starts_with('-')
                            && !part.ends_with('-')
                            && part
                                .bytes()
                                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                    }) =>
            {
                domain
            }
            _ => return Err(AnalyticsError::BadDomain.into()),
        };
        let name = String::from(name.trim());
        if name.is_empty() || name.chars().count() > 120 {
            return Err(AnalyticsError::BadName.into());
        }
        if !berm_lang::rows::<(String,)>("SELECT id FROM sites WHERE domain = ?", &(&domain,))?
            .is_empty()
        {
            return Err(AnalyticsError::SiteTaken.into());
        }
        let (count,) = berm_lang::rows::<(u64,)>("SELECT COUNT(*) FROM sites", &[0u8; 0])?
            .pop()
            .unwrap_or_default();
        if count >= MAX_SITES {
            return Err("This copy supports twenty sites".into());
        }
        let created = berm_lang::now();
        let (id,) = berm_lang::rows::<(String,)>("INSERT INTO sites (id, domain, name, created) VALUES (lower(hex(randomblob(16))), ?, ?, ?) RETURNING id", &(&domain, &name, created))?.pop().ok_or(AnalyticsError::NoSite)?;
        Ok(Self {
            id,
            domain,
            name,
            created,
        })
    }

    pub fn get(id: &str) -> Result<Option<Self>> {
        Ok(berm_lang::rows::<(String, String, u64)>(
            "SELECT domain, name, created FROM sites WHERE id = ?",
            &(id,),
        )?
        .pop()
        .map(|(domain, name, created)| Self {
            id: id.into(),
            domain,
            name,
            created,
        }))
    }

    pub fn all() -> Result<Vec<Self>> {
        Ok(berm_lang::rows::<(String, String, String, u64)>(
            "SELECT id, domain, name, created FROM sites ORDER BY created, id",
            &[0u8; 0],
        )?
        .into_iter()
        .map(|(id, domain, name, created)| Self {
            id,
            domain,
            name,
            created,
        })
        .collect())
    }

    pub fn remove(id: &str) -> Result<bool> {
        for table in ["events", "days", "seen", "visitors", "pages", "sources"] {
            berm_lang::rows::<()>(&format!("DELETE FROM {table} WHERE site = ?"), &(id,))?;
        }
        Ok(
            !berm_lang::rows::<(String,)>("DELETE FROM sites WHERE id = ? RETURNING id", &(id,))?
                .is_empty(),
        )
    }
}

pub fn host(given: &str) -> Option<String> {
    let url = Url::parse(given).ok()?;
    matches!(url.scheme(), "http" | "https")
        .then(|| url.host_str().map(String::from))
        .flatten()
}

pub fn same_site(host: &str, domain: &str) -> bool {
    host.strip_prefix("www.").unwrap_or(host) == domain.strip_prefix("www.").unwrap_or(domain)
}

pub fn span(from: Option<u64>, to: Option<u64>) -> Result<(u64, u64)> {
    let last = to.unwrap_or_else(berm_lang::now) / DAY;
    let first = from.map_or(last.saturating_sub(29), |from| from / DAY);
    if first > last || last.saturating_sub(first) > 365 {
        return Err("Choose an ordered date range of at most 366 days".into());
    }
    Ok((first, last))
}

pub fn limit(limit: Option<u64>) -> u64 {
    limit.unwrap_or(20).clamp(1, 100)
}
