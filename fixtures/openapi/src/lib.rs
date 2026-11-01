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
        fetch_item: "GET /items/{itemId}",
        create_item: "POST /items",
        ping: "GET /ping",
        optional: "POST /optional",
    },
}
