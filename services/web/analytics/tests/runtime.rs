use anyhow::{Result, bail};
use berm::Client;
use berm_api::{Output, host::Caller, storage::Identity};
use berm_build::fixture;
use berm_host::test::{self, TestHost};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::sync::OnceLock;

const SERVICE: &str = "publisher/analytics";
const PROOF: &str = "https://example.com/.well-known/crabtalk-analytics";
const STRIPE: &str = "https://api.stripe.com/v1";
const YEAR: u64 = 365 * 24 * 60 * 60;
const KEY: &str = "secret_11111111111111111111111111111111";
const PUBLISHER_KEY: &str = "secret_22222222222222222222222222222222";

struct Analytics {
    host: TestHost,
    storage_id: String,
}

impl Analytics {
    async fn open() -> Result<Self> {
        static IMAGE: OnceLock<Vec<u8>> = OnceLock::new();
        let image = match IMAGE.get() {
            Some(image) => image,
            None => {
                let image = if std::env::var_os(fixture::variable("crab-analytics")).is_some() {
                    fixture::image("crab-analytics")?
                } else {
                    test::build("crab-analytics")?
                };
                IMAGE.get_or_init(|| image)
            }
        };
        let host = TestHost::open(1_000_000).await?;
        let service = host.deploy(SERVICE, image).await?;
        let storage_id = service
            .storage_id
            .get()
            .cloned()
            .expect("caller storage ID");
        Ok(Self { host, storage_id })
    }

    async fn call(&self, tool: &str, input: Value) -> Result<Value> {
        self.as_account("alice", None, tool, input).await
    }

    async fn as_account(
        &self,
        member: &str,
        account: Option<&str>,
        tool: &str,
        input: Value,
    ) -> Result<Value> {
        let caller = Caller {
            id: member.into(),
            account: account.map(Into::into),
            key: account.map(|_| "backend-key".into()),
            app: None,
            user: None,
            client: None,
            agent: None,
        };
        self.invoke(&caller, tool, input).await
    }

    async fn invoke(&self, caller: &Caller, tool: &str, input: Value) -> Result<Value> {
        let (_, result) = self
            .host
            .host
            .call(
                SERVICE,
                Some(caller),
                tool,
                serde_json::to_vec(&input)?,
                false,
            )
            .await?;
        output(result)
    }

    async fn app(&self, domain: &str, input: Value) -> Result<Value> {
        let mut input = input;
        input["domain"] = json!(domain);
        let caller = Caller {
            id: "alice".into(),
            account: None,
            key: None,
            app: Some("site-app".into()),
            user: None,
            client: Some("203.0.113.8".into()),
            agent: Some("Test browser".into()),
        };
        self.invoke(&caller, "track", input).await
    }

    async fn claim(&self, account: &str, domain: &str) -> Result<Value> {
        let args = json!({"domain":domain});
        let pending = self
            .as_account(account, None, "claim", args.clone())
            .await?;
        self.host.arrange(
            "GET",
            pending["url"].as_str().unwrap(),
            200,
            pending["token"].as_str().unwrap().as_bytes(),
        );
        self.as_account(account, None, "claim", args).await
    }

    fn shared(&self) -> Result<Connection> {
        Ok(Connection::open(self.host.host.databases.path(SERVICE))?)
    }

    fn caller_file(&self, account: &str) -> Result<Connection> {
        let identity = Identity {
            account: account.into(),
            user: None,
        };
        let name = format!("callers/{}/{}", self.storage_id, identity.digest());
        Ok(Connection::open(self.host.host.databases.path(&name))?)
    }

    async fn connect_stripe(&self) -> Result<()> {
        self.host
            .grant_resource(SERVICE, KEY, "api.stripe.com", "alice", "rk_fixture");
        self.call("connect_stripe", json!({"secret_id":KEY}))
            .await?;
        Ok(())
    }

    fn stripe(
        &self,
        endpoint: &str,
        status: Option<&str>,
        window: (u64, u64),
        after: Option<&str>,
        rows: Value,
        more: bool,
    ) {
        self.host.arrange(
            "GET",
            &stripe_url(endpoint, status, window, after),
            200,
            json!({"data": rows, "has_more": more})
                .to_string()
                .into_bytes(),
        );
    }

    fn payments(&self, window: (u64, u64), rows: &[Value]) {
        let mut after = None;
        for (index, page) in rows.chunks(100).enumerate() {
            let more = (index + 1) * 100 < rows.len();
            self.stripe("payment_intents", None, window, after, json!(page), more);
            after = page.last().and_then(|record| record["id"].as_str());
        }
    }

    fn without_sessions_or_invoices(&self, window: (u64, u64)) {
        self.stripe(
            "checkout/sessions",
            Some("complete"),
            window,
            None,
            json!([]),
            false,
        );
        self.stripe("invoices", Some("paid"), window, None, json!([]), false);
    }
}

fn stripe_url(
    endpoint: &str,
    status: Option<&str>,
    (cursor, through): (u64, u64),
    after: Option<&str>,
) -> String {
    let filter = status.map_or_else(String::new, |s| format!("status={s}&"));
    let after = after.map_or_else(String::new, |id| format!("&starting_after={id}"));
    format!(
        "{STRIPE}/{endpoint}?{filter}created%5Bgt%5D={cursor}&created%5Blte%5D={through}&limit=100{after}"
    )
}

fn payment(id: u64, created: u64) -> Value {
    json!({"id":format!("pi_{id}"), "created":created, "status":"succeeded", "amount_received":id, "currency":"usd"})
}

fn output(output: Output) -> Result<Value> {
    match output {
        Output::Done(value) if value.is_empty() => Ok(Value::Null),
        Output::Done(value) => Ok(serde_json::from_str(&value)?),
        Output::Failed(error) => bail!("{error}"),
    }
}

#[tokio::test]
async fn claim_verifies_a_public_host_and_requires_fresh_proof_for_transfer() -> Result<()> {
    let analytics = Analytics::open().await?;
    let pending = analytics
        .call("claim", json!({"domain":"HTTPS://Example.COM:443/path"}))
        .await?;
    let token = pending["token"].as_str().unwrap();
    assert_eq!(
        pending,
        json!({"domain":"example.com","url":PROOF,"token":token,"verified":false})
    );
    assert_eq!(token.len(), 32);
    assert!(token.bytes().all(|byte| byte.is_ascii_hexdigit()));
    analytics
        .host
        .arrange("GET", PROOF, 200, b"wrong".as_slice());
    assert_eq!(
        analytics
            .call("claim", json!({"domain":"example.com"}))
            .await?,
        pending
    );
    analytics
        .host
        .arrange("GET", PROOF, 200, format!("\n {token} \n"));
    let verified = analytics
        .call("claim", json!({"domain":"example.com"}))
        .await?;
    assert_eq!(verified["verified"], true);
    assert_eq!(analytics.host.fetched().last().unwrap().url, PROOF);
    let args = json!({"domain":"example.com"});
    let refusal = analytics
        .as_account("bob", None, "claim", args.clone())
        .await
        .unwrap_err()
        .to_string();
    let challenge = refusal
        .split_once("Publish ")
        .unwrap()
        .1
        .split_whitespace()
        .next()
        .unwrap();
    assert!(refusal.contains("This domain is claimed by another account"));
    assert_ne!(challenge, token);
    analytics
        .app(
            "example.com",
            json!({"visitor":"old","session":"s","path":"/private"}),
        )
        .await?;
    analytics
        .host
        .arrange("GET", PROOF, 200, challenge.as_bytes());
    assert_eq!(
        analytics
            .as_account("bob", None, "claim", args.clone())
            .await?["verified"],
        true
    );
    let buffer = analytics
        .host
        .sql(SERVICE, "SELECT count(*) FROM buffer", json!([]))
        .await?;
    assert_eq!(buffer["rows"], json!([[0]]));
    assert!(
        analytics
            .call("claim", args)
            .await
            .unwrap_err()
            .to_string()
            .contains("claimed by another account")
    );

    let pending = analytics
        .call("claim", json!({"domain":"other.example"}))
        .await?;
    let replacement = analytics
        .as_account("bob", None, "claim", json!({"domain":"other.example"}))
        .await?;
    assert_eq!(replacement["verified"], false);
    assert_ne!(pending["token"], replacement["token"]);
    Ok(())
}

#[tokio::test]
async fn www_visits_and_events_share_the_bare_domain_claim() -> Result<()> {
    let analytics = Analytics::open().await?;
    let claim = analytics.claim("alice", "www.example.com").await?;
    assert_eq!(claim["domain"], "example.com");
    assert_eq!(claim["url"], PROOF);
    assert_eq!(claim["verified"], true);
    assert_eq!(
        analytics
            .call("claim", json!({"domain":"example.com"}))
            .await?,
        claim
    );

    analytics
        .app(
            "www.example.com",
            json!({"visitor":"v","session":"s","path":"/"}),
        )
        .await?;
    analytics
        .call(
            "event",
            json!({
                "domain":"www.example.com","visitor":"v","session":"s","name":"signup"
            }),
        )
        .await?;
    let funnel = analytics
        .call("funnel", json!({"steps":["/","signup"],"since":0}))
        .await?;
    assert_eq!(funnel["steps"][1]["visitors"], 1);
    let domains: String = analytics.caller_file("alice")?.query_row(
        "SELECT group_concat(DISTINCT domain) FROM hits",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(domains, "example.com");
    Ok(())
}

#[tokio::test]
async fn an_anonymous_app_gets_no_client_key_but_track_uses_one_when_supplied() -> Result<()> {
    let analytics = Analytics::open().await?;
    let input = json!({"session":"s","path":"/"});
    let refusal = analytics
        .app("example.com", input.clone())
        .await
        .unwrap_err()
        .to_string();
    assert!(refusal.contains("Verify this domain"));
    analytics.claim("alice", "example.com").await?;
    let missing = analytics
        .app("example.com", input)
        .await
        .unwrap_err()
        .to_string();
    assert!(missing.contains("No visitor ID is available"));
    let buffer = analytics
        .host
        .sql(SERVICE, "SELECT count(*) FROM buffer", json!([]))
        .await?;
    assert_eq!(buffer["rows"], json!([[0]]));

    let args = serde_json::to_vec(&json!({"domain":"example.com","session":"s","path":"/"}))?;
    let ran = analytics
        .host
        .call_with_client(
            SERVICE,
            "track",
            &args,
            Client {
                key: Some("daily-key".into()),
                ip: None,
            },
        )
        .await?;
    assert_eq!(output(ran.result)?, json!({"visitor":"daily-key"}));
    assert_eq!(
        analytics
            .app(
                "example.com",
                json!({"visitor":"stored","session":"s","path":"/"})
            )
            .await?,
        json!({"visitor":"stored"})
    );
    Ok(())
}

#[tokio::test]
async fn track_validates_fields_and_caps_each_domain_at_ten_thousand_rows() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.claim("alice", "example.com").await?;
    analytics.claim("bob", "other.example").await?;
    let input = json!({"visitor":"v","session":"s","path":"/"});
    let bad = [
        ("path", json!("/".repeat(2049))),
        ("referrer", json!("é".repeat(1025))),
        ("utm", json!({"campaign":"x".repeat(257)})),
        ("event", json!("x".repeat(65))),
        ("event", json!("Sign Up")),
        ("value", json!(-1)),
        ("currency", json!("us")),
        ("visitor", json!("")),
    ];
    for (field, invalid) in bad {
        let mut args = input.clone();
        args[field] = invalid;
        assert!(analytics.app("example.com", args).await.is_err(), "{field}");
    }
    let max = json!({
        "visitor":"v","session":"s","path":"/".repeat(2048), "referrer":"x".repeat(2048),
        "utm":{"source":"x".repeat(256),"medium":"y".repeat(256),"campaign":"z".repeat(256)},
        "event":"x".repeat(64),"value":0,"currency":"USD"
    });
    analytics.app("example.com", max).await?;
    analytics.app("other.example", input.clone()).await?;
    analytics.shared()?.execute_batch(
        "WITH RECURSIVE x(n) AS (VALUES(1) UNION ALL SELECT n + 1 FROM x WHERE n < 9999)
         INSERT INTO buffer (domain,visitor,session,kind,path,at)
         SELECT 'example.com','v','s','view','/seed',1000 FROM x;",
    )?;
    analytics.app("example.com", input).await?;
    let result = analytics
        .host
        .sql(
            SERVICE,
            "SELECT domain, count(*), min(id), max(id) FROM buffer GROUP BY domain ORDER BY domain",
            json!([]),
        )
        .await?;
    assert_eq!(
        result["rows"],
        json!([["example.com", 10000, 3, 10002], ["other.example", 1, 2, 2]])
    );
    Ok(())
}

#[tokio::test]
async fn drain_retries_after_caller_commit_without_duplicates_and_obeys_the_limit() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.claim("alice", "example.com").await?;
    analytics
        .app(
            "example.com",
            json!({"visitor":"v","session":"s","path":"/pricing"}),
        )
        .await?;
    // Recreate the durable state left if the caller commit succeeds and the shared commit fails.
    analytics.caller_file("alice")?.execute_batch(
        "INSERT INTO hits (buffer_id,domain,visitor,session,kind,path,channel,at)
         VALUES (1,'example.com','v','s','view','/pricing','direct',1000);",
    )?;
    analytics.shared()?.execute_batch(
        "WITH RECURSIVE x(n) AS (VALUES(1) UNION ALL SELECT n + 1 FROM x WHERE n < 5000)
         INSERT INTO buffer (domain,visitor,session,kind,path,at)
         SELECT 'example.com','v','s','view','/seed',1000 FROM x;",
    )?;
    let report = json!({"steps":["/pricing","/seed"],"since":0});
    analytics.call("funnel", report.clone()).await?;
    let count: i64 =
        analytics
            .caller_file("alice")?
            .query_row("SELECT count(*) FROM hits", [], |row| row.get(0))?;
    assert_eq!(count, 5000);
    let buffer = analytics
        .host
        .sql(SERVICE, "SELECT id FROM buffer", json!([]))
        .await?;
    assert_eq!(buffer["rows"], json!([[5001]]));
    analytics.call("funnel", report.clone()).await?;
    analytics.call("funnel", report).await?;
    let count: i64 =
        analytics
            .caller_file("alice")?
            .query_row("SELECT count(*) FROM hits", [], |row| row.get(0))?;
    assert_eq!(count, 5001);
    let buffer = analytics
        .host
        .sql(SERVICE, "SELECT count(*) FROM buffer", json!([]))
        .await?;
    assert_eq!(buffer["rows"], json!([[0]]));
    Ok(())
}

#[tokio::test]
async fn funnel_matches_ordered_paths_prefixes_and_events_within_since() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.claim("alice", "example.com").await?;
    for (index, (visitor, path, event)) in [
        ("a", "/landing", None),
        ("a", "/docs/install", None),
        ("a", "/docs/install", Some("signup")),
        ("b", "/docs/install", None),
        ("b", "/landing", None),
        ("b", "/docs", Some("signup")),
        ("c", "/landing?next=1", None),
        ("c", "/docs/guides", None),
        ("c", "/docs", Some("signup")),
        ("d", "/landing", None),
        ("d", "/docs/", None),
    ]
    .into_iter()
    .enumerate()
    {
        analytics.host.set_now(1_001_000 + index as u64 * 1000);
        analytics
            .app(
                "example.com",
                json!({"visitor":visitor,"session":"s","path":path,"event":event}),
            )
            .await?;
    }
    let args = json!({"steps":["/landing","/docs/*","signup"],"since":1001});
    let report = analytics.call("funnel", args).await?;
    assert_eq!(
        report["steps"],
        json!([
            {"step":"/landing","visitors":3},
            {"step":"/docs/*","visitors":2,"conversion":2.0/3.0},
            {"step":"signup","visitors":1,"conversion":0.5}
        ])
    );
    let report = analytics
        .call(
            "funnel",
            json!({"steps":["/landing","/docs/*","signup"],"since":1002}),
        )
        .await?;
    assert_eq!(report["steps"][2]["visitors"], 0);
    assert!(
        analytics
            .call("funnel", json!({"steps":["signup"],"since":0}))
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn channels_report_first_and_last_touch_self_referrals_and_separate_currencies() -> Result<()>
{
    let analytics = Analytics::open().await?;
    analytics.claim("alice", "example.com").await?;
    for (at, visitor, referrer, source) in [
        (1001, "a", "https://www.search.example/query", None),
        (1002, "b", "https://www.example.com/path", None),
        (
            1003,
            "a",
            "https://www.example.com/path",
            Some("newsletter"),
        ),
        (1005, "a", "https://www.ads.example/path", None),
    ] {
        analytics.host.set_now(at * 1000);
        analytics.app("example.com", json!({
            "visitor":visitor,"session":"s","path":"/","referrer":referrer,"utm":{"source":source}
        })).await?;
    }
    analytics
        .call(
            "event",
            json!({"domain":"example.com","visitor":"b","session":"s","name":"signup"}),
        )
        .await?;
    analytics.caller_file("alice")?.execute_batch(
        "INSERT INTO payments (id,customer,visitor,amount,currency,at) VALUES
         ('pi_1','cus_a','a',100,'usd',1004),
         ('pi_2','cus_a','a',200,'eur',1005),
         ('pi_3','cus_unknown',NULL,300,'usd',1005);",
    )?;
    let first = analytics
        .call("channels", json!({"since":0,"model":"first"}))
        .await?;
    assert_eq!(
        first["channels"],
        json!([
            {"channel":"direct","visitors":1,"paying_customers":0,"revenue":{}},
            {"channel":"search.example","visitors":1,"paying_customers":1,"revenue":{"eur":200,"usd":100}},
            {"channel":"unattributed","visitors":0,"paying_customers":1,"revenue":{"usd":300}}
        ])
    );
    let last = analytics
        .call("channels", json!({"since":1002,"model":"last"}))
        .await?;
    assert_eq!(
        last["channels"],
        json!([
            {"channel":"direct","visitors":1,"paying_customers":0,"revenue":{}},
            {"channel":"newsletter","visitors":1,"paying_customers":1,"revenue":{"eur":200,"usd":100}},
            {"channel":"unattributed","visitors":0,"paying_customers":1,"revenue":{"usd":300}}
        ])
    );
    Ok(())
}

#[tokio::test]
async fn stripe_resolves_a_cross_account_grant_pages_and_attributes_renewals_once() -> Result<()> {
    let analytics = Analytics::open().await?;
    let failure = analytics
        .call("sync_revenue", json!({}))
        .await
        .unwrap_err()
        .to_string();
    assert!(failure.contains("connect_stripe"));
    analytics
        .host
        .grant_resource(SERVICE, KEY, "api.stripe.com", "alice", "rk_fixture");
    analytics.host.grant_resource(
        SERVICE,
        PUBLISHER_KEY,
        "api.stripe.com",
        "publisher",
        "publisher_key",
    );
    assert!(
        analytics
            .as_account("bob", None, "connect_stripe", json!({"secret_id":KEY}))
            .await
            .is_err()
    );
    assert!(
        analytics
            .call("connect_stripe", json!({"secret_id":PUBLISHER_KEY}))
            .await
            .is_err()
    );
    assert!(
        analytics
            .as_account(
                "publisher",
                Some("other-account"),
                "connect_stripe",
                json!({"secret_id":PUBLISHER_KEY})
            )
            .await
            .is_err()
    );
    analytics
        .as_account(
            "publisher",
            None,
            "connect_stripe",
            json!({"secret_id":PUBLISHER_KEY}),
        )
        .await?;
    analytics
        .call("connect_stripe", json!({"secret_id":KEY}))
        .await?;
    analytics.claim("alice", "example.com").await?;
    analytics.host.set_now(101_000);
    analytics
        .app(
            "example.com",
            json!({
                "visitor":"v","session":"s","path":"/","utm":{"source":"search"}
            }),
        )
        .await?;
    analytics.host.set_now(1_000_000);
    analytics.stripe(
        "checkout/sessions",
        Some("complete"),
        (0, 999),
        None,
        json!([
            {"id":"cs_new","created":150,"customer":"cus_a","metadata":{"crabtalk_visitor_id":"v"}}
        ]),
        true,
    );
    analytics.stripe(
        "checkout/sessions",
        Some("complete"),
        (0, 999),
        Some("cs_new"),
        json!([
            {"id":"cs_old","created":149,"customer":null,"metadata":null}
        ]),
        false,
    );
    analytics.stripe("payment_intents", None, (0, 999), None, json!([
        {"id":"pi_renewal","created":300,"status":"succeeded","amount_received":500,"currency":"usd","customer":"cus_a"},
        {"id":"pi_pending","created":250,"status":"requires_payment_method","amount_received":0,"currency":"usd"},
        {"id":"pi_paid","created":200,"status":"succeeded","amount_received":100,"currency":"eur",
         "metadata":{"crabtalk_visitor_id":"v","crabtalk_session_id":"s"},"customer":"cus_a"}
    ]), false);
    analytics.stripe(
        "invoices",
        Some("paid"),
        (0, 999),
        None,
        json!([
            {"id":"in_cycle","created":299,"billing_reason":"subscription_cycle",
             "payment_intent":"pi_renewal","amount_paid":500,"currency":"usd",
             "status_transitions":{"paid_at":301},"customer":"cus_a"},
            {"id":"in_first","created":200,"billing_reason":"subscription_create",
             "payment_intent":"pi_paid","amount_paid":100,"currency":"eur",
             "status_transitions":{"paid_at":200},"customer":"cus_a"}
        ]),
        false,
    );
    let synced = analytics.call("sync_revenue", json!({})).await?;
    assert_eq!(
        synced,
        json!({"checkout_sessions":1,"payment_intents":2,"renewal_invoices":1,"synced_through":999,"has_more":false})
    );

    let report = analytics
        .call("channels", json!({"since":100,"model":"last"}))
        .await?;
    assert_eq!(
        report["channels"],
        json!([
            {"channel":"search","visitors":1,"paying_customers":1,"revenue":{"eur":100,"usd":500}}
        ])
    );
    assert_eq!(report["revenue_synced_through"], 999);
    assert_eq!(report["revenue_fully_synced"], true);
    let requests = analytics.host.fetched();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.starts_with(STRIPE))
            .count(),
        4
    );
    assert!(
        requests
            .iter()
            .filter(|request| request.url.starts_with(STRIPE))
            .all(|request| request
                .headers
                .iter()
                .any(|(name, value)| name.eq_ignore_ascii_case("authorization")
                    && value == "Bearer rk_fixture"))
    );
    let count: i64 =
        analytics
            .caller_file("alice")?
            .query_row("SELECT count(*) FROM payments", [], |row| row.get(0))?;
    assert_eq!(count, 2);
    let visitor: String = analytics.caller_file("alice")?.query_row(
        "SELECT visitor FROM payments WHERE id = 'pi_renewal'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(visitor, "v");
    Ok(())
}

#[tokio::test]
async fn stripe_sync_finishes_a_large_history_over_several_calls() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.connect_stripe().await?;
    const WINDOWS: u64 = 8;
    const PER_WINDOW: u64 = 131;
    analytics.host.set_now((WINDOWS * YEAR + 1) * 1000);
    for window in 0..WINDOWS {
        let window_start = window * YEAR;
        let bounds = (window_start, window_start + YEAR);
        analytics.without_sessions_or_invoices(bounds);
        let rows: Vec<_> = (1..=PER_WINDOW)
            .rev()
            .map(|offset| {
                payment(
                    window * PER_WINDOW + offset,
                    if offset == PER_WINDOW {
                        bounds.1
                    } else {
                        window_start + offset
                    },
                )
            })
            .collect();
        analytics.payments(bounds, &rows);
    }

    let mut imported = 0;
    for (windows, remaining) in [(3, true), (6, true), (8, false)] {
        let synced = analytics.call("sync_revenue", json!({})).await?;
        imported += synced["payment_intents"].as_u64().unwrap();
        assert_eq!(synced["synced_through"], windows * YEAR);
        assert_eq!(synced["has_more"], remaining);
        assert_eq!(imported, windows * PER_WINDOW);
    }
    let requests = analytics.host.fetched().len();
    let synced = analytics.call("sync_revenue", json!({})).await?;
    assert_eq!(synced["payment_intents"], 0);
    assert_eq!(synced["has_more"], false);
    assert_eq!(analytics.host.fetched().len(), requests);
    let db = analytics.caller_file("alice")?;
    let mut found = db.prepare("SELECT amount FROM payments ORDER BY amount")?;
    let amounts = found
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    assert_eq!(
        amounts,
        (1..=(WINDOWS * PER_WINDOW) as i64).collect::<Vec<_>>()
    );
    Ok(())
}

#[tokio::test]
async fn stripe_shrinks_dense_windows_and_remembers_the_split_after_the_call_cap() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.connect_stripe().await?;
    analytics.host.set_now(1_001_000);
    analytics.without_sessions_or_invoices((0, 1000));
    let mut rows: Vec<_> = (2..=301)
        .rev()
        .map(|id| {
            payment(
                id,
                if id > 151 {
                    750 + id - 151
                } else {
                    500 + id - 1
                },
            )
        })
        .collect();
    analytics.payments((500, 1000), &rows);
    analytics.payments((750, 1000), &rows[..150]);
    analytics.payments((500, 750), &rows[150..]);
    rows.push(payment(1, 500));
    analytics.payments((0, 1000), &rows);
    analytics.payments((0, 500), &rows[300..]);

    let first = analytics.call("sync_revenue", json!({})).await?;
    assert_eq!(first["synced_through"], 500);
    assert_eq!(first["has_more"], true);
    assert_eq!(first["payment_intents"], 1);
    let first_amount: i64 = analytics.caller_file("alice")?.query_row(
        "SELECT sum(amount) FROM payments",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(first_amount, 1);
    let second = analytics.call("sync_revenue", json!({})).await?;
    assert_eq!(second["synced_through"], 1000);
    assert_eq!(second["has_more"], false);
    assert_eq!(second["payment_intents"], 300);
    let total: (i64, i64) = analytics.caller_file("alice")?.query_row(
        "SELECT count(*), sum(amount) FROM payments",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(total, (301, 301 * 302 / 2));
    Ok(())
}

#[tokio::test]
async fn channels_reports_stored_revenue_when_a_later_stripe_page_fails() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.connect_stripe().await?;
    analytics.host.set_now((2 * YEAR + 1) * 1000);
    analytics.without_sessions_or_invoices((0, YEAR));
    analytics.without_sessions_or_invoices((YEAR, 2 * YEAR));
    analytics.stripe(
        "payment_intents",
        None,
        (0, YEAR),
        None,
        json!([payment(7, YEAR)]),
        false,
    );
    analytics.stripe(
        "payment_intents",
        None,
        (YEAR, 2 * YEAR),
        None,
        json!([payment(11, 2 * YEAR)]),
        true,
    );
    analytics.host.arrange(
        "GET",
        &stripe_url("payment_intents", None, (YEAR, 2 * YEAR), Some("pi_11")),
        503,
        b"try again later".as_slice(),
    );

    let report = analytics
        .call("channels", json!({"since":0,"model":"first"}))
        .await?;
    assert_eq!(report["revenue_synced_through"], YEAR);
    assert_eq!(report["revenue_fully_synced"], false);
    assert_eq!(
        report["channels"],
        json!([
            {"channel":"unattributed","visitors":0,"paying_customers":1,"revenue":{"usd":7}}
        ])
    );
    let failed = analytics.call("sync_revenue", json!({})).await?;
    assert_eq!(failed["synced_through"], YEAR);
    assert_eq!(failed["has_more"], true);
    assert_eq!(failed["payment_intents"], 0);
    assert!(failed["error"].as_str().unwrap().contains("503"));

    analytics.stripe(
        "payment_intents",
        None,
        (YEAR, 2 * YEAR),
        Some("pi_11"),
        json!([payment(5, YEAR + 1)]),
        false,
    );
    let repaired = analytics
        .call("channels", json!({"since":0,"model":"first"}))
        .await?;
    assert_eq!(repaired["revenue_synced_through"], 2 * YEAR);
    assert_eq!(repaired["revenue_fully_synced"], true);
    assert_eq!(repaired["channels"][0]["revenue"], json!({"usd":23}));
    Ok(())
}

#[tokio::test]
async fn two_accounts_with_the_same_key_creator_cannot_read_or_write_each_others_rows() -> Result<()>
{
    let analytics = Analytics::open().await?;
    analytics.claim("alice", "example.com").await?;
    let pending = analytics
        .as_account(
            "alice",
            Some("team"),
            "claim",
            json!({"domain":"team.example"}),
        )
        .await?;
    analytics.host.arrange(
        "GET",
        pending["url"].as_str().unwrap(),
        200,
        pending["token"].as_str().unwrap().as_bytes(),
    );
    assert_eq!(
        analytics
            .as_account(
                "teammate",
                Some("team"),
                "claim",
                json!({"domain":"team.example"})
            )
            .await?["verified"],
        true
    );
    analytics
        .app(
            "example.com",
            json!({"visitor":"v","session":"s","path":"/private"}),
        )
        .await?;
    analytics
        .app(
            "team.example",
            json!({"visitor":"v","session":"s","path":"/team","utm":{"source":"team-news"}}),
        )
        .await?;
    let event = json!({"domain":"example.com","visitor":"v","session":"s","name":"signup"});
    assert!(
        analytics
            .as_account("alice", Some("team"), "event", event.clone())
            .await
            .is_err()
    );
    analytics
        .as_account("alice", Some("alice"), "event", event)
        .await?;
    analytics
        .as_account(
            "teammate",
            Some("team"),
            "event",
            json!({
                "domain":"team.example","visitor":"v","session":"s","name":"signup"
            }),
        )
        .await?;
    let args = json!({"steps":["/private","signup"],"since":0});
    assert_eq!(
        analytics.call("funnel", args.clone()).await?["steps"][1]["visitors"],
        1
    );
    assert_eq!(
        analytics
            .as_account("alice", Some("team"), "funnel", args)
            .await?["steps"][1]["visitors"],
        0
    );
    assert_eq!(
        analytics
            .as_account(
                "alice",
                Some("team"),
                "funnel",
                json!({
                    "steps":["/team","signup"],"since":0
                })
            )
            .await?["steps"][1]["visitors"],
        1
    );
    let event: (String, Option<i64>) = analytics.caller_file("team")?.query_row(
        "SELECT channel,buffer_id FROM hits WHERE kind = 'event'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(event, ("team-news".into(), None));
    Ok(())
}
