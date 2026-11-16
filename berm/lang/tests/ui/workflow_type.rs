use berm_lang::workflow;
#[path = "../workflows/support/mod.rs"]
mod support;
use support::{EchoCall, Input};
struct Flow;
/// Validate this graph
#[workflow]
impl Flow {
    #[step]
    fn first(input: &Input) -> EchoCall {
        EchoCall { value: input.count }
    }
    #[output]
    fn done(first: &String) -> String {
        first.clone()
    }
}
fn main() {}
