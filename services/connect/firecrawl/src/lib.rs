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
        /// Scrape a page
        scrape: "POST /scrape",
        /// Search the web
        search: "POST /search",
        /// Map a website
        map: "POST /map",
        /// Start a website crawl
        crawl: "POST /crawl",
        /// Check a website crawl
        crawl_status: "GET /crawl/{id}",
        /// Extract a batch of pages
        batch_scrape: "POST /batch/scrape",
        /// Check a batch scrape
        batch_scrape_status: "GET /batch/scrape/{id}",
    },
}
