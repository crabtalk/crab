use crate::{
    audit::{
        AGENT, CHECK_TIMEOUT, FINISH_BUDGET_MS,
        discovery::{allowed, request},
    },
    crawlers::Robots,
    markdown,
};
use berm_lang::{Result, caller_storage, deadline, http};
use url::Url;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Method {
    Suffix = 1,
    Accept = 2,
}

#[derive(Clone, Copy)]
pub(super) enum Coverage {
    Unknown,
    Missing,
    Available(Method),
}

pub(super) struct SiteMarkdown {
    root: Url,
    robots: Robots,
    method: Method,
}

impl SiteMarkdown {
    pub fn read(site: &str) -> Result<Option<Self>> {
        let mode = caller_storage::rows::<(String, u8)>(
            "SELECT robots, root_markdown FROM runs WHERE site = ?1 AND root_markdown <> 0",
            &(site,),
        )?
        .pop();
        let Some((rules, method)) = mode else {
            return Ok(None);
        };
        let Some(method) = Method::from_stored(method) else {
            return Ok(None);
        };
        let root = Url::parse(site)
            .and_then(|site| site.join("/"))
            .map_err(|_| "Invalid audited site")?;
        Ok(Some(Self {
            root,
            robots: Robots::from(rules.as_str()),
            method,
        }))
    }

    pub fn coverage(&self, urls: &[&str]) -> Result<Vec<Option<bool>>> {
        let mut coverage = vec![None; urls.len()];
        let mut indices = Vec::new();
        let mut pages = Vec::new();
        for (index, url) in urls.iter().enumerate() {
            let url = Url::parse(url).map_err(|_| "Invalid sitemap page URL")?;
            if url == self.root {
                coverage[index] = Some(true);
            } else {
                indices.push(index);
                pages.push(url);
            }
        }
        if deadline() > FINISH_BUDGET_MS {
            for (index, result) in indices.into_iter().zip(self.method.coverage(
                &pages,
                &self.robots,
                CHECK_TIMEOUT,
            )?) {
                coverage[index] = match result {
                    Coverage::Available(_) => Some(true),
                    Coverage::Missing => Some(false),
                    Coverage::Unknown => None,
                };
            }
        }
        Ok(coverage)
    }
}

impl Method {
    fn from_stored(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Suffix),
            2 => Some(Self::Accept),
            _ => None,
        }
    }

    pub fn coverage(self, pages: &[Url], robots: &Robots, timeout: u64) -> Result<Vec<Coverage>> {
        let alternate = match self {
            Self::Suffix => Self::Accept,
            Self::Accept => Self::Suffix,
        };
        let first = self.probe(pages.iter().map(Some), robots, timeout)?;
        let second = alternate.probe(
            pages
                .iter()
                .zip(&first)
                .map(|(url, &found)| (found != Some(true)).then_some(url)),
            robots,
            timeout,
        )?;
        Ok(first
            .into_iter()
            .zip(second)
            .map(|(first, second)| match (first, second) {
                (Some(true), _) => Coverage::Available(self),
                (_, Some(true)) => Coverage::Available(alternate),
                (Some(false), Some(false)) => Coverage::Missing,
                _ => Coverage::Unknown,
            })
            .collect())
    }

    fn probe<'a>(
        self,
        pages: impl Iterator<Item = Option<&'a Url>>,
        robots: &Robots,
        timeout: u64,
    ) -> Result<Vec<Option<bool>>> {
        let urls: Vec<_> = pages
            .map(|page| {
                page.map(|url| match self {
                    Self::Suffix => markdown::copy_url(url),
                    Self::Accept => url.clone(),
                })
                .filter(|url| allowed(robots, url))
            })
            .collect();
        let requests: Vec<_> = urls
            .iter()
            .flatten()
            .map(|url| {
                let mut request = request(url.as_str(), 1);
                request.timeout = Some(timeout);
                if self == Self::Accept {
                    request.headers = &[("User-Agent", AGENT), markdown::ACCEPT];
                }
                request
            })
            .collect();
        if requests.is_empty() {
            return Ok(vec![None; urls.len()]);
        }
        let mut responses = http::fetch_all(&requests)?.into_iter();
        Ok(urls
            .iter()
            .map(|url| {
                url.as_ref()?;
                self.outcome(responses.next()?.ok()?)
            })
            .collect())
    }

    fn outcome(self, response: http::Response) -> Option<bool> {
        let available = match self {
            Self::Suffix => markdown::has_copy(&response),
            Self::Accept => markdown::negotiated(&response),
        };
        if available {
            return Some(true);
        }
        match response.status {
            200..=303 | 305..=308 | 401 | 403 | 404 | 406 | 410 => Some(false),
            _ => None,
        }
    }
}
