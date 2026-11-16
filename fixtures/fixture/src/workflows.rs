use alloc::{string::String, vec::Vec};
use berm_lang::{Branch, Result, Schema, tool, workflow};

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

#[derive(Schema)]
struct FlowInput {
    count: u32,
    take: bool,
    fail_at: Option<u32>,
}
#[derive(Schema)]
struct Proceed {
    fail_at: Option<u32>,
}
#[derive(Schema)]
struct Stop {}
#[derive(Schema, Branch)]
enum Route {
    Yes(Proceed),
    No(Stop),
}
#[derive(Schema)]
struct FlowOutput {
    left: u64,
    right: u64,
    values: Vec<u32>,
    optional: Option<u32>,
    maybe: Option<u32>,
    identity: String,
}

/// Append an item; fail_at requests a transactional failure.
#[tool]
fn workflow_item(index: u32, fail_at: Option<u32>) -> Result<u32> {
    let mut items: Vec<u32> = berm_lang::get("workflow_items")?
        .map(|bytes| berm_lang::serde_json::from_slice(&bytes))
        .transpose()?
        .unwrap_or_default();
    items.push(index);
    berm_lang::set("workflow_items", &berm_lang::json::to(&items)?.into_bytes())?;
    if Some(index) == fail_at {
        return Err("item failed".into());
    }
    Ok(index)
}

struct WorkflowFlow;
#[workflow]
impl WorkflowFlow {
    #[step]
    fn left(input: &FlowInput) -> crate::SpinCall {
        let _ = input;
        crate::SpinCall { rounds: 3 }
    }
    #[step]
    fn right(input: &FlowInput) -> crate::SpinCall {
        let _ = input;
        crate::SpinCall { rounds: 4 }
    }
    #[branch]
    fn choose(input: &FlowInput) -> Route {
        if input.take {
            Route::Yes(Proceed {
                fail_at: input.fail_at,
            })
        } else {
            Route::No(Stop {})
        }
    }
    #[step]
    fn optional(choose: &Proceed) -> WorkflowItemCall {
        WorkflowItemCall {
            index: 99,
            fail_at: choose.fail_at,
        }
    }
    #[step]
    fn maybe(input: &FlowInput) -> Option<WorkflowItemCall> {
        input.take.then_some(WorkflowItemCall {
            index: 88,
            fail_at: input.fail_at,
        })
    }
    #[step(each, max = 3)]
    fn items(input: &FlowInput) -> Vec<WorkflowItemCall> {
        (0..input.count)
            .map(|index| WorkflowItemCall {
                index,
                fail_at: input.fail_at,
            })
            .collect()
    }
    #[step]
    fn identity(input: &FlowInput) -> crate::WhoamiCall {
        let _ = input;
        crate::WhoamiCall {}
    }
    #[output]
    fn done(
        left: &u64,
        right: &u64,
        optional: Option<&u32>,
        maybe: Option<&u32>,
        items: &[u32],
        identity: &String,
    ) -> FlowOutput {
        FlowOutput {
            left: *left,
            right: *right,
            values: items.to_vec(),
            optional: optional.copied(),
            maybe: maybe.copied(),
            identity: identity.clone(),
        }
    }
}

struct WorkflowExternal;
#[workflow]
impl WorkflowExternal {
    #[step]
    fn ask(input: &u64) -> crate::fixture::WhoamiCall {
        let _ = input;
        crate::fixture::WhoamiCall {}
    }
    #[output]
    fn done(ask: &String) -> String {
        ask.clone()
    }
}

struct WorkflowFlaky;
#[workflow]
impl WorkflowFlaky {
    #[step]
    fn call(input: &String) -> crate::FlakyCall {
        crate::FlakyCall { url: input.clone() }
    }
    #[output]
    fn done(call: &()) -> () {
        let _ = call;
    }
}

#[derive(Schema)]
struct NestedInput {
    service: String,
    tool: String,
    args: String,
}
struct WorkflowNested;
#[workflow]
impl WorkflowNested {
    #[step(each, max = 2)]
    fn calls(input: &NestedInput) -> Vec<crate::InvokeCall> {
        (0..2)
            .map(|_| crate::InvokeCall {
                service: input.service.clone(),
                tool: input.tool.clone(),
                args: input.args.clone(),
                delay: None,
            })
            .collect()
    }
    #[output]
    fn done(calls: &[String]) -> Vec<String> {
        calls.to_vec()
    }
}
