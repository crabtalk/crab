use berm_lang::workflow;
use support::{EchoCall, Input};
mod support;
struct Flow;
/// Flow
#[workflow]
impl Flow {
    #[step]
    fn maybe(input: &Input) -> Option<EchoCall> {
        (input.count > 0).then_some(EchoCall { value: input.count })
    }
    #[output]
    fn done(maybe: Option<&u64>) -> u64 {
        maybe.copied().unwrap_or(0)
    }
}
fn main() {}
