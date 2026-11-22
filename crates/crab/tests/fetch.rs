//! A host reached only through a pattern is refused at an address that is not
//! public; a host declared by name is sent; a body over the request's limit is
//! answered cut there.

use anyhow::Result;
use berm::{
    Berm, Calls, Config, Engine, Output, Reach, abi,
    syscall::http::{self, Request, Response},
};
use berm_api::Image;
use berm_build::fixture;
use crab_agent::host::Fetch;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};
use tower::{Service, ServiceExt};

fn request(host: &str, reach: Reach) -> Request {
    Request {
        service: "fixture".to_owned(),
        chain: 1,
        span: 0,
        method: "GET".to_owned(),
        url: format!("http://{host}:9/"),
        host: host.trim_matches(['[', ']']).to_owned(),
        reach,
        headers: Vec::new(),
        body: Vec::new(),
        timeout: None,
        limit: None,
    }
}

async fn refusal(host: &str, reach: Reach) -> String {
    let mut fetch = Fetch::new().expect("a client builds");
    match fetch.call(request(host, reach)).await {
        Ok(response) => format!("answered {}", response.status),
        Err(error) => format!("{error:#}"),
    }
}

#[tokio::test]
async fn a_pattern_is_refused_private_addresses() -> Result<()> {
    for host in [
        "127.0.0.1",
        "10.0.0.1",
        "192.168.1.1",
        "169.254.169.254",
        "100.64.0.1",
        "0.0.0.0",
        "[::1]",
        "[fe80::1]",
        "[fd00::1]",
        "[::ffff:127.0.0.1]",
    ] {
        let found = refusal(host, Reach::Pattern).await;
        assert!(found.contains("is not a public address"), "{host}: {found}");
    }
    Ok(())
}

#[tokio::test]
async fn a_pattern_is_refused_a_name_resolving_only_to_loopback() -> Result<()> {
    let found = refusal("localhost", Reach::Pattern).await;
    assert!(found.contains("no public address"), "{found}");
    Ok(())
}

#[tokio::test]
async fn a_named_host_is_sent_wherever_it_points() -> Result<()> {
    let found = refusal("127.0.0.1", Reach::Named).await;
    assert!(!found.contains("public"), "{found}");
    Ok(())
}

#[tokio::test]
async fn a_body_over_the_limit_is_answered_cut() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    thread::spawn(move || -> Result<()> {
        let (mut stream, _) = listener.accept()?;
        let _ = stream.read(&mut [0; 1024])?;
        let body = vec![b'x'; 1 << 16];
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n",
            body.len()
        )?;
        stream.write_all(&body)?;
        Ok(())
    });
    let mut fetch = Fetch::new()?;
    let response = fetch
        .call(Request {
            url: format!("http://127.0.0.1:{port}/"),
            limit: Some(10),
            ..request("127.0.0.1", Reach::Named)
        })
        .await?;
    assert_eq!(response.body, b"xxxxxxxxxx");
    Ok(())
}

#[tokio::test]
async fn local_guest_receives_a_response_and_a_coded_address_refusal_in_one_batch() -> Result<()> {
    let original = fixture::image("berm-fixture")?;
    let section = abi::SERVICE_SECTION;
    let mut manifest: Value = serde_json::from_slice(Image(&original).section(section)?.unwrap())?;
    manifest["deps"]["http"]["fetch"] = json!(["*"]);
    let image = Image(&original).replace_section(section, &serde_json::to_vec(&manifest)?)?;
    let fetch = Fetch::new()?;
    let send = move |_, request: Request| {
        let fetch = fetch.clone();
        async move {
            if request.host == "8.8.8.8" {
                return Ok(Response {
                    status: 200,
                    headers: vec![],
                    body: b"public fixture".to_vec(),
                });
            }
            fetch.oneshot(request).await
        }
    };
    let secret = |_, _, _| async { Ok(None) };
    let engine = Engine::new(&Config::default())?;
    let runtime = Berm::new(
        &engine,
        Calls::by_name(Calls::DEFAULT_DEPTH),
        vec![
            http::fetch_all(send.clone(), secret),
            http::fetch(send, secret),
        ],
    );
    runtime.deploy("fixture", &image)?;
    for (urls, expected) in [
        (
            json!(["http://localhost:9/", "http://8.8.8.8/"]),
            json!([
                {"refused":{"explanation":"E0091"}},
                {"response":{"status":200,"body":"public fixture"}}
            ]),
        ),
        (
            json!(["http://localhost:9/"]),
            json!([{"refused":{"explanation":"E0091"}}]),
        ),
    ] {
        let call = runtime
            .call(
                None,
                "fixture",
                "http_reply_probe",
                serde_json::to_vec(&json!({"urls":urls}))?,
            )
            .await?;
        let Output::Done(output) = call.result else {
            anyhow::bail!("{:?}", call.result)
        };
        assert_eq!(serde_json::from_str::<Value>(&output)?, expected);
    }
    Ok(())
}
