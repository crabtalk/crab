#![cfg(feature = "tools")]
use berm_api::Manifest;
use berm_build::fixture;

#[test]
fn image_exports_the_complete_scope_and_upstream_input_constraints() {
    let bytes = fixture::image(env!("CARGO_PKG_NAME")).unwrap();
    let manifest = Manifest::try_from(bytes.as_slice()).unwrap();
    let mut names: Vec<_> = manifest
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "autocomplete_companies",
            "autocomplete_keywords",
            "autocomplete_positions",
            "autocomplete_technologies",
            "count_companies",
            "count_similar_companies",
            "enrich_companies_batch",
            "enrich_company",
            "enrich_company_by_properties",
            "find_person_email",
            "find_similar_companies",
            "get_account",
            "get_bulk_email_enrichment",
            "get_company",
            "get_company_bulk_enrichment",
            "get_company_export",
            "get_company_workforce",
            "get_country",
            "get_job",
            "get_people_export",
            "get_person",
            "list_company_bulk_enrichments",
            "list_company_exports",
            "list_industries",
            "list_jobs",
            "list_people_exports",
            "list_regions",
            "lookup_person",
            "preview_companies",
            "scroll_companies",
            "scroll_people",
            "scroll_similar_companies",
            "search_cities",
            "search_companies",
            "search_countries",
            "search_people",
            "search_states",
            "start_bulk_email_enrichment",
            "start_company_bulk_enrichment",
            "start_company_export",
            "start_people_export"
        ]
    );
    let batch = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "enrich_companies_batch")
        .unwrap();
    let domains =
        &batch.parameters["properties"]["input"]["properties"]["body"]["properties"]["domains"];
    assert_eq!(domains["minItems"], 1);
    assert_eq!(domains["maxItems"], 50);
    let search = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "search_companies")
        .unwrap();
    let body = &search.parameters["properties"]["input"]["properties"]["body"]["properties"];
    assert_eq!(body["pageSize"]["maximum"], 100);
    assert!(body.get("workforceGrowth").is_some());
    assert!(body.get("semanticQuery").is_some());
    let bulk = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "start_bulk_email_enrichment")
        .unwrap();
    assert_eq!(
        bulk.parameters["properties"]["input"]["properties"]["body"]["properties"]["items"]["maxItems"],
        1000
    );
}
