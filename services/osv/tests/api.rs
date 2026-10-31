use berm_lang::{
    http::Response,
    serde_json::{self, Value, json},
    test,
};
use osv::{PackageVersion, Query};
fn package() -> PackageVersion {
    PackageVersion {
        ecosystem: "crates.io".into(),
        name: "example".into(),
        version: "1.2.3".into(),
    }
}
fn respond(path: &str, body: Value) {
    test::respond(
        "POST",
        &format!("https://api.osv.dev/v1/{path}"),
        200,
        &serde_json::to_vec(&body).unwrap(),
    );
}

#[test]
fn query_preserves_full_advisories_and_sends_exact_package_and_page() {
    respond(
        "query",
        json!({"vulns":[{"id":"GHSA-1","modified":"2026-10-01T00:00:00Z","affected":[{"ranges":[{"events":[{"fixed":"2.0.0"}]}]}],"database_specific":{"severity":"HIGH"}}],"next_page_token":"next"}),
    );
    let page = osv::query(&package(), Some("previous")).unwrap();
    assert_eq!(page.next_page_token.as_deref(), Some("next"));
    assert_eq!(page.vulns[0]["database_specific"]["severity"], "HIGH");
    let sent: Value = serde_json::from_slice(&test::fetched()[0].body).unwrap();
    assert_eq!(
        sent,
        json!({"package":{"ecosystem":"crates.io","name":"example"},"version":"1.2.3","page_token":"previous"})
    );
    assert!(test::fetched()[0].header("authorization").is_none());
}

#[test]
fn empty_pages_and_complete_results_are_distinct_from_incomplete_results() {
    respond("query", json!({}));
    assert!(osv::query_all(&package()).unwrap().is_empty());
    respond("query", json!({"next_page_token":"next"}));
    let page = osv::query(&package(), None).unwrap();
    assert!(page.vulns.is_empty());
    assert!(page.next_page_token.is_some());
    let error = osv::query_all(&package()).unwrap_err().message;
    assert!(error.contains("repeated a pagination token"));
    respond(
        "query",
        json!({"vulns":[{"id":"B","modified":"t"},{"id":"A","modified":"t"},{"id":"A","modified":"t"}]}),
    );
    let all = osv::query_all(&package()).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0]["id"], "A");
}

#[test]
fn batch_preserves_order_empty_matches_and_independent_cursors() {
    respond(
        "querybatch",
        json!({"results":[{}, {"vulns":[{"id":"GHSA-A","modified":"t"}],"next_page_token":"p2"}]}),
    );
    let queries = [
        Query {
            package: package(),
            page_token: None,
        },
        Query {
            package: package(),
            page_token: Some("p1".into()),
        },
    ];
    let pages = osv::query_batch(&queries).unwrap();
    assert!(pages[0].vulns.is_empty());
    assert_eq!(pages[1].vulns[0].id, "GHSA-A");
    assert_eq!(pages[1].next_page_token.as_deref(), Some("p2"));
    let sent: Value = serde_json::from_slice(&test::fetched()[0].body).unwrap();
    assert!(sent["queries"][0].get("page_token").is_none());
    assert_eq!(sent["queries"][1]["page_token"], "p1");
}

#[test]
fn advisory_ids_are_encoded_as_one_path_segment() {
    test::respond(
        "GET",
        "https://api.osv.dev/v1/vulns/ID%2Fwith%3Fparts",
        200,
        br#"{"id":"ID/with?parts","modified":"t","withdrawn":"t2"}"#,
    );
    assert_eq!(osv::advisory("ID/with?parts").unwrap()["withdrawn"], "t2");
    assert!(test::fetched()[0].body.is_empty());
}

#[test]
fn malformed_results_and_provider_errors_are_not_empty_successes() {
    for body in [
        json!([]),
        json!({"vulns":null}),
        json!({"vulns":[{}]}),
        json!({"next_page_token":4}),
    ] {
        respond("query", body);
        assert!(osv::query(&package(), None).is_err());
    }
    respond("querybatch", json!({"results":[]}));
    assert!(
        osv::query_batch(&[Query {
            package: package(),
            page_token: None
        }])
        .is_err()
    );
    test::respond_with(
        "POST",
        "https://api.osv.dev/v1/query",
        Response {
            status: 429,
            headers: vec![("Retry-After".into(), "10".into())],
            body: br#"{"message":"rate limit"}"#.to_vec(),
        },
    );
    let error = osv::query(&package(), None).unwrap_err().message;
    assert!(error.contains("429"));
    assert!(error.contains("retry-after=10"));
}

#[cfg(feature = "tools")]
#[test]
fn tool_dispatch_reads_structured_package_and_returns_structured_page() {
    respond("query", json!({}));
    let result = test::call(
        osv::tools::berm_tool_query,
        br#"{"package":{"ecosystem":"PyPI","name":"example","version":"1"}}"#,
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&result).unwrap(),
        json!({"vulns":[]})
    );
}
