//! A host reached only through a pattern is refused at an address that is not
//! public; a host declared by name is sent.

use anyhow::Result;
use berm::{Reach, syscall::http::Request};
use bermd::Fetch;
use tower::Service;

fn request(host: &str, reach: Reach) -> Request {
    Request {
        method: "GET".to_owned(),
        url: format!("http://{host}:9/"),
        host: host.trim_matches(['[', ']']).to_owned(),
        reach,
        headers: Vec::new(),
        body: Vec::new(),
        timeout: None,
    }
}

async fn refusal(host: &str, reach: Reach) -> String {
    let mut fetch = Fetch::new(None).expect("a client builds");
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
