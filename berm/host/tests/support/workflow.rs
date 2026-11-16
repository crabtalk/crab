use berm_api::{
    Budget, Manifest, digest,
    workflow::{
        Version,
        graph::{Kind, Source, Target},
        run::{Accepted, Binding, Run, Status, Trigger},
    },
};
use serde_json::{Value, json};
#[allow(dead_code)]
#[path = "../../../api/tests/support/workflow.rs"]
mod workflow;
pub const CAPACITY: u64 = 32768;

pub fn image(inputs: &[Value]) -> Vec<u8> {
    let output = json!({"kind":"calls","inputs":inputs}).to_string();
    let wasm = wat::parse_str(format!("(module (import \"berm\" \"berm.done\" (func $done (param i32 i32) (result i64))) (memory (export \"memory\") 1) (data (i32.const 0) {output:?}) (func (export \"berm_workflow_identity_send\") i32.const 0 i32.const {} call $done drop) (func (export \"berm_workflow_identity_done\")))", output.len())).unwrap();
    let schema = json!({"type":"integer"});
    workflow::with_record(
        &wasm,
        json!({
            "workflow":{"version":1,"name":"identity","graph":{
                "input":schema,"output":{"type":"array","items":schema},
                "nodes":[
                    {"id":"send","export":"berm_workflow_identity_send","kind":"step","call":{"target":{"kind":"service","service":"mail/send","tool":"send","idempotency_ms":10000},"input":schema,"output":schema,"cases":[]},"mode":{"kind":"each","max":3},"dependencies":[{"source":{"kind":"input"},"optional":false,"case":null}],"guards":[]},
                    {"id":"done","export":"berm_workflow_identity_done","kind":"output","dependencies":[{"source":{"kind":"node","id":"send"},"optional":false,"case":null}],"guards":[]}
                ]}},"abi_version":berm::abi::ABI_VERSION,"crate":"workflow-only"
        }),
        json!({"max_cost":1000,"checkpoint_bytes":CAPACITY,"retain":"1s"}),
    )
}

pub fn run(id: &str, image: &[u8]) -> Run {
    let manifest = Manifest::try_from(image).unwrap();
    let spec = &manifest.workflows[0];
    let budget = Budget {
        cpu: 1000,
        read: 0,
        written: 0,
        memory: 1 << 20,
    };
    let bindings = spec
        .entry
        .graph
        .nodes
        .iter()
        .filter_map(|node| match &node.kind {
            Kind::Step { call, .. } => Some((
                node.id.clone(),
                Binding {
                    target: call.target.clone(),
                    service: "mail/send".into(),
                    image: "accepted-provider-image".into(),
                    input: call.input.clone(),
                    output: call.output.clone(),
                    budget,
                    query: false,
                    idempotency_ms: match call.target {
                        Target::Service { idempotency_ms, .. } => idempotency_ms,
                        _ => None,
                    },
                    authorization: "revision-1".into(),
                },
            )),
            _ => None,
        })
        .collect();
    Run {
        version: Version::V1,
        id: id.into(),
        service: "owner/identity".into(),
        workflow: "identity".into(),
        source: Source {
            image: digest::of(image),
            graph: spec.entry.digest().unwrap(),
        },
        input: json!(2),
        trigger: Trigger::Schedule {
            key: format!("occurrence-{id}"),
            at: 100,
        },
        created_at: 100,
        expires_at: 1100,
        status: Status::Pending,
        accepted: Accepted {
            bindings,
            control_budget: budget,
            control_buffer: manifest.buffer as u64,
            checkpoint_bytes: CAPACITY,
            deadline_ms: 100,
        },
        attempts: vec![],
        checkpoints: Default::default(),
        result: None,
        error: None,
    }
}
