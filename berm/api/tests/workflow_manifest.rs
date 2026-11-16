use berm_api::{
    Manifest,
    project::Project,
    workflow::config::{Config, Invalid},
};
use serde_json::json;
use std::str::FromStr;
mod support {
    pub mod workflow;
}

#[test]
fn workflow_configuration_round_trips_and_defaults_retention() {
    let source = r#"
[workflows.identity]
access = "public"
title = "Return a value"
price = 1000
max_cost = 2000
budget = { cpu = 1000, read = 0, written = 0, memory = 16 }
checkpoint_bytes = 4096
[workflows.identity.schedule]
cron = "0 8 * * *"
input = 42
"#;
    let project: Project = toml::from_str(source).unwrap();
    let config = &project.workflows["identity"];
    config.check().unwrap();
    assert_eq!(config.retain.as_secs(), 604800);
    let wire = serde_json::to_value(config).unwrap();
    assert_eq!(serde_json::from_value::<Config>(wire).unwrap(), *config);
    let image = support::workflow::image(serde_json::to_value(config).unwrap());
    let manifest = Manifest::try_from(image.as_slice()).unwrap();
    assert!(manifest.tools.is_empty());
    assert_eq!(manifest.workflows.len(), 1);
    let encoded = serde_json::to_string(&manifest).unwrap();
    assert_eq!(
        Manifest::from_str(&encoded).unwrap().workflows,
        manifest.workflows
    );
}

#[test]
fn unknown_configuration_invalid_schedule_and_input_are_workflow_failures() {
    for config in [
        json!({"max_cost":1000,"checkpoint_bytes":0}),
        json!({"max_cost":1000,"checkpoint_bytes":4096,"unexpected":true}),
        json!({"max_cost":1000,"checkpoint_bytes":4096,"schedule":{"cron":"0 8 * * *","input":"wrong"}}),
        json!({"max_cost":1000,"checkpoint_bytes":4096,"schedule":{"cron":"0 0 8 * * *","input":1}}),
        json!({"max_cost":1000,"checkpoint_bytes":4096,"budget":{"read":1}}),
    ] {
        let image = support::workflow::image(config);
        let error = Manifest::try_from(image.as_slice()).unwrap_err();
        assert!(error.is::<Invalid>(), "{error:#}");
    }
}

#[test]
fn workflow_control_exports_are_checked_without_running_builders() {
    let image = support::workflow::image(json!({"max_cost":1000,"checkpoint_bytes":4096}));
    assert!(Manifest::try_from(image.as_slice()).is_ok());
    let mut missing = image.clone();
    let symbol = b"berm_workflow_identity_done";
    let at = missing
        .windows(symbol.len())
        .position(|window| window == symbol)
        .unwrap();
    missing[at] = b'x';
    let error = Manifest::try_from(missing.as_slice()).unwrap_err();
    assert!(error.is::<Invalid>());
    assert!(
        error
            .to_string()
            .contains("missing workflow control export")
    );
}

#[test]
fn workflow_contract_changes_require_review_and_preserve_schema_compatibility() {
    let image = support::workflow::image(
        json!({"access":"public","max_cost":1000,"checkpoint_bytes":4096}),
    );
    let old = Manifest::try_from(image.as_slice()).unwrap();
    let mut next = old.clone();
    next.workflows[0].config.max_cost = 2000;
    assert!(old.upgrade_requires_review(&next));
    assert!(old.breaks(&next).is_empty());
    next.workflows[0].entry.graph.input = json!({"type":"string"});
    assert!(old.breaks(&next).iter().any(|change| {
        change
            .to_string()
            .contains("workflow identity: input changed")
    }));
    next.workflows.clear();
    assert_eq!(old.breaks(&next)[0].reason, "removed");
}

#[test]
fn workflow_error_context_is_not_duplicated() {
    let error = anyhow::Error::from(Invalid(
        anyhow::anyhow!("invalid parameter").context("workflow configuration"),
    ))
    .context("service check failed");
    assert!(error.is::<Invalid>());
    assert_eq!(
        format!("{error:#}"),
        "service check failed: workflow configuration: invalid parameter"
    );
}
