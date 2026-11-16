#![no_std]
extern crate alloc;
use berm_lang::http::Client;

const CLIENT: Client<'static> = Client {
    name: "Example",
    headers: &[("Authorization", "Bearer {secret:EXAMPLE_KEY}")],
};

crab_openapi::service! {
    spec: env!("OPENAPI_SPEC"),
    client: crate::CLIENT,
    tools: cfg(feature = "tools"),
    operations: {
        #[tool(access = "internal")]
        /// Read an item
        fetch_item: "GET /items/{itemId}",
        #[tool(access = "internal")]
        /// Create an item
        create_item: "POST /items",
        #[tool(access = "internal")]
        /// Ping the API
        ping: "GET /ping",
        #[tool(access = "internal")]
        /// Send optional input
        optional: "POST /optional",
    },
}
