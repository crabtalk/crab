use berm_lang::workflow;
use support::{EchoCall, Input};
mod support;
struct Flow;
#[workflow]
impl Flow {
    #[step]
    fn first(input: &Input) -> EchoCall {
        EchoCall { value: input.count }
    }
    #[output]
    fn done(first: &u64) -> u64 {
        *first
    }
}
fn main() {}
