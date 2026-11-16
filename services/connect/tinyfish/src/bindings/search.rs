crab_openapi::service! {
    spec: env!("SEARCH_SPEC"),
    client: crate::bindings::CLIENT,
    module: crate::bindings::search,
    raw_response: true,
    tools: false,
    operations: {
        /// Search the web
        #[tool(catalog = "web.search")]
        search: "GET /",
    },
}
