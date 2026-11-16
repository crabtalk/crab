use anyhow::Result;
use berm_api::host;
use clap::Parser;
use crab_agent::cmd::workflow::{self, Workflow};
use crabtalk_sdk::{
    client::{Client, Owner},
    services::workflow::{Inspection, Run},
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, ExitCode},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const RUN: &str = "9e9fbff7-93b3-4d79-bf73-2c39fc038cb0";

#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    workflow: Workflow,
}

struct Gateway {
    client: Client,
    requests: JoinHandle<Vec<(String, Value)>>,
}

impl Gateway {
    fn spawn(owner: Option<Owner>, replies: Vec<(&'static str, u16, Value)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let requests = thread::spawn(move || {
            let mut requests = Vec::new();
            for (expected, status, response) in replies {
                let deadline = Instant::now() + Duration::from_secs(10);
                let mut stream: TcpStream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "missing {expected}");
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("{error}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                let mut chunk = [0; 4096];
                let (headers, body) = loop {
                    let size = stream.read(&mut chunk).unwrap();
                    assert!(size > 0, "incomplete request");
                    request.extend_from_slice(&chunk[..size]);
                    let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n")
                    else {
                        continue;
                    };
                    let headers = String::from_utf8(request[..end].to_vec()).unwrap();
                    let size = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|length| length.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + size {
                        let body = if size == 0 {
                            Value::Null
                        } else {
                            serde_json::from_slice(&request[end + 4..end + 4 + size]).unwrap()
                        };
                        break (headers, body);
                    }
                };
                assert!(headers.starts_with(expected), "{headers}");
                requests.push((headers, body));
                let body = response.to_string();
                write!(stream, "HTTP/1.1 {status} Response\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
            requests
        });
        let mut client = Client::new(format!("http://{address}"));
        if let Some(owner) = owner {
            client = client.owner(owner).token("credential");
        }
        Self { client, requests }
    }
}

fn inspection() -> Value {
    let terms: Value = serde_json::from_str(include_str!(
        "../../sdk/tests/fixtures/workflow-terms-v1.json"
    ))
    .unwrap();
    json!({
        "version": 1, "service": "alice/research", "title": "Research report", "source": terms["source"],
        "quote": "quote-1", "task_price": 50_000, "deadline_ms": 30_000, "retention_ms": 604_800_000,
        "entry": {"version": 1, "name": "report", "graph": {
            "input": {"type":"object", "properties":{"question":{"type":"string"}},"required":["question"]}, "output": {"type":"object"},
            "nodes": [
                {"id":"search","export":"search","dependencies":[{"source":{"kind":"input"},"optional":false,"case":null}],"guards":[], "kind":"step", "call":{"target":{"kind":"service","service":"owner/search","tool":"search","idempotency_ms":null},"input":{},"output":{},"cases":[]}, "mode":{"kind":"each","max":12}},
                {"id":"report","export":"report","dependencies":[{"source":{"kind":"node","id":"search"},"optional":false,"case":null}],"guards":[],"kind":"output"}
            ]
        }},
        "accounting": terms
    })
}

fn record(status: &str) -> Value {
    let accepted = inspection();
    json!({
        "version":1,"id":RUN,"service":"alice/research","workflow":"report","source":accepted["source"],
        "input":{"question":"why"},"created_at":1000,"expires_at":604_801_000,"status":status,
        "attempts":[],"checkpoints":{"search":{"state":{"state":"skipped"},"items":[]}},
        "result":null,"error":null,"continuation":{"state":"available"},"accepted":accepted,
        "accounting":{"terms":accepted["accounting"],"spend":{"run":RUN,"incurred":"120","reserved":"80"},"settlement":{"state":"held","reservation":"hold-1"}}
    })
}

#[test]
fn inspection_and_dry_run_resolve_without_admitting_and_validate_input() -> Result<()> {
    for arguments in [
        vec!["workflow", "inspect", "research.report"],
        vec!["workflow", "dry-run", "research.report"],
        vec![
            "workflow",
            "run",
            "research.report",
            r#"{"question":"why"}"#,
            "--dry-run",
        ],
    ] {
        let gateway = Gateway::spawn(
            None,
            vec![(
                "GET /services/local/research/workflows/report ",
                200,
                inspection(),
            )],
        );
        let workflow = Cli::try_parse_from(arguments)?.workflow;
        assert_eq!(
            workflow.run(&gateway.client, true, false)?,
            ExitCode::SUCCESS
        );
        let requests = gateway.requests.join().unwrap();
        assert_eq!(requests.len(), 1);
        let headers = requests[0].0.to_ascii_lowercase();
        assert!(headers.contains(host::CALLER));
        assert!(headers.contains("local"));
    }
    let gateway = Gateway::spawn(
        None,
        vec![(
            "GET /services/local/research/workflows/report ",
            200,
            inspection(),
        )],
    );
    let workflow =
        Cli::try_parse_from(["workflow", "run", "research.report", "{}", "--dry-run"])?.workflow;
    assert!(
        workflow
            .run(&gateway.client, true, false)
            .unwrap_err()
            .to_string()
            .contains("schema")
    );
    gateway.requests.join().unwrap();
    Ok(())
}

#[test]
fn cloud_run_sends_the_accepted_quote_and_explicit_stable_identity() -> Result<()> {
    let gateway = Gateway::spawn(
        Some(Owner::Org("team".into())),
        vec![
            (
                "GET /services/alice/research/workflows/report ",
                200,
                inspection(),
            ),
            (
                "POST /services/alice/research/workflows/report/runs ",
                202,
                record("running"),
            ),
        ],
    );
    let workflow = Cli::try_parse_from([
        "workflow",
        "run",
        "alice/research.report",
        r#"{"question":"why"}"#,
        "--id",
        RUN,
    ])?
    .workflow;
    assert_eq!(
        workflow.run(&gateway.client, true, true)?,
        ExitCode::SUCCESS
    );
    let requests = gateway.requests.join().unwrap();
    assert_eq!(
        requests[1].1,
        json!({"id":RUN,"quote":"quote-1","input":{"question":"why"}})
    );
    for (headers, _) in requests {
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("authorization: bearer credential")
        );
        assert!(!headers.to_ascii_lowercase().contains(host::CALLER));
    }
    Ok(())
}

#[test]
fn status_continue_and_cancel_use_the_same_service_scoped_run() -> Result<()> {
    let gateway = Gateway::spawn(
        None,
        vec![
            (
                "GET /services/local/research/workflows/report/runs/",
                200,
                record("failed"),
            ),
            (
                "GET /services/local/research/workflows/report/runs/",
                200,
                record("failed"),
            ),
            (
                "POST /services/local/research/workflows/report/runs/9e9fbff7-93b3-4d79-bf73-2c39fc038cb0/continue ",
                202,
                record("running"),
            ),
            (
                "POST /services/local/research/workflows/report/runs/9e9fbff7-93b3-4d79-bf73-2c39fc038cb0/cancel ",
                200,
                record("cancelled"),
            ),
        ],
    );
    for (action, expected) in [
        ("status", ExitCode::from(2)),
        ("continue", ExitCode::SUCCESS),
        ("cancel", ExitCode::SUCCESS),
    ] {
        let workflow = Cli::try_parse_from(["workflow", action, "research.report", RUN])?.workflow;
        assert_eq!(workflow.run(&gateway.client, true, true)?, expected);
    }
    assert_eq!(gateway.requests.join().unwrap().len(), 4);
    Ok(())
}

#[test]
fn refusal_keeps_status_and_explanation_and_cannot_admit_a_run() -> Result<()> {
    let gateway = Gateway::spawn(
        None,
        vec![(
            "GET /services/local/research/workflows/report ",
            409,
            json!({"error":"workflow terms changed", "explain":"E0060"}),
        )],
    );
    let workflow = Cli::try_parse_from([
        "workflow",
        "run",
        "research.report",
        r#"{"question":"why"}"#,
    ])?
    .workflow;
    let error = workflow.run(&gateway.client, true, true).unwrap_err();
    let refusal = error.downcast_ref::<crabtalk_sdk::client::Error>().unwrap();
    assert!(
        matches!(refusal, crabtalk_sdk::client::Error::Api {status:409, explain:Some(code), ..} if code == "E0060")
    );
    gateway.requests.join().unwrap();
    Ok(())
}

#[test]
fn unavailable_continuation_does_not_send_a_continue_request() -> Result<()> {
    let mut run = record("indeterminate");
    run["continuation"] = json!({"state":"unavailable", "reason":{"status":409,"headers":{},"body":{"error":"unknown external write", "explain":"E0067"}}});
    let gateway = Gateway::spawn(
        None,
        vec![(
            "GET /services/local/research/workflows/report/runs/",
            200,
            run,
        )],
    );
    let workflow = Cli::try_parse_from(["workflow", "continue", "research.report", RUN])?.workflow;
    let error = workflow.run(&gateway.client, true, true).unwrap_err();
    let failure = error
        .downcast_ref::<berm_api::workflow::run::Failure>()
        .unwrap();
    assert_eq!(failure.status, 409);
    assert_eq!(failure.body["explain"], "E0067");
    gateway.requests.join().unwrap();
    Ok(())
}

#[test]
fn human_output_distinguishes_graph_price_quote_cap_and_eligibility() -> Result<()> {
    let inspection: Inspection = serde_json::from_value(inspection())?;
    let run: Run = serde_json::from_value(record("failed"))?;
    let mut output = Vec::new();
    workflow::show_inspection(&inspection, &mut output)?;
    workflow::show_run(&run, &mut output)?;
    let output = String::from_utf8(output)?;
    for expected in [
        "fixed task price: $0.05",
        "owner spend cap: $0.04",
        "direct-call/control quote: $0.03",
        "at most 12 calls",
        "report <- [search]: output",
        "continuation available",
        "$0.00012 incurred, $0.000080 reserved, $0.04 cap",
        "search: skipped",
    ] {
        assert!(output.contains(expected), "{expected}: {output}");
    }
    Ok(())
}

#[test]
fn workflow_command_dispatch_rejects_invalid_targets_and_ids() -> Result<()> {
    for arguments in [
        vec!["workflow", "run", "research", "{}"],
        vec!["workflow", "status", "research.report", "not-a-uuid"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_crab"))
            .args(arguments)
            .output()?;
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("invalid value"));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_crab"))
        .args(["workflow", "--help"])
        .output()?;
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout)?;
    for action in ["inspect", "run", "status", "continue", "cancel"] {
        assert!(help.contains(action));
    }
    Ok(())
}

#[test]
fn local_admission_uses_the_selected_unchecked_caller_and_anonymous_dry_run_is_safe() -> Result<()>
{
    let gateway = Gateway::spawn(
        None,
        vec![
            (
                "GET /services/local/research/workflows/report ",
                200,
                inspection(),
            ),
            (
                "POST /services/local/research/workflows/report/runs ",
                202,
                record("running"),
            ),
        ],
    );
    let workflow = Cli::try_parse_from([
        "workflow",
        "run",
        "research.report",
        r#"{"question":"why"}"#,
        "--caller",
        "alice",
        "--id",
        RUN,
    ])?
    .workflow;
    assert_eq!(
        workflow.run(&gateway.client, true, true)?,
        ExitCode::SUCCESS
    );
    let requests = gateway.requests.join().unwrap();
    for (headers, _) in requests {
        let headers = headers.to_ascii_lowercase();
        assert!(headers.contains(host::CALLER));
        assert!(headers.contains("alice"));
    }
    let gateway = Gateway::spawn(
        None,
        vec![(
            "GET /services/local/research/workflows/report ",
            200,
            inspection(),
        )],
    );
    let workflow = Cli::try_parse_from([
        "workflow",
        "run",
        "research.report",
        r#"{"question":"why"}"#,
        "--anonymous",
        "--dry-run",
    ])?
    .workflow;
    assert_eq!(
        workflow.run(&gateway.client, true, false)?,
        ExitCode::SUCCESS
    );
    assert!(
        !gateway.requests.join().unwrap()[0]
            .0
            .to_ascii_lowercase()
            .contains(host::CALLER)
    );
    Ok(())
}
