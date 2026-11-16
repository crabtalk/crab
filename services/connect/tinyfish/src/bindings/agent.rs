crab_openapi::service! {
    spec: env!("MAIN_SPEC"),
    client: crate::bindings::CLIENT,
    module: crate::bindings::agent,
    raw_response: true,
    tools: false,
    operations: {
        /// Run a browser agent
        #[tool(catalog = "web.agent.run")]
        run: "POST /v1/automation/run-async",
        /// Check a browser-agent run
        #[tool(catalog = "web.agent.run.status")]
        run_status: "GET /v1/runs/{id}",
        /// Cancel a browser-agent run
        #[tool(catalog = "web.agent.run.cancel")]
        run_cancel: "POST /v1/runs/{id}/cancel",
        /// Retrieve a step screenshot
        #[tool(catalog = "web.agent.step.screenshot")]
        step_screenshot: "GET /v1/runs/{id}/steps/{stepId}/screenshot",
        /// Retrieve step HTML
        #[tool(catalog = "web.agent.step.html")]
        step_html: "GET /v1/runs/{id}/steps/{stepId}/html",
    },
}
