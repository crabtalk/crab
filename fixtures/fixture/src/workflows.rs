use berm_lang::workflow;

struct WorkflowExample;
#[workflow]
impl WorkflowExample {
    #[step]
    fn spin(input: &u64) -> crate::SpinCall {
        crate::SpinCall { rounds: *input }
    }
    #[output]
    fn done(spin: &u64) -> u64 {
        *spin
    }
}
