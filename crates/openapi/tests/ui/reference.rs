extern crate alloc;

crab_openapi::service! {
    spec: "../../../../crates/openapi/tests/specs/reference.json",
    client: crate::CLIENT,
    operations: { example: "POST /items" },
}

fn main() {}
