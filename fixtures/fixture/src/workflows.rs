use alloc::{string::String, vec::Vec};
use berm_lang::{Branch, Result, Schema, tool, workflow};

struct WorkflowExample;
/// Count work
#[workflow(price = 1000, max_cost = 10000, checkpoint_bytes = 1048576)]
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

/// Append an item; fail_at requests a transactional failure
///
/// Append an item; fail_at requests a transactional failure.
#[tool(access = "internal")]
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
/// Workflow Flow
#[workflow(max_cost = 10000, checkpoint_bytes = 1048576)]
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
        identity: &str,
    ) -> FlowOutput {
        FlowOutput {
            left: *left,
            right: *right,
            values: items.to_vec(),
            optional: optional.copied(),
            maybe: maybe.copied(),
            identity: identity.into(),
        }
    }
}

struct WorkflowExternal;
/// Workflow External
#[workflow(max_cost = 10000, checkpoint_bytes = 1048576)]
impl WorkflowExternal {
    #[step]
    fn ask(input: &u64) -> crate::fixture::WhoamiCall {
        let _ = input;
        crate::fixture::WhoamiCall {}
    }
    #[output]
    fn done(ask: &str) -> String {
        ask.into()
    }
}

struct WorkflowFlaky;
/// Workflow Flaky
#[workflow(max_cost = 10000, checkpoint_bytes = 1048576)]
impl WorkflowFlaky {
    #[step]
    fn call(input: &str) -> crate::FlakyCall {
        crate::FlakyCall { url: input.into() }
    }
    #[output]
    fn done(call: &()) {
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
/// Workflow Nested
#[workflow(max_cost = 2000, checkpoint_bytes = 1048576)]
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

/// Workflow mail
///
/// The endpoint must retain idempotency keys for at least ten seconds.
#[tool(idempotency = "10s")]
fn workflow_mail(url: String, idempotency_key: String) -> Result<String> {
    let response = berm_lang::http::fetch(
        "POST",
        &url,
        &[("Idempotency-Key", &idempotency_key)],
        b"hello",
    )?;
    if response.status != 200 {
        return Err("mail delivery was not acknowledged".into());
    }
    Ok(String::from_utf8(response.body).map_err(|_| "mail response is not UTF-8")?)
}

#[derive(Schema)]
struct MailInput {
    url: String,
    keys: Vec<String>,
}

struct WorkflowMail;
/// Workflow Mail
#[workflow(max_cost = 10000, checkpoint_bytes = 1048576)]
impl WorkflowMail {
    #[step(each, max = 3)]
    fn messages(input: &MailInput) -> Vec<WorkflowMailCall> {
        input
            .keys
            .iter()
            .map(|key| WorkflowMailCall {
                url: input.url.clone(),
                idempotency_key: key.clone(),
            })
            .collect()
    }
    #[output]
    fn done(messages: &[String]) -> Vec<String> {
        messages.to_vec()
    }
}
