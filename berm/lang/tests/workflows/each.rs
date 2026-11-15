use berm_lang::workflow;
use support::{EchoCall, Input};
mod support;
struct Flow;
#[workflow]
impl Flow {
    #[step(each, max = 5)]
    fn read(input: &Input) -> Vec<EchoCall> {
        (0..input.count).map(|value| EchoCall { value }).collect()
    }
    #[output]
    fn done(read: &[u64]) -> u64 {
        read.iter().sum()
    }
}
fn main() {}
