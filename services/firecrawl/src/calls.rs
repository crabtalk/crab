use crate::{api, format::Format, types};
use alloc::{format, string::String, vec::Vec};
use berm_lang::{
    Result,
    serde_json::{self, Value, json},
};

const MAX_ITEMS: u64 = 10;
const MAX_DISCOVERY_DEPTH: u64 = 10;
const MAX_FILTERS: usize = 20;
const MAX_FORMATS: usize = 3;
const MAX_AGE: u64 = 172_800_000;
const MAX_MAP_LINKS: u64 = 1000;

/// Scrape one page. PDF parsing and charged extraction formats are disabled.
#[cfg_attr(feature = "tools", berm_lang::tool)]
pub fn scrape(
    url: String,
    formats: Option<Vec<Format>>,
    only_main_content: Option<bool>,
    max_age: Option<u64>,
) -> Result<Value> {
    let mut body = json!({"url":url});
    basic_options(&mut body, formats, only_main_content, max_age)?;
    response(api::scrape(&types::ScrapeInput { body })?)
}

/// Search up to ten web results without scraping their content.
#[cfg_attr(feature = "tools", berm_lang::tool)]
pub fn search(
    query: String,
    limit: u64,
    include_domains: Option<Vec<String>>,
    exclude_domains: Option<Vec<String>>,
) -> Result<Value> {
    item_count(limit)?;
    let mut body = json!({"query":query,"limit":limit});
    filters(
        &mut body,
        [
            ("includeDomains", include_domains),
            ("excludeDomains", exclude_domains),
        ],
    )?;
    response(api::search(&types::SearchInput { body })?)
}

/// Discover URLs without scraping their content; limit is from 1 to 1000.
#[cfg_attr(feature = "tools", berm_lang::tool)]
pub fn map(
    url: String,
    search: Option<String>,
    limit: Option<u64>,
    include_subdomains: Option<bool>,
    ignore_query_parameters: Option<bool>,
) -> Result<Value> {
    let mut body = json!({"url":url});
    if let Some(value) = search {
        body["search"] = value.into();
    }
    let limit = limit.unwrap_or(MAX_MAP_LINKS);
    if !(1..=MAX_MAP_LINKS).contains(&limit) {
        return Err("limit must be from 1 to 1000".into());
    }
    body["limit"] = limit.into();
    if let Some(value) = include_subdomains {
        body["includeSubdomains"] = value.into();
    }
    if let Some(value) = ignore_query_parameters {
        body["ignoreQueryParameters"] = value.into();
    }
    response(api::map(&types::MapInput { body })?)
}

/// Start a crawl of at most ten pages; PDF parsing and charged add-ons are disabled.
#[cfg_attr(feature = "tools", berm_lang::tool)]
pub fn crawl(
    url: String,
    limit: u64,
    max_discovery_depth: Option<u64>,
    include_paths: Option<Vec<String>>,
    exclude_paths: Option<Vec<String>>,
) -> Result<Value> {
    item_count(limit)?;
    let mut body = json!({"url":url,"limit":limit,"scrapeOptions":{"parsers":[]}});
    if let Some(value) = max_discovery_depth {
        if value > MAX_DISCOVERY_DEPTH {
            return Err("max_discovery_depth must be at most 10".into());
        }
        body["maxDiscoveryDepth"] = value.into();
    }
    filters(
        &mut body,
        [
            ("includePaths", include_paths),
            ("excludePaths", exclude_paths),
        ],
    )?;
    response(api::crawl(&types::CrawlInput { body })?)
}

/// Read one page of an owned crawl's results and credit usage. Pass skip from next.
#[cfg_attr(feature = "tools", berm_lang::tool)]
pub fn crawl_status(id: String, skip: Option<i64>) -> Result<Value> {
    skip_offset(skip)?;
    response(api::crawl_status(&types::CrawlStatusInput {
        path: types::CrawlStatusInputPath { id },
        query: skip.map(|skip| types::CrawlStatusInputQuery { skip: Some(skip) }),
    })?)
}

/// Start a batch of one to ten URLs; formats are Markdown, HTML and links.
#[cfg_attr(feature = "tools", berm_lang::tool)]
pub fn batch_scrape(
    urls: Vec<String>,
    formats: Vec<Format>,
    only_main_content: Option<bool>,
    max_age: Option<u64>,
) -> Result<Value> {
    item_count(urls.len() as u64)?;
    let mut body = json!({"urls":urls});
    basic_options(&mut body, Some(formats), only_main_content, max_age)?;
    response(api::batch_scrape(&types::BatchScrapeInput { body })?)
}

/// Read one page of an owned batch's results and credit usage. Pass skip from next.
#[cfg_attr(feature = "tools", berm_lang::tool)]
pub fn batch_scrape_status(id: String, skip: Option<i64>) -> Result<Value> {
    skip_offset(skip)?;
    response(api::batch_scrape_status(&types::BatchScrapeStatusInput {
        path: types::BatchScrapeStatusInputPath { id },
        query: skip.map(|skip| types::BatchScrapeStatusInputQuery { skip: Some(skip) }),
    })?)
}

fn basic_options(
    body: &mut Value,
    formats: Option<Vec<Format>>,
    only_main_content: Option<bool>,
    max_age: Option<u64>,
) -> Result<()> {
    body["parsers"] = json!([]);
    if let Some(formats) = formats {
        if formats.is_empty() || formats.len() > MAX_FORMATS {
            return Err("formats must contain 1 to 3 entries".into());
        }
        body["formats"] = serde_json::to_value(formats)?;
    }
    if let Some(value) = only_main_content {
        body["onlyMainContent"] = value.into();
    }
    if let Some(value) = max_age {
        if value > MAX_AGE {
            return Err("max_age must be at most 172800000".into());
        }
        body["maxAge"] = value.into();
    }
    Ok(())
}

fn filters(body: &mut Value, filters: [(&str, Option<Vec<String>>); 2]) -> Result<()> {
    for (name, values) in filters {
        if let Some(values) = values {
            if values.len() > MAX_FILTERS {
                return Err(format!("{name} must contain at most 20 entries").into());
            }
            body[name] = serde_json::to_value(values)?;
        }
    }
    Ok(())
}

fn item_count(limit: u64) -> Result<()> {
    if !(1..=MAX_ITEMS).contains(&limit) {
        return Err("page or result count must be from 1 to 10".into());
    }
    Ok(())
}

fn skip_offset(skip: Option<i64>) -> Result<()> {
    if skip.is_some_and(|skip| skip < 0) {
        return Err("skip must be nonnegative".into());
    }
    Ok(())
}

fn response(value: Value) -> Result<Value> {
    if value["success"] == false {
        return Err(format!("Firecrawl: {value}").into());
    }
    Ok(value)
}
