#![no_std]
extern crate alloc;
use berm_lang::http::Client;

const CLIENT: Client<'static> = Client {
    name: "People Data Labs",
    headers: &[("X-API-Key", "{secret:PDL_API_KEY}")],
};

crab_openapi::service! {
    spec: env!("OPENAPI_SPEC"),
    client: crate::CLIENT,
    tools: cfg(feature = "tools"),
    operations: {
        enrich_person: "GET /v5/person/enrich",
        search_people: "GET /v5/person/search",
        enrich_company: "GET /v5/company/enrich",
        search_companies: "GET /v5/company/search",
    },
}
