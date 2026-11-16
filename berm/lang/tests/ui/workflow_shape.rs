use berm_lang::workflow;
#[path = "../workflows/support/mod.rs"]
mod support;
use support::Input;
struct Flow;
#[workflow]
impl Flow {
    #[output]
    async fn done(input: &Input) -> u64 {
        input.count
    }
}
fn main() {}
