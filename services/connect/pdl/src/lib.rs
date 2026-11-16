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
        /// Enrich a person
        #[tool(catalog = "person.enrich")]
        enrich_person: "GET /v5/person/enrich",
        /// Search people
        #[tool(catalog = "person.search")]
        search_people: "GET /v5/person/search",
        /// Enrich a company
        #[tool(catalog = "company.enrich")]
        enrich_company: "GET /v5/company/enrich",
        /// Search companies
        #[tool(catalog = "company.search")]
        search_companies: "GET /v5/company/search",
    },
}
