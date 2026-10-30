//! Replaying built code keeps the tape boundary and never fetches the old image.
use anyhow::Result;
use berm_api::host::CallInput;
use crabtalk_sdk::client::Client;
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    process::{Command, ExitCode},
    thread,
};

fn source(tool: &str, image: Option<Vec<u8>>) -> Result<Client> {
    let input = serde_json::to_vec(&CallInput {
        tool: tool.to_owned(),
        args: r#"{"query":"fixed"}"#.to_owned(),
        caller: None,
        digest: format!("sha256:{}", "0".repeat(64)),
    })?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    thread::spawn(move || -> Result<()> {
        let mut bodies = vec![("input", input), ("tape", vec![1])];
        if let Some(image) = image {
            bodies.push(("image", image));
        }
        for (kind, body) in bodies {
            let (mut stream, _) = listener.accept()?;
            let mut request = [0; 4096];
            let count = stream.read(&mut request)?;
            assert!(
                String::from_utf8_lossy(&request[..count])
                    .starts_with(&format!("GET /services/local/fixture/calls/1/{kind} "))
            );
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                body.len()
            )?;
            stream.write_all(&body)?;
        }
        Ok(())
    });
    Ok(Client::new(format!("http://{address}")))
}

#[test]
fn built_replays_accept_a_new_image_but_keep_call_exit_codes() -> Result<()> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/fixture");
    for (tool, expected) in [
        ("echo", ExitCode::SUCCESS),
        ("boom", ExitCode::from(2)),
        ("random", ExitCode::from(2)),
    ] {
        let from = source(tool, None)?;
        assert_eq!(
            crab_agent::cmd::replay::tape(&from, "fixture", 1, Some(&dir))?,
            expected
        );
    }
    Ok(())
}

#[test]
fn ordinary_replay_still_checks_the_recorded_digest() -> Result<()> {
    let from = source("echo", Some(b"wrong image".to_vec()))?;
    let error = crab_agent::cmd::replay::tape(&from, "fixture", 1, None).unwrap_err();
    assert!(error.to_string().contains("another digest"));
    Ok(())
}

#[test]
fn build_and_live_replay_are_mutually_exclusive() -> Result<()> {
    let output = Command::new(env!("CARGO_BIN_EXE_crab"))
        .args(["replay", "fixture", "1", "--build", "--on", "default"])
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
    Ok(())
}
