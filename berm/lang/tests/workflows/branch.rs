use berm_lang::workflow;
use support::{EchoCall, Input, No, Route, Yes};
mod support;
struct Flow;
/// Flow
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
    #[output]
    fn done(yes: Option<&u64>, no: Option<&u64>) -> u64 {
        yes.or(no).copied().unwrap_or(0)
    }
}
fn main() {}
