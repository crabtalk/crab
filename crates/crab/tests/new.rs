use anyhow::Result;
use berm_api::{
    Output,
    host::{Caller, Deploy},
};
use crab_agent::host::Node;
use serde_json::json;
use std::{fs, path::Path, process::Command, time::Duration};
use tokio::time;

#[tokio::test]
async fn renamed_starter_tools_build_and_state_survives_callers_and_restarts() -> Result<()> {
    let root = tempfile::tempdir()?;
    let output = Command::new(env!("CARGO_BIN_EXE_crab"))
        .args(["new", "notes"])
        .current_dir(root.path())
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let project = root.path().join("notes");
    assert!(!project.join("tests/tools.rs").exists());
    let manifest = project.join("Cargo.toml");
    let lang = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../berm/lang")
        .canonicalize()?;
    let text = fs::read_to_string(&manifest)?;
    let dependency = text
        .lines()
        .find(|line| line.starts_with("berm-lang ="))
        .unwrap();
    fs::write(
        &manifest,
        text.replace(dependency, &format!("berm-lang = {{ path = {:?} }}", lang)),
    )?;
    let source = project.join("src/lib.rs");
    fs::write(
        &source,
        fs::read_to_string(&source)?.replace("pub fn echo(", "pub fn greeting("),
    )?;
    let config = project.join("crab.toml");
    fs::write(
        &config,
        fs::read_to_string(&config)?.replace("[tools.echo]", "[tools.greeting]"),
    )?;
    let image = fs::read(berm_build::build(&project, None)?)?;
    let state = root.path().join("host");
    let node = Node::new(
        state.clone(),
        state.join("secrets.toml"),
        4,
        Duration::from_secs(1),
        Duration::from_secs(30),
        None,
    )
    .await?;
    let digest = node.keep(&image).await?;
    let deploy: Deploy =
        serde_json::from_value(json!({"reference":digest,"digest":digest,"force":true}))?;
    node.deploy("notes", deploy).await?;
    let alice: Caller = serde_json::from_value(json!({"id":"alice"}))?;
    let bob: Caller = serde_json::from_value(json!({"id":"bob"}))?;
    assert!(matches!(
        node.host
            .call(
                "notes",
                Some(&alice),
                "greeting",
                br#"{"query":"hi"}"#.to_vec(),
                false
            )
            .await?
            .1,
        Output::Done(_)
    ));
    node.host
        .call(
            "notes",
            Some(&alice),
            "remember",
            br#"{"message":"kept"}"#.to_vec(),
            false,
        )
        .await?;
    let result = node
        .host
        .call("notes", Some(&bob), "recall", b"{}".to_vec(), false)
        .await?
        .1;
    assert!(matches!(result, Output::Done(value) if value == r#""kept""#));
    drop(node);
    let node = Node::new(
        state.clone(),
        state.join("secrets.toml"),
        4,
        Duration::from_secs(1),
        Duration::from_secs(30),
        None,
    )
    .await?;
    time::timeout(Duration::from_secs(5), async {
        while node.get("notes").is_none() {
            time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    let result = node
        .host
        .call("notes", Some(&bob), "recall", b"{}".to_vec(), false)
        .await?
        .1;
    assert!(matches!(result, Output::Done(value) if value == r#""kept""#));
    Ok(())
}

#[test]
fn deploy_help_distinguishes_browser_approval_from_terminal_confirmation() {
    for args in [vec!["--help"], vec!["deploy", "--help"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_crab"))
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success());
        let help = String::from_utf8(output.stdout)
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(help.contains("deployments:write"), "{help}");
        assert!(help.contains("browser approval"), "{help}");
    }
}
