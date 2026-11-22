use crate::account::Account;
use alloc::{format, string::String};
use berm_lang::{Result, Schema, deny, get, http, now, random, rows, set, sql};
use sha2::{Digest, Sha256};
use url::{Host, Url};

const PROOF_PATH: &str = "/.well-known/crabtalk-analytics";
const TRANSFER_SEED: &str = "claim_transfer_seed";

#[derive(Schema)]
pub struct Claim {
    pub domain: String,
    pub url: String,
    pub token: String,
    pub verified: bool,
}

impl Claim {
    pub(crate) fn check(domain: &str, account: &mut Account) -> Result<Self> {
        let domain = normalize(domain)?;
        let url = format!("https://{domain}{PROOF_PATH}");
        let current = rows::<(String, String, Option<u64>)>(
            "SELECT account, token, verified_at FROM claims WHERE domain = ?1",
            &(&domain,),
        )?
        .pop();
        let own = current
            .as_ref()
            .is_some_and(|(owner, _, _)| Some(owner) == account.claim_id.as_ref());
        let (token, verified) = match current {
            Some((_, token, verified_at)) if own => {
                let verified = verified_at.is_some() || proof(&url, &token)?;
                (token, verified)
            }
            Some((_, current_token, Some(_))) => {
                // Refusals roll back. Derive the same private challenge on a retry.
                let seed = get::<[u8; 32]>(TRANSFER_SEED)?.ok_or("Missing domain transfer seed")?;
                let scope = berm_lang::json::to(&(&domain, &account.member, &current_token))?;
                let mut digest = Sha256::new();
                digest.update(scope);
                digest.update(seed);
                let token = hex::encode(&digest.finalize()[..16]);
                if !proof(&url, &token)? {
                    return Err(deny(&format!(
                        "This domain is claimed by another account. Publish {token} at {url} and retry to transfer it"
                    )));
                }
                sql(
                    "DELETE FROM buffer WHERE domain = ?1",
                    &berm_lang::json::to(&(&domain,))?,
                )?;
                (token, true)
            }
            _ => {
                let mut token = [0; 16];
                random(&mut token);
                (hex::encode(token), false)
            }
        };
        if verified && get::<[u8; 32]>(TRANSFER_SEED)?.is_none() {
            let mut seed = [0; 32];
            random(&mut seed);
            set(TRANSFER_SEED, &seed)?;
        }
        rows::<()>(
            "INSERT INTO claims (domain, account, token, verified_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (domain) DO UPDATE SET account = excluded.account, token = excluded.token,
             verified_at = IIF(claims.account = excluded.account AND claims.verified_at IS NOT NULL,
                              claims.verified_at, excluded.verified_at)",
            &(
                &domain,
                account.claim_id()?,
                &token,
                verified.then(|| now() / 1000),
            ),
        )?;
        Ok(Self {
            domain,
            url,
            token,
            verified,
        })
    }
}

pub(crate) fn normalize(domain: &str) -> Result<String> {
    let domain = domain.trim();
    let url = if domain.contains("://") {
        Url::parse(domain)
    } else {
        Url::parse(&format!("https://{domain}"))
    }
    .map_err(|_| "Use a valid domain name")?;
    let Some(Host::Domain(host)) = url.host() else {
        return Err("Use a valid domain name".into());
    };
    let host = host.strip_suffix('.').unwrap_or(host);
    let host = host.strip_prefix("www.").unwrap_or(host);
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || host.len() > 253
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err("Use a valid domain name".into());
    }
    Ok(host.into())
}

pub(crate) fn verified(domain: &str) -> Result<Option<String>> {
    Ok(rows::<(String,)>(
        "SELECT account FROM claims WHERE domain = ?1 AND verified_at IS NOT NULL",
        &(domain,),
    )?
    .pop()
    .map(|(account,)| account))
}

fn proof(url: &str, token: &str) -> Result<bool> {
    let response = http::fetch_all(&[http::Request {
        method: "GET",
        url,
        headers: &[],
        body: b"",
        timeout: Some(5000),
        limit: Some(4097),
    }])?
    .pop();
    Ok(matches!(response, Some(Ok(response))
        if response.is_success() && response.body.len() <= 4096
            && core::str::from_utf8(&response.body).is_ok_and(|body| body.trim() == token)))
}
