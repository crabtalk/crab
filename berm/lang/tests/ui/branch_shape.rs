use berm_lang::{Branch, Schema};
#[derive(Schema, Branch)]
enum Route {
    Ready,
    Failed { message: String },
}
fn main() {}
