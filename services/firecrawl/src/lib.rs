#![no_std]
extern crate alloc;
use berm_lang::http::Client;

pub mod calls;
pub mod format;

const CLIENT: Client<'static> = Client {
    name: "Firecrawl",
    headers: &[("Authorization", "Bearer {secret:FIRECRAWL_API_KEY}")],
};

crab_openapi::service! {
    spec: env!("OPENAPI_SPEC"),
    client: crate::CLIENT,
    tools: false,
    operations: {
        scrape: "POST /scrape",
        search: "POST /search",
        map: "POST /map",
        crawl: "POST /crawl",
        crawl_status: "GET /crawl/{id}",
        batch_scrape: "POST /batch/scrape",
        batch_scrape_status: "GET /batch/scrape/{id}",
    },
}
