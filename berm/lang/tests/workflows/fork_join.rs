use berm_lang::workflow;
use support::{EchoCall, Input};
mod support;
struct Flow;
#[workflow]
impl Flow {
    #[step]
    fn left(input: &Input) -> EchoCall {
        EchoCall { value: input.count }
    }
    #[step]
    fn right(input: &Input) -> EchoCall {
        EchoCall { value: input.count }
    }
    #[output]
    fn done(left: &u64, right: &u64) -> u64 {
        left + right
    }
}
fn main() {}
