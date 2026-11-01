extern crate alloc;

crab_openapi::service! {
    spec: "../../../../crates/openapi/tests/specs/version.json",
    client: crate::CLIENT,
    operations: { example: "GET /ping" },
}

fn main() {}
