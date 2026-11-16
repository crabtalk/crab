use berm_api::workflow::graph::{Entry, Kind, Mode, Selection};
use berm_lang::{
    Branch, Result, Schema,
    abi::workflow::{ControlInput, ControlOutput, ControlValue},
    serde_json::{self, Value, json},
    test, tool, workflow,
};

#[derive(Schema)]
struct Query {
    count: u64,
    enabled: bool,
}
#[derive(Schema)]
struct Changed {
    count: u64,
}
#[derive(Schema)]
struct Same {
    count: u64,
}
#[derive(Schema, Branch)]
enum Route {
    Changed(Changed),
    Same(Same),
}

/// Return a number.
#[tool]
fn add(value: u64) -> Result<u64> {
    Ok(value + 1)
}
type Add = AddCall;

struct Track;
#[workflow]
impl Track {
    #[step]
    fn first(input: &Query) -> Result<Add> {
        if input.count == 999 {
            return Err("invalid count".into());
        }
        Ok(Add { value: input.count })
    }
    #[step]
    fn second(input: &Query) -> Add {
        Add {
            value: input.count + 1,
        }
    }
    #[branch]
    fn route(first: &u64, second: &u64) -> Route {
        if first != second {
            Route::Changed(Changed { count: *first })
        } else {
            Route::Same(Same { count: *first })
        }
    }
    #[step(each, max = 3)]
    fn read(input: &Query, first: &u64) -> Vec<Add> {
        (0..input.count).map(|i| Add { value: first + i }).collect()
    }
    #[step]
    fn note(input: &Query) -> Option<Add> {
        input.enabled.then_some(Add { value: 7 })
    }
    #[step]
    fn notify(route: &Changed) -> Add {
        Add { value: route.count }
    }
    #[output]
    fn report(read: &[u64], note: Option<&u64>, notify: Option<&u64>, route: &Route) -> u64 {
        let _ = route;
        read.iter().sum::<u64>() + note.copied().unwrap_or(0) + notify.copied().unwrap_or(0)
    }
}

fn call(export: extern "C" fn(), nodes: Value, count: u64) -> Result<ControlOutput, String> {
    let nodes = nodes
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, value)| {
            (
                name.clone(),
                if *value == json!("skipped") {
                    ControlValue::Skipped
                } else {
                    ControlValue::Present {
                        value: value.clone(),
                    }
                },
            )
        })
        .collect();
    let input = ControlInput {
        input: json!({"count":count,"enabled":false}),
        nodes,
    };
    let bytes = test::call(export, &serde_json::to_vec(&input).unwrap())?;
    Ok(serde_json::from_slice(&bytes).unwrap())
}

#[test]
fn static_records_contain_typed_shapes_guards_and_symbolic_bounds() {
    let record: Value = serde_json::from_slice(Track::WORKFLOW).unwrap();
    let entry: Entry = serde_json::from_value(record["workflow"].clone()).unwrap();
    entry.graph.check(100).unwrap();
    assert_eq!(entry.name, "track");
    let Kind::Step { mode, .. } = &entry.graph.nodes[3].kind else {
        panic!("lost each")
    };
    assert_eq!(*mode, Mode::Each { max: 3 });
    assert_eq!(entry.graph.nodes[5].guards[0].producer, "route");
    assert_eq!(
        entry.graph.nodes[5].guards[0].selection,
        Selection::Case {
            name: "changed".into()
        }
    );
    assert!(entry.graph.nodes[6].guards.is_empty());
}

#[test]
fn adapters_prepare_calls_each_lists_and_optional_skips_without_dispatch() {
    let ControlOutput::Calls { inputs } = call(berm_workflow_track_first, json!({}), 2).unwrap()
    else {
        panic!("no prepared call")
    };
    assert_eq!(inputs, vec![json!({"value":2})]);
    let ControlOutput::Calls { inputs } =
        call(berm_workflow_track_read, json!({"first":3}), 2).unwrap()
    else {
        panic!("no each list")
    };
    assert_eq!(inputs, vec![json!({"value":3}), json!({"value":4})]);
    assert!(matches!(
        call(berm_workflow_track_note, json!({}), 2).unwrap(),
        ControlOutput::Skipped
    ));
    assert!(
        call(berm_workflow_track_read, json!({"first":3}), 4)
            .unwrap_err()
            .contains("exceeds its each bound of 3")
    );
    assert_eq!(
        call(berm_workflow_track_first, json!({}), 999).unwrap_err(),
        "invalid count"
    );
}

#[test]
fn branch_projections_and_optional_joins_preserve_selected_values() {
    let ControlOutput::Branch { case, value } =
        call(berm_workflow_track_route, json!({"first":2,"second":3}), 2).unwrap()
    else {
        panic!("no branch")
    };
    assert_eq!(case, "changed");
    assert_eq!(value, json!({"changed":{"count":2}}));
    let ControlOutput::Calls { inputs } = call(
        berm_workflow_track_notify,
        json!({"route":value.clone()}),
        2,
    )
    .unwrap() else {
        panic!("no notification")
    };
    assert_eq!(inputs[0], json!({"value":2}));
    assert!(
        call(
            berm_workflow_track_notify,
            json!({"route":{"same":{"count":2}}}),
            2
        )
        .unwrap_err()
        .contains("is skipped")
    );
    let ControlOutput::Output { value: result } = call(
        berm_workflow_track_report,
        json!({"read":[3,4],"note":"skipped","notify":5,"route":value}),
        2,
    )
    .unwrap() else {
        panic!("no output")
    };
    assert_eq!(result, 12);
    assert!(
        call(
            berm_workflow_track_report,
            json!({"read":[3],"note":null,"notify":"skipped","route":{"same":{"count":2}}}),
            2
        )
        .is_err()
    );
}

berm_lang::declare_service!(variants);
struct Remote;
#[workflow]
impl Remote {
    #[step]
    fn get(input: &Query) -> variants::PlatformCall {
        let _ = input;
        variants::PlatformCall {}
    }
    #[step]
    fn custom(get: &variants::types::PlatformCustomPayload) -> Add {
        Add {
            value: get.endpoint.len() as u64,
        }
    }
    #[step]
    fn grok(get: &variants::types::PlatformGrokPayload) -> Add {
        Add {
            value: get.0.url.len() as u64,
        }
    }
    #[output]
    fn done(custom: Option<&u64>, grok: Option<&u64>) -> u64 {
        custom.or(grok).copied().unwrap_or(0)
    }
}

#[test]
fn union_call_results_activate_typed_arms_and_join_without_cloning_payloads() {
    let record: Value = serde_json::from_slice(Remote::WORKFLOW).unwrap();
    let entry: Entry = serde_json::from_value(record["workflow"].clone()).unwrap();
    entry.graph.check(100).unwrap();
    assert_eq!(
        entry.graph.nodes[1].dependencies[0].case.as_deref(),
        Some("custom")
    );
    let ControlOutput::Calls { inputs } = call(
        berm_workflow_remote_custom,
        json!({"get":{"custom":{"endpoint":"abc"}}}),
        0,
    )
    .unwrap() else {
        panic!("no custom arm")
    };
    assert_eq!(inputs[0], json!({"value":3}));
    let ControlOutput::Output { value } = call(
        berm_workflow_remote_done,
        json!({"custom":"skipped","grok":4}),
        0,
    )
    .unwrap() else {
        panic!("no join")
    };
    assert_eq!(value, 4);
}

/// Return an absent number.
#[tool]
fn nullable() -> Result<Option<u64>> {
    Ok(None)
}
struct NullResult;
#[workflow]
impl NullResult {
    #[step]
    fn read(input: &Query) -> NullableCall {
        let _ = input;
        NullableCall {}
    }
    #[output]
    fn done(read: Option<&Option<u64>>) -> bool {
        read.is_some()
    }
}

#[test]
fn a_successful_null_value_is_not_a_skipped_dependency() {
    let ControlOutput::Output { value } =
        call(berm_workflow_null_result_done, json!({"read":null}), 0).unwrap()
    else {
        panic!("no result")
    };
    assert_eq!(value, true);
    let ControlOutput::Output { value } =
        call(berm_workflow_null_result_done, json!({"read":"skipped"}), 0).unwrap()
    else {
        panic!("no result")
    };
    assert_eq!(value, false);
}
