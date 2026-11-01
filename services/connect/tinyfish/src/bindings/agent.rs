crab_openapi::service! {
    spec: env!("MAIN_SPEC"),
    client: crate::bindings::CLIENT,
    module: crate::bindings::agent,
    raw_response: true,
    tools: false,
    operations: {
        run: "POST /v1/automation/run-async",
        run_status: "GET /v1/runs/{id}",
        run_cancel: "POST /v1/runs/{id}/cancel",
        step_screenshot: "GET /v1/runs/{id}/steps/{stepId}/screenshot",
        step_html: "GET /v1/runs/{id}/steps/{stepId}/html",
    },
}
