use berm_lang::{Branch, Schema};
#[derive(Schema)]
struct Payload {
    value: String,
}
type Alias = Payload;
#[derive(Schema, Branch)]
enum Route {
    First(Payload),
    Second(Alias),
}
fn main() {}
