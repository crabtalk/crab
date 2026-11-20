#![no_std]
extern crate alloc;
use berm_lang::http::Client;

const CLIENT: Client<'static> = Client {
    name: "CompanyEnrich",
    headers: &[("Authorization", "Bearer {secret:COMPANYENRICH_API_KEY}")],
};

crab_openapi::service! {
    spec: env!("OPENAPI_SPEC"),
    client: crate::CLIENT,
    tools: cfg(feature = "tools"),
    operations: {
        /// Enrich company
        #[tool(catalog = "company.enrich")]
        enrich_company: "GET /companies/enrich",
        /// Enrich company by properties
        #[tool(catalog = "company.enrich")]
        enrich_company_by_properties: "POST /companies/enrich",
        /// Enrich companies batch
        #[tool(catalog = "company.enrich.batch")]
        enrich_companies_batch: "POST /companies/enrich/batch",
        /// Get company
        #[tool(catalog = "company.profile")]
        get_company: "GET /companies",
        /// Get company workforce
        #[tool(catalog = "company.workforce")]
        get_company_workforce: "GET /companies/workforce",
        /// Autocomplete companies
        #[tool(catalog = "company.autocomplete")]
        autocomplete_companies: "GET /companies/autocomplete",
        /// Search companies
        #[tool(catalog = "company.search")]
        search_companies: "POST /companies/search",
        /// Scroll companies
        #[tool(catalog = "company.search.scroll")]
        scroll_companies: "POST /companies/search/scroll",
        /// Count companies
        #[tool(catalog = "company.search.count")]
        count_companies: "POST /companies/search/count",
        /// Preview companies
        #[tool(catalog = "company.search.preview")]
        preview_companies: "POST /companies/search/preview",
        /// Find similar companies
        #[tool(catalog = "company.similar")]
        find_similar_companies: "POST /companies/similar",
        /// Scroll similar companies
        #[tool(catalog = "company.similar.scroll")]
        scroll_similar_companies: "POST /companies/similar/scroll",
        /// Count similar companies
        #[tool(catalog = "company.similar.count")]
        count_similar_companies: "POST /companies/similar/count",
        /// Start company bulk enrichment
        #[tool(catalog = "company.enrich.bulk.start")]
        start_company_bulk_enrichment: "POST /companies/enrich/bulk",
        /// Get company bulk enrichment
        #[tool(catalog = "company.enrich.bulk.status")]
        get_company_bulk_enrichment: "GET /companies/enrich/bulk/{jobId}",
        /// List company bulk enrichments
        #[tool(catalog = "company.enrich.bulk.list")]
        list_company_bulk_enrichments: "GET /companies/enrich/bulk/jobs",
        /// Start company export
        #[tool(catalog = "company.export.start")]
        start_company_export: "POST /companies/search/async",
        /// Get company export
        #[tool(catalog = "company.export.status")]
        get_company_export: "GET /companies/search/async/{jobId}",
        /// List company exports
        #[tool(catalog = "company.export.list")]
        list_company_exports: "GET /companies/search/async/jobs",
        /// Search people
        #[tool(catalog = "person.search")]
        search_people: "POST /people/search",
        /// Scroll people
        #[tool(catalog = "person.search.scroll")]
        scroll_people: "POST /people/search/scroll",
        /// Get person
        #[tool(catalog = "person.profile")]
        get_person: "GET /people",
        /// Lookup person
        #[tool(catalog = "person.enrich")]
        lookup_person: "POST /people/lookup",
        /// Find person email
        #[tool(catalog = "person.email")]
        find_person_email: "GET /people/email",
        /// Start bulk email enrichment
        #[tool(catalog = "person.email.bulk.start")]
        start_bulk_email_enrichment: "POST /people/email/bulk",
        /// Get bulk email enrichment
        #[tool(catalog = "person.email.bulk.status")]
        get_bulk_email_enrichment: "GET /people/email/bulk/{jobId}",
        /// Start people export
        #[tool(catalog = "person.export.start")]
        start_people_export: "POST /people/search/async",
        /// Get people export
        #[tool(catalog = "person.export.status")]
        get_people_export: "GET /people/search/async/{jobId}",
        /// List people exports
        #[tool(catalog = "person.export.list")]
        list_people_exports: "GET /people/search/async/jobs",
        /// List jobs
        list_jobs: "GET /jobs",
        /// Get job
        get_job: "GET /jobs/{jobId}",
        /// List industries
        list_industries: "GET /industries",
        /// Autocomplete keywords
        autocomplete_keywords: "GET /keywords/autocomplete",
        /// Autocomplete technologies
        autocomplete_technologies: "GET /technologies/autocomplete",
        /// Autocomplete positions
        autocomplete_positions: "GET /positions/autocomplete",
        /// List regions
        list_regions: "GET /geo/regions",
        /// Get country
        get_country: "GET /geo/countries/{countryCode}",
        /// Search countries
        search_countries: "POST /geo/countries",
        /// Search states
        search_states: "POST /geo/states",
        /// Search cities
        search_cities: "POST /geo/cities",
        /// Get account
        get_account: "GET /me",
    },
}
