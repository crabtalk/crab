crab_openapi::service! {
    spec: env!("FETCH_SPEC"),
    client: crate::bindings::CLIENT,
    module: crate::bindings::fetch,
    raw_response: true,
    tools: false,
    operations: {
        fetch: "POST /",
    },
}
