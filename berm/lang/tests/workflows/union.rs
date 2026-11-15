use berm_lang::workflow;
use support::{EchoCall, Input, Route, Yes};
mod support;
struct Flow;
#[workflow]
impl Flow {
    #[step]
    fn get(input: &Input) -> EchoCall {
        EchoCall { value: input.count }
    }
    #[branch]
    fn route(get: &u64) -> Route {
        Route::Yes(Yes { count: *get })
    }
    #[output]
    fn done(route: &Route) -> String {
        berm_lang::serde_json::to_string(route).unwrap()
    }
}
fn main() {}
