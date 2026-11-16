use berm_api::{
    host,
    workflow::{
        run::{Failure, Run, State},
        view::{self, Continuation},
    },
};
use serde_json::{Value, json};

#[test]
fn consumer_history_removes_metering_and_keeps_failure_recovery() {
    let mut native: Run =
        serde_json::from_str(include_str!("fixtures/workflow-run-v1.json")).unwrap();
    let failure = Failure {
        status: 402,
        headers: [
            ("BeRm-MeTeR".into(), "private-cost".into()),
            (host::CALLER.into(), "private-caller".into()),
            ("retry-after".into(), "3".into()),
            ("deployment-cost".into(), "private-cost".into()),
        ]
        .into(),
        body: json!({"error":"cap exhausted","explain":"E0061","recovery":{"action":"inspect"}}),
    };
    native.error = Some(failure.clone());
    for attempt in &mut native.attempts {
        attempt.error = Some(failure.clone());
    }
    for checkpoint in native.checkpoints.values_mut() {
        checkpoint.state = State::Failed {
            error: failure.clone(),
        };
        for item in &mut checkpoint.items {
            item.state = State::Unknown {
                error: failure.clone(),
            };
            for dispatch in &mut item.dispatches {
                dispatch.state = State::Failed {
                    error: failure.clone(),
                };
            }
        }
    }
    let mut consumer = view::Run::from_native(
        &native,
        Continuation::Unavailable {
            reason: failure.clone(),
        },
    );
    consumer.remove_error_headers(&["deployment-cost"]);
    let encoded = serde_json::to_string(&consumer).unwrap();
    assert!(!encoded.contains("private-cost"));
    assert!(!encoded.contains("private-caller"));
    let value: Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(value["error"]["status"], 402);
    assert_eq!(value["error"]["body"], failure.body);
    assert_eq!(value["error"]["headers"]["retry-after"], "3");
    assert_eq!(value["continuation"]["reason"]["body"]["explain"], "E0061");
    assert!(native.error.unwrap().headers.contains_key("BeRm-MeTeR"));
}
