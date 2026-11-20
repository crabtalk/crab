use anyhow::{Context, Result};
use berm::syscall::http::Response;
use berm_api::{
    Meter,
    host::Caller,
    storage::Identity,
    workflow::run::{Run, Status, Trigger},
};
use berm_host::{test::TestHost, workflow::StorageGrant};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::{Duration, Instant},
};

const SERVICE: &str = "fixture/seo";
const SITE: &str = "https://seo.example/";
const ACCOUNT: &str = "fixture-caller";

fn image() -> Result<&'static [u8]> {
    static IMAGE: OnceLock<Vec<u8>> = OnceLock::new();
    if let Some(image) = IMAGE.get() {
        return Ok(image);
    }
    let path = std::env::var_os("BERM_TEST_IMAGE_CRAB_SEO")
        .map(Ok)
        .unwrap_or_else(|| {
            berm_build::build(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."), None)
                .map(PathBuf::into_os_string)
        })?;
    Ok(IMAGE.get_or_init(|| fs::read(path).expect("built SEO image")))
}

async fn host() -> Result<TestHost> {
    let host = TestHost::open(100).await?;
    let service = host.deploy(SERVICE, image()?).await?;
    assert_eq!(service.manifest.database_size, 0);
    assert!(service.manifest.caller_storage.is_some());
    Ok(host)
}

async fn run(host: &TestHost, account: &str, input: Value) -> Result<Run> {
    let run = admit(host, account, input).await?;
    host.host.execute_workflow(&run.id).await
}

async fn admit(host: &TestHost, account: &str, input: Value) -> Result<Run> {
    host.advance(1);
    let run = host.host.prepare_workflow(
        SERVICE,
        "audit",
        input,
        Trigger::Manual {
            caller: Caller {
                account: None,
                id: account.into(),
                key: None,
                app: None,
                user: None,
                client: None,
                agent: None,
            },
        },
    )?;
    host.host
        .workflows
        .insert(
            run.clone(),
            image()?.to_vec(),
            StorageGrant {
                scope: "fixture".into(),
                bytes: u64::MAX,
            },
        )
        .await?;
    Ok(run)
}

fn report(run: &Run) -> &Value {
    assert_eq!(
        run.status,
        Status::Succeeded,
        "{:?}. Nodes: {:?}",
        run.error,
        run.checkpoints
            .iter()
            .map(|(node, point)| (node, point.state.to_string()))
            .collect::<Vec<_>>(),
    );
    run.result.as_ref().expect("successful audit report")
}

fn sitemap(host: &TestHost, pages: impl IntoIterator<Item = String>) {
    let mut xml = String::from("<urlset>");
    for page in pages {
        xml.push_str(&format!(
            "<url><loc>{page}</loc><lastmod>2020-01-01</lastmod></url>"
        ));
    }
    xml.push_str("</urlset>");
    host.arrange("GET", &format!("{SITE}sitemap.xml"), 200, xml);
}

fn respond(host: &TestHost, url: &str, status: u16, html: String, delay: Duration) {
    host.fetch.respond(
        "GET",
        url,
        Response {
            status,
            body: html.into_bytes(),
            headers: vec![
                ("content-type".into(), "text/html; charset=utf-8".into()),
                ("etag".into(), "\"fixture\"".into()),
                (
                    "last-modified".into(),
                    "Wed, 18 Nov 2026 00:00:00 GMT".into(),
                ),
            ],
        },
        delay,
    );
}

fn html(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=en><head><title>{title}</title><meta name=description content='{title}'></head><body><h1>{title}</h1>{body}</body></html>"
    )
}

fn database(host: &TestHost, account: &str) -> Result<PathBuf> {
    let service = host.host.get(SERVICE).context("SEO service")?;
    let storage = service.storage_id.get().context("caller storage ID")?;
    let identity = Identity {
        account: account.into(),
        user: None,
    };
    let file = format!("{}.sqlite", identity.digest());
    let prefix = format!("berm-test-{}-", std::process::id());
    for entry in fs::read_dir(std::env::temp_dir())? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            let path = entry
                .path()
                .join("databases/callers")
                .join(storage)
                .join(&file);
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    anyhow::bail!("fixture caller database missing")
}

fn paid_urls(run: &Run) -> Vec<&str> {
    let Some(berm_api::workflow::run::Prepared::Calls { calls }) =
        &run.checkpoints["audit"].prepared
    else {
        return Vec::new();
    };
    calls
        .iter()
        .map(|call| call.input["url"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn incremental_reports_and_priority() -> Result<()> {
    let host = host().await?;
    let a = format!("{SITE}a");
    let b = format!("{SITE}b");
    let c = format!("{SITE}c");
    let blocked = format!("{SITE}blocked");
    let redirect = format!("{SITE}redirect");
    let dead = format!("{SITE}dead");
    let missing = format!("{SITE}missing");
    let robots = format!("{SITE}robots.txt");
    let listings = [
        a.clone(),
        b.clone(),
        c.clone(),
        blocked.clone(),
        redirect.clone(),
        dead.clone(),
    ];
    sitemap(&host, listings.clone());
    host.arrange(
        "GET",
        &robots,
        200,
        "User-agent: CrabTalkSEO\nDisallow: /blocked",
    );
    let a_body = "<a href='/b'>b</a><a href='/dead'>dead</a><a href='/missing'>missing</a><img alt='before'>";
    let b_body = "<a href='/c'>c</a><a href='/a'>a</a>";
    respond(&host, &a, 200, html("Duplicate", a_body), Duration::ZERO);
    respond(&host, &b, 200, html("Duplicate", b_body), Duration::ZERO);
    respond(
        &host,
        &c,
        200,
        html("C", "<a href='/b'>b</a>"),
        Duration::ZERO,
    );
    respond(&host, &redirect, 301, String::new(), Duration::ZERO);
    respond(&host, &dead, 404, String::new(), Duration::ZERO);
    respond(&host, &missing, 200, html("Missing", ""), Duration::ZERO);

    let first = run(
        &host,
        ACCOUNT,
        json!({"site": SITE, "pages": [b], "max": 1}),
    )
    .await?;
    let baseline = report(&first);
    assert_eq!(paid_urls(&first), [&b]);
    assert_eq!(baseline["listed"], 6);
    assert_eq!(baseline["checked"], 6);
    assert_eq!(baseline["audited"], 1);
    assert_eq!(baseline["pending"], 2);
    assert_eq!(baseline["new"], json!([]));
    assert_eq!(baseline["fixed"], json!([]));
    assert!(host.fetched().iter().all(|request| request.url != blocked));
    for kind in ["sitemap_disallows", "page_redirect", "page_unavailable"] {
        assert!(
            baseline["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| finding["kind"] == kind)
        );
    }

    for url in [&a, &b, &c] {
        respond(&host, url, 304, String::new(), Duration::ZERO);
    }
    let second = run(&host, ACCOUNT, json!({"site": SITE})).await?;
    let filled = report(&second);
    assert_eq!(paid_urls(&second), [&a, &c]);
    assert_eq!(filled["audited"], 2);
    assert_eq!(filled["pending"], 0);
    for kind in [
        "duplicate_title",
        "duplicate_description",
        "broken_link",
        "missing_from_sitemap",
    ] {
        assert!(
            filled["new"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| finding["kind"] == kind)
        );
    }
    assert!(
        filled["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| {
                finding["kind"] == "duplicate_title"
                    && finding["page"] == a
                    && finding["stale"] == true
            })
    );
    let fetched = host.fetched();
    assert!(fetched.iter().any(|request| {
        request.url == b
            && request
                .headers
                .iter()
                .any(|(name, value)| name == "If-None-Match" && value == "\"fixture\"")
            && request
                .headers
                .iter()
                .any(|(name, _)| name == "If-Modified-Since")
    }));

    respond(
        &host,
        &a,
        200,
        html("Duplicate", &format!("{a_body}<p>New build</p>")),
        Duration::ZERO,
    );
    let unchanged = run(&host, ACCOUNT, json!({"site": SITE})).await?;
    assert_eq!(report(&unchanged)["audited"], 0);
    assert!(paid_urls(&unchanged).is_empty());

    respond(
        &host,
        &a,
        200,
        html("A unique title", "<img alt='after'>"),
        Duration::ZERO,
    );
    respond(
        &host,
        &b,
        200,
        html("Duplicate", &format!("{b_body}<img alt='changed'>")),
        Duration::ZERO,
    );
    respond(
        &host,
        &c,
        200,
        html("C", "<a href='/b'>b</a><img alt='changed'>"),
        Duration::ZERO,
    );
    respond(&host, &dead, 200, html("Now reachable", ""), Duration::ZERO);
    let capped = run(&host, ACCOUNT, json!({"site": SITE, "max": 1})).await?;
    assert_eq!(report(&capped)["pending"], 3);
    assert_eq!(paid_urls(&capped), [&dead]);
    let prioritized = run(&host, ACCOUNT, json!({"site": SITE, "max": 1})).await?;
    assert_eq!(report(&prioritized)["audited"], 1);
    assert_eq!(paid_urls(&prioritized), [&b]);
    let fixed = run(&host, ACCOUNT, json!({"site": SITE})).await?;
    let fixed = report(&fixed);
    for kind in [
        "duplicate_title",
        "duplicate_description",
        "broken_link",
        "missing_from_sitemap",
    ] {
        assert!(
            fixed["fixed"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| finding["kind"] == kind)
        );
    }

    host.arrange("GET", &robots, 503, "");
    sitemap(&host, [a.clone()]);
    let incomplete = run(&host, ACCOUNT, json!({"site": SITE})).await?;
    let incomplete = report(&incomplete);
    assert_eq!(incomplete["sitemap_complete"], false);
    assert_eq!(incomplete["robots_read"], false);
    assert_eq!(incomplete["audited"], 0);
    assert!(
        incomplete["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["page"] == c)
    );
    host.arrange(
        "GET",
        &robots,
        200,
        "User-agent: CrabTalkSEO\nDisallow: /blocked",
    );

    sitemap(&host, listings.into_iter().filter(|url| url != &c));
    let removed = run(&host, ACCOUNT, json!({"site": SITE, "max": 0})).await?;
    assert_eq!(report(&removed)["listed"], 5);
    assert!(paid_urls(&removed).is_empty());
    let conn =
        Connection::open_with_flags(database(&host, ACCOUNT)?, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let rows: i64 = conn.query_row("SELECT count(*) FROM pages WHERE url = ?1", [&c], |row| {
        row.get(0)
    })?;
    assert_eq!(rows, 0);
    let active: i64 = conn.query_row("SELECT count(*) FROM runs", [], |row| row.get(0))?;
    assert_eq!(active, 0);

    let another = run(&host, "another-caller", json!({"site": SITE, "max": 1})).await?;
    assert_eq!(report(&another)["new"], json!([]));
    assert_eq!(paid_urls(&another).len(), 1);
    Ok(())
}

#[tokio::test]
async fn one_live_run_per_caller_and_site() -> Result<()> {
    let host = host().await?;
    sitemap(&host, []);
    let release = host.fetch.respond_blocked(
        "GET",
        &format!("{SITE}robots.txt"),
        Response {
            status: 404,
            headers: vec![],
            body: vec![],
        },
    );
    let first = run(&host, ACCOUNT, json!({"site": SITE}));
    let second = async {
        for _ in 0..3000 {
            if !host.fetched().is_empty() {
                let result = run(&host, ACCOUNT, json!({"site": SITE})).await;
                release.notify_one();
                return result;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        anyhow::bail!("first run did not reach discovery")
    };
    let (first, second) = tokio::join!(first, second);
    assert_eq!(report(&first?)["listed"], 0);
    let second = second?;
    assert_eq!(second.status, Status::Failed);
    assert!(format!("{:?}", second.error).contains("An audit of this site is already running."));
    Ok(())
}

#[tokio::test]
async fn retry_save_returns_the_original_comparison() -> Result<()> {
    let host = host().await?;
    let url = format!("{SITE}page");
    sitemap(&host, [url.clone()]);
    host.arrange("GET", &format!("{SITE}robots.txt"), 404, "");
    respond(&host, &url, 200, html("Original", ""), Duration::ZERO);
    let first = admit(&host, ACCOUNT, json!({"site": SITE})).await?;
    host.host.workflows.store.run(|conn| conn.execute_batch(
        "CREATE TRIGGER lose_seo_save BEFORE UPDATE ON runs
         WHEN json_extract(CAST(NEW.body AS TEXT), '$.checkpoints.save.items[0].state.state') = 'succeeded'
         BEGIN SELECT RAISE(ABORT, 'save checkpoint unavailable'); END;"
    )).await?;
    assert!(host.host.execute_workflow(&first.id).await.is_err());
    host.host
        .workflows
        .store
        .run(|conn| conn.execute_batch("DROP TRIGGER lose_seo_save"))
        .await?;

    let path = database(&host, ACCOUNT)?;
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let original: String = conn.query_row(
        "SELECT report FROM reports WHERE run = ?1",
        [&first.id],
        |row| row.get(0),
    )?;
    let original: Value = serde_json::from_str(&original)?;

    host.advance(60_001);
    host.host.workflows.recover(host.now()).await?;
    respond(&host, &url, 200, html("Changed", ""), Duration::ZERO);
    let second = run(&host, ACCOUNT, json!({"site": SITE, "max": 0})).await?;
    assert_ne!(report(&second), &original);

    let resumed = host.host.continue_workflow(&first.id).await?;
    assert_eq!(report(&resumed), &original);
    let latest: String = conn.query_row("SELECT run FROM reports", [], |row| row.get(0))?;
    assert_eq!(latest, second.id);
    Ok(())
}

#[tokio::test]
async fn thousand_page_fixture() -> Result<()> {
    let host = host().await?;
    let urls: Vec<_> = (0..1_000)
        .map(|index| format!("{SITE}page-{index:04}"))
        .collect();
    sitemap(&host, urls.iter().cloned());
    host.arrange("GET", &format!("{SITE}robots.txt"), 404, "");
    for (index, url) in urls.iter().enumerate() {
        let links: String = (1..=24)
            .map(|offset| format!("<a href='{}'>Next</a>", urls[(index + offset) % urls.len()]))
            .collect();
        respond(
            &host,
            url,
            200,
            html(&format!("Guide {index}"), &links),
            Duration::ZERO,
        );
    }
    let first = run(&host, ACCOUNT, json!({"site": SITE})).await?;
    assert_eq!(report(&first)["checked"], 1_000);
    assert_eq!(report(&first)["audited"], 100);
    for url in &urls {
        respond(&host, url, 304, String::new(), Duration::ZERO);
    }
    for pending in (0..9).rev().map(|n| n * 100) {
        let completed = run(&host, ACCOUNT, json!({"site": SITE})).await?;
        assert_eq!(report(&completed)["pending"], pending);
    }

    let path = database(&host, ACCOUNT)?;
    let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let (count, scans): (i64, i64) = conn.query_row(
        "SELECT count(scan), sum(length(CAST(scan AS BLOB))) FROM pages",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(count, 1_000);
    let pages_bytes: i64 = conn.query_row(
        "SELECT sum(pgsize) FROM dbstat WHERE name = 'pages'",
        [],
        |row| row.get(0),
    )?;
    let pages: i64 = conn.query_row("PRAGMA page_count", [], |row| row.get(0))?;
    let page_size: i64 = conn.query_row("PRAGMA page_size", [], |row| row.get(0))?;
    println!(
        "storage scans_bytes={scans} pages_bytes={pages_bytes} sqlite_bytes={}",
        pages * page_size
    );

    for url in &urls {
        respond(&host, url, 304, String::new(), Duration::from_millis(100));
    }
    let begin = Instant::now();
    let conditional = run(&host, ACCOUNT, json!({"site": SITE})).await?;
    let elapsed = begin.elapsed();
    assert_eq!(report(&conditional)["checked"], 1_000);
    assert_eq!(report(&conditional)["audited"], 0);
    let check = &conditional.checkpoints["check"];
    let mut total = Meter::default();
    let mut peak = Meter::default();
    for item in &check.items {
        let meter = &item.dispatches[0].meter;
        total += *meter;
        peak.cpu = peak.cpu.max(meter.cpu);
        peak.read = peak.read.max(meter.read);
        peak.written = peak.written.max(meter.written);
        peak.memory = peak.memory.max(meter.memory);
    }
    println!(
        "conditional count=1000 batches={} workflow_ms={} cpu_total_us={} cpu_peak_us={} peak_read={} peak_written={} peak_memory_bytes={}",
        check.items.len(),
        elapsed.as_millis(),
        total.cpu,
        peak.cpu,
        peak.read,
        peak.written,
        peak.memory
    );
    assert!(elapsed < Duration::from_secs(60));

    let extra = format!("{SITE}zz-last");
    sitemap(&host, urls.iter().cloned().chain([extra.clone()]));
    respond(&host, &extra, 200, html("Extra", ""), Duration::ZERO);
    let rotated = run(&host, ACCOUNT, json!({"site": SITE, "max": 101})).await?;
    assert_eq!(report(&rotated)["listed"], 1_001);
    assert_eq!(report(&rotated)["checked"], 1_000);
    assert_eq!(paid_urls(&rotated), [&extra]);
    Ok(())
}
