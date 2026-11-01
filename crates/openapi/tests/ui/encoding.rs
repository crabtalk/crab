extern crate alloc;

crab_openapi::service! {
    spec: "../../../../crates/openapi/tests/specs/encoding.json",
    client: crate::CLIENT,
    operations: { example: "GET /items/{itemId}" },
}

fn main() {}
