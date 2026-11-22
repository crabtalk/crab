use crate::{account::Account, claim};
use alloc::string::String;
use berm_lang::{Result, Schema, caller_storage, client, deny, now, rows};
use url::Url;

const BUFFER_LIMIT: u64 = 10_000;

#[derive(Schema, Default)]
pub struct Utm {
    pub source: Option<String>,
    pub medium: Option<String>,
    pub campaign: Option<String>,
}

#[derive(Schema)]
pub struct Tracked {
    pub visitor: String,
}

impl Tracked {
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        domain: &str,
        visitor: Option<String>,
        session: &str,
        path: &str,
        referrer: Option<&str>,
        utm: Utm,
        event: Option<&str>,
        value: Option<i64>,
        currency: Option<&str>,
    ) -> Result<Self> {
        let domain = claim::normalize(domain)?;
        if claim::verified(&domain)?.is_none() {
            return Err(deny("Verify this domain with claim before tracking visits"));
        }
        let visitor = match visitor {
            Some(visitor) => visitor,
            None => client()?.ok_or("No visitor ID is available. Supply a visitor ID to track")?,
        };
        validate_ids(&visitor, session)?;
        if path.len() > 2048 || !path.starts_with('/') {
            return Err("Use a path starting with / and at most 2048 bytes".into());
        }
        if referrer.is_some_and(|value| value.len() > 2048) {
            return Err("Referrer must be at most 2048 bytes".into());
        }
        if [&utm.source, &utm.medium, &utm.campaign]
            .into_iter()
            .flatten()
            .any(|value| value.len() > 256)
        {
            return Err("UTM fields must be at most 256 bytes each".into());
        }
        validate_event(event, value, currency)?;
        let currency = currency.map(str::to_ascii_lowercase);
        let kind = if event.is_some() { "event" } else { "view" };
        rows::<()>(
            "INSERT INTO buffer
             (domain, visitor, session, kind, path, referrer, source, medium, campaign, name, value, currency, at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            &(
                &domain, &visitor, session, kind, path, referrer, utm.source, utm.medium,
                utm.campaign, event, value, currency, now() / 1000,
            ),
        )?;
        rows::<()>(
            "DELETE FROM buffer WHERE domain = ?1 AND id <= (
                SELECT id FROM buffer WHERE domain = ?1 ORDER BY id DESC LIMIT 1 OFFSET ?2
             )",
            &(&domain, BUFFER_LIMIT),
        )?;
        Ok(Self { visitor })
    }
}

pub(crate) fn event(
    account: &Account,
    domain: &str,
    visitor: &str,
    session: &str,
    name: &str,
    value: Option<i64>,
    currency: Option<&str>,
) -> Result<()> {
    let domain = claim::normalize(domain)?;
    if account.claim_id.is_none() || claim::verified(&domain)? != account.claim_id {
        return Err(deny(
            "Verify this domain in your account before recording events",
        ));
    }
    validate_ids(visitor, session)?;
    validate_event(Some(name), value, currency)?;
    caller_storage::rows::<()>(
        "INSERT INTO hits (domain, visitor, session, kind, channel, name, value, currency, at)
         VALUES (?1, ?2, ?3, 'event', COALESCE((SELECT channel FROM hits WHERE visitor = ?2
                 ORDER BY at DESC, id DESC LIMIT 1), 'direct'), ?4, ?5, ?6, ?7)",
        &(
            &domain,
            visitor,
            session,
            name,
            value,
            currency.map(str::to_ascii_lowercase),
            now() / 1000,
        ),
    )?;
    Ok(())
}

fn validate_ids(visitor: &str, session: &str) -> Result<()> {
    if [visitor, session]
        .iter()
        .any(|value| value.is_empty() || value.len() > 256)
    {
        return Err("Visitor and session IDs must be 1 to 256 bytes".into());
    }
    Ok(())
}

fn validate_event(name: Option<&str>, value: Option<i64>, currency: Option<&str>) -> Result<()> {
    if name.is_some_and(|name| {
        name.is_empty()
            || name.len() > 64
            || !name.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            })
    }) {
        return Err(
            "Use an event name of 1 to 64 lowercase letters, digits, underscores or hyphens".into(),
        );
    }
    match (value, currency) {
        (None, None) => Ok(()),
        (Some(value), Some(currency))
            if value >= 0
                && currency.len() == 3
                && currency.bytes().all(|byte| byte.is_ascii_alphabetic()) =>
        {
            Ok(())
        }
        _ => Err("Supply a non-negative value and a three-letter currency together".into()),
    }
}

pub(crate) fn channel(domain: &str, source: Option<&str>, referrer: Option<&str>) -> String {
    if let Some(source) = source.filter(|source| !source.is_empty()) {
        return source.into();
    }
    if let Some(url) = referrer.and_then(|referrer| Url::parse(referrer).ok())
        && matches!(url.scheme(), "http" | "https")
        && let Some(host) = url.host_str()
    {
        let host = host.trim_end_matches('.');
        let host = host.strip_prefix("www.").unwrap_or(host);
        if host != domain.strip_prefix("www.").unwrap_or(domain) {
            return host.into();
        }
    }
    "direct".into()
}
