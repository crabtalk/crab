use berm_lang::workflow;
#[path = "../workflows/support/mod.rs"]
mod support;
use support::{EchoCall, Input, No, Route, Yes};
struct Flow;
#[workflow]
impl Flow {
    #[branch]
    fn route(input: &Input) -> Route {
        Route::Yes(Yes { count: input.count })
    }
    #[step]
    fn yes(route: &Yes) -> EchoCall {
        EchoCall { value: route.count }
    }
    #[step]
    fn no(route: &No) -> EchoCall {
        EchoCall { value: route.count }
    }
    #[step]
    fn both(yes: &u64, no: &u64) -> EchoCall {
        EchoCall { value: yes + no }
    }
    #[output]
    fn done(both: Option<&u64>) -> u64 {
        both.copied().unwrap_or(0)
    }
}
fn main() {}
