use berm_lang::workflow;
#[path = "../workflows/support/mod.rs"]
mod support;
use support::{EchoCall, Input};
struct Flow;
/// Validate this graph
#[workflow]
impl Flow {
    #[step(each)]
    fn first(input: &Input) -> Vec<EchoCall> {
        vec![EchoCall { value: input.count }]
    }
    #[output]
    fn done(first: &[u64]) -> u64 {
        first[0]
    }
}
fn main() {}
