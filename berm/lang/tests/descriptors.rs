use berm_lang::{
    Branch, Result, Schema,
    schema::{self, Piece},
    serde_json::{self, Value, json},
    tool,
    workflow::{Branch as BranchContract, Call, Projection, Target},
};

berm_lang::declare_service!(variants);
berm_lang::declare_service!(nullable);
berm_lang::declare_service!(workflow_calls);

#[derive(Schema)]
struct Changed {
    value: String,
}
#[derive(Schema)]
struct Same {
    reason: String,
}
#[derive(Schema, Branch)]
enum Route {
    Changed(Changed),
    Same(Same),
}

/// Compare the input.
#[tool]
fn compare(value: String, note: Option<String>) -> Result<Route> {
    let _ = note;
    Ok(Route::Changed(Changed { value }))
}

/// Complete without a result body.
#[tool]
fn complete() -> Result<()> {
    Ok(())
}

#[test]
fn descriptors_keep_targets_inputs_outputs_and_aliases_typed() {
    type Alias = CompareCall;
    let args = Alias {
        value: "new".into(),
        note: None,
    };
    assert_eq!(serde_json::to_value(&args).unwrap(), json!({"value":"new"}));
    assert_eq!(Alias::TARGET, Target::Local { tool: "compare" });
    assert_eq!(Alias::CASES.len(), 2);
    assert_eq!(Alias::CASES[0].name, "changed");
    let result: <Alias as Call>::Output = compare(args.value, args.note).unwrap();
    assert_eq!(BranchContract::case(&result), "changed");
    assert_eq!(Projection::<Changed>::project(result).unwrap().value, "new");
    assert_eq!(CompleteCall::TARGET, Target::Local { tool: "complete" });
    let _: <CompleteCall as Call>::Output = complete().unwrap();
    let remote = workflow_calls::SendCall {
        r#type: "report".into(),
        note: None,
    };
    assert_eq!(
        serde_json::to_value(&remote).unwrap(),
        json!({"type":"report"})
    );
    assert_eq!(
        workflow_calls::SendCall::TARGET,
        Target::Service {
            service: "owner/mail",
            tool: "send",
            idempotency_ms: Some(86_400_000)
        }
    );
    const REMOTE: &[Piece] = &[Piece::Shape(
        &<workflow_calls::SendCall as Call>::Output::SHAPE,
    )];
    const BYTES: [u8; schema::len(REMOTE)] = schema::render(REMOTE);
    assert_eq!(
        serde_json::from_slice::<Value>(&BYTES).unwrap()["title"],
        "Receipt"
    );
    assert!(matches!(
        nullable::RecallCall::TARGET,
        Target::Service {
            idempotency_ms: None,
            ..
        }
    ));
    let _: Option<nullable::types::Entry> =
        None::<<nullable::RecallCall as Call>::Output>.flatten();
}

#[test]
fn unselected_payloads_are_absent_and_selected_payloads_keep_their_schema() {
    let value = Route::Same(Same {
        reason: "unchanged".into(),
    });
    assert!(Projection::<Changed>::project(value).is_none());
    assert_eq!(<Route as Projection<Same>>::CASE, Some("same"));
    const PIECES: &[Piece] = &[Piece::Shape(
        <Route as BranchContract>::CASES[1].payload.unwrap(),
    )];
    const BYTES: [u8; schema::len(PIECES)] = schema::render(PIECES);
    let shape: Value = serde_json::from_slice(&BYTES).unwrap();
    assert_eq!(shape["required"], json!(["reason"]));
}

#[test]
fn imported_union_cases_project_without_changing_existing_client_types() {
    use variants::types::{
        Platform, PlatformCustomPayload, PlatformGrokPayload, PlatformLocalPayload,
    };
    let custom = Platform::Custom {
        endpoint: "https://example.com".into(),
        label: None,
    };
    let payload: PlatformCustomPayload = Projection::project(custom).unwrap();
    assert_eq!(payload.endpoint, "https://example.com");
    let local = Projection::<PlatformLocalPayload>::project(Platform::Local).unwrap();
    assert_eq!(serde_json::to_value(local).unwrap(), json!({}));
    let value = Platform::Grok(variants::types::Connection {
        url: "a".into(),
        credential: None,
    });
    let payload: PlatformGrokPayload = Projection::project(value).unwrap();
    assert_eq!(payload.0.url, "a");
    assert_eq!(variants::PlatformCall::CASES.len(), 3);
    assert_eq!(variants::PlatformCall::CASES[0].name, "custom");
}

#[test]
fn anonymous_imported_unions_have_typed_descriptors_and_keep_value_clients() {
    use workflow_calls::types::{ChooseOutput, ChooseOutputReadyPayload};
    let value = ChooseOutput::Ready(workflow_calls::types::Receipt { id: "r1".into() });
    let payload: ChooseOutputReadyPayload = Projection::project(value).unwrap();
    assert_eq!(payload.0.id, "r1");
    assert_eq!(workflow_calls::ChooseCall::CASES.len(), 2);
    berm_lang::test::answer("owner/mail", "choose", Ok(br#"{"ready":{"id":"r1"}}"#));
    let value: Value = workflow_calls::calls::choose().unwrap();
    assert_eq!(value["ready"]["id"], "r1");
}
