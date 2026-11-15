use berm_lang::workflow;
#[path = "../workflows/support/mod.rs"]
mod support;
use support::{EchoCall, Input};
struct Flow;
#[workflow]
impl Flow {
    #[step]
    fn unused(input: &Input) -> EchoCall {
        EchoCall { value: input.count }
    }
    #[output]
    fn done(input: &Input) -> u64 {
        input.count
    }
}
fn main() {}
