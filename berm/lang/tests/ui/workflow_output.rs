use berm_lang::workflow;
#[path = "../workflows/support/mod.rs"]
mod support;
use support::{EchoCall, Input};
struct Flow;
/// Validate this graph
#[workflow]
impl Flow {
    #[step]
    fn maybe(input: &Input) -> Option<EchoCall> {
        Some(EchoCall { value: input.count })
    }
    #[output]
    fn done(maybe: &u64) -> u64 {
        *maybe
    }
}
fn main() {}
