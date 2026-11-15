use berm_lang::workflow;
#[path = "../workflows/support/mod.rs"]
mod support;
use support::{EchoCall, Input};
struct Flow;
#[workflow]
impl Flow {
    #[step]
    fn first(later: &u64) -> EchoCall {
        EchoCall { value: *later }
    }
    #[step]
    fn later(input: &Input) -> EchoCall {
        EchoCall { value: input.count }
    }
    #[output]
    fn done(first: &u64) -> u64 {
        *first
    }
}
fn main() {}
