#![no_std]
extern crate alloc;
use berm_lang::http::Client;

const CLIENT: Client<'static> = Client {
    name: "Context.dev",
    headers: &[("Authorization", "Bearer {secret:CONTEXT_DEV_API_KEY}")],
};

// Reviewed snapshot from https://docs.context.dev/openapi.json.
crab_openapi::service! {
    spec: "openapi.json",
    client: crate::CLIENT,
    tools: cfg(feature = "tools"),
    operations: {
        brand: "POST /brand/retrieve",
        styleguide: "GET /web/styleguide",
        scrape: "POST /web/scrape",
        answers: "POST /web/answers",
        search: "POST /web/search",
        map: "GET /web/urls",
        crawl: "POST /web/crawl",
    },
}
