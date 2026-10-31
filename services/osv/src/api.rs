use crate::types::{BatchPage, PackageVersion, Page, Query};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
use berm_lang::{
    Result,
    http::Client,
    serde_json::{self, Value, json},
};
use url::Url;

fn fetch(method: &str, url: &str, body: &Value) -> Result<Value> {
    let bytes = if method == "GET" {
        Vec::new()
    } else {
        serde_json::to_vec(body)?
    };
    let value: Value = Client {
        name: "OSV",
        headers: &[("Content-Type", "application/json")],
    }
    .json(method, url, &bytes)?;
    if !value.is_object() {
        return Err("OSV returned an unreadable response".into());
    }
    Ok(value)
}
fn payload(package: &PackageVersion, token: Option<&str>) -> Result<Value> {
    if [&package.ecosystem, &package.name, &package.version]
        .iter()
        .any(|s| s.trim().is_empty())
    {
        return Err("Provide a package ecosystem, name and installed version".into());
    }
    let mut body = json!({"package":{"ecosystem":package.ecosystem,"name":package.name},"version":package.version});
    if let Some(token) = token.filter(|t| !t.is_empty()) {
        body["page_token"] = json!(token);
    }
    Ok(body)
}
fn token(value: &Value) -> Result<Option<String>> {
    match value.get("next_page_token") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(token)) => Ok((!token.is_empty()).then(|| token.clone())),
        _ => Err("OSV returned an invalid pagination token".into()),
    }
}
fn records(value: &Value) -> Result<Vec<Value>> {
    match value.get("vulns") {
        None => Ok(Vec::new()),
        Some(Value::Array(rows)) => {
            for row in rows {
                check_advisory(row)?;
            }
            Ok(rows.clone())
        }
        _ => Err("OSV returned invalid advisory records".into()),
    }
}
fn check_advisory(value: &Value) -> Result<()> {
    if ["id", "modified"]
        .iter()
        .any(|key| !value[*key].as_str().is_some_and(|s| !s.is_empty()))
    {
        return Err("OSV advisory has no identifier or modification time".into());
    }
    Ok(())
}
/// Query one page. Continue even an empty page when it carries a next-page token.
pub fn query(package: &PackageVersion, page_token: Option<&str>) -> Result<Page> {
    let value = fetch(
        "POST",
        "https://api.osv.dev/v1/query",
        &payload(package, page_token)?,
    )?;
    Ok(Page {
        vulns: records(&value)?,
        next_page_token: token(&value)?,
    })
}
/// Read a complete result, capped at ten pages. Incomplete results fail rather than look empty.
pub fn query_all(package: &PackageVersion) -> Result<Vec<Value>> {
    let mut next = None;
    let mut seen = BTreeSet::new();
    let mut vulns = BTreeMap::<String, Value>::new();
    for _ in 0..10 {
        let page = query(package, next.as_deref())?;
        for vuln in page.vulns {
            vulns.insert(
                vuln["id"].as_str().expect("validated advisory").into(),
                vuln,
            );
        }
        match page.next_page_token {
            None => return Ok(vulns.into_values().collect()),
            Some(token) if seen.insert(token.clone()) => next = Some(token),
            Some(_) => return Err("OSV repeated a pagination token; results are incomplete".into()),
        }
    }
    Err("OSV exceeded ten pages; results are incomplete".into())
}
/// Query up to 1000 packages. Ordered results carry independent pagination tokens.
pub fn query_batch(queries: &[Query]) -> Result<Vec<BatchPage>> {
    if queries.is_empty() || queries.len() > 1000 {
        return Err("Provide 1 to 1000 package queries".into());
    }
    let bodies: Vec<_> = queries
        .iter()
        .map(|q| payload(&q.package, q.page_token.as_deref()))
        .collect::<Result<_>>()?;
    let value = fetch(
        "POST",
        "https://api.osv.dev/v1/querybatch",
        &json!({"queries":bodies}),
    )?;
    let results = value["results"]
        .as_array()
        .filter(|rows| rows.len() == queries.len())
        .ok_or("OSV returned an invalid batch response")?;
    results
        .iter()
        .map(|row| {
            if !row.is_object() {
                return Err("OSV returned an invalid batch item".into());
            }
            let vulns = records(row)?
                .into_iter()
                .map(|v| {
                    serde_json::from_value(v)
                        .map_err(|_| "OSV returned an invalid advisory reference".into())
                })
                .collect::<Result<_>>()?;
            Ok(BatchPage {
                vulns,
                next_page_token: token(row)?,
            })
        })
        .collect()
}
/// Read a complete advisory by its case-sensitive OSV identifier.
pub fn advisory(id: &str) -> Result<Value> {
    if id.trim().is_empty() {
        return Err("Provide an OSV advisory ID".into());
    }
    let mut url = Url::parse("https://api.osv.dev/v1/vulns/").expect("constant URL");
    url.path_segments_mut()
        .expect("HTTP URL")
        .pop_if_empty()
        .push(id);
    let value = fetch("GET", url.as_str(), &Value::Null)?;
    check_advisory(&value)?;
    Ok(value)
}
