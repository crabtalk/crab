use berm_lang::{Branch, Schema, workflow::Projection};
#[derive(Schema)]
struct First {
    value: String,
}
#[derive(Schema)]
struct Second {
    value: String,
}
#[derive(Schema, Branch)]
enum Route {
    First(First),
}
fn main() {
    let route = Route::First(First {
        value: String::new(),
    });
    let _ = Projection::<Second>::project(route);
}
