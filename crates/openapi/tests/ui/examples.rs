extern crate alloc;
use berm_lang::http::Client;
const CLIENT: Client<'static> = Client {
    name: "Example",
    headers: &[],
};
crab_openapi::service! {
    spec: "../../../../crates/openapi/tests/specs/api.json", client: crate::CLIENT,
    operations: { #[doc = "Ping"] #[tool(example = "one", example = "two", example = "three", example = "four")] ping: "GET /ping" },
}
fn main() {}
