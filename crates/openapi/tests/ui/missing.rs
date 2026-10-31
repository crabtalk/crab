extern crate alloc;

crab_openapi::service! {
    spec: "../../../../crates/openapi/tests/specs/missing.json",
    client: crate::CLIENT,
    operations: { example: "GET /missing" },
}

fn main() {}
