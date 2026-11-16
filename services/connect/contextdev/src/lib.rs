#![no_std]
extern crate alloc;
use berm_lang::http::Client;

const CLIENT: Client<'static> = Client {
    name: "Context.dev",
    headers: &[("Authorization", "Bearer {secret:CONTEXT_DEV_API_KEY}")],
};

crab_openapi::service! {
    spec: env!("OPENAPI_SPEC"),
    client: crate::CLIENT,
    tools: cfg(feature = "tools"),
    operations: {
        /// Retrieve a brand
        brand: "POST /brand/retrieve",
        /// Extract a styleguide
        styleguide: "GET /web/styleguide",
        /// Extract a page
        #[tool(catalog = "web.extract")]
        scrape: "POST /web/scrape",
        /// Answer a research task
        answers: "POST /web/answers",
        /// Search the web
        #[tool(catalog = "web.search")]
        search: "POST /web/search",
        /// Map a website
        #[tool(catalog = "web.map")]
        map: "GET /web/urls",
        /// Crawl a website
        #[tool(catalog = "web.crawl")]
        crawl: "POST /web/crawl",
    },
}
