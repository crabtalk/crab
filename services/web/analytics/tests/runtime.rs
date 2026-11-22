use anyhow::{Context, Result, bail};
use berm::{Client, Service};
use berm_api::{Output, host::Caller, storage::Identity};
use berm_build::fixture;
use berm_host::test::{self, TestHost};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::sync::{Arc, OnceLock};

const SERVICE: &str = "publisher/analytics";
const PROOF: &str = "https://example.com/.well-known/crabtalk-analytics";
const WWW_PROOF: &str = "https://www.example.com/.well-known/crabtalk-analytics";
const STRIPE: &str = "https://api.stripe.com/v1";
const DAY: u64 = 24 * 60 * 60;
const YEAR: u64 = 365 * DAY;
const KEY: &str = "secret_11111111111111111111111111111111";
const PUBLISHER_KEY: &str = "secret_22222222222222222222222222222222";

struct Analytics {
    host: TestHost,
    service: Arc<Service>,
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
        Ok(Self { host, service })
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
        let storage_id = self.service.storage_id.get().expect("caller storage ID");
        let name = format!("callers/{storage_id}/{}", identity.digest());
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
        .arrange("GET", PROOF, 301, b"Moved permanently".as_slice());
    analytics
        .host
        .arrange("GET", WWW_PROOF, 200, challenge.as_bytes());
    let transferred = analytics
        .as_account("bob", None, "claim", args.clone())
        .await?;
    assert_eq!(transferred["verified"], true);
    assert_eq!(transferred["domain"], "example.com");
    assert_eq!(transferred["url"], WWW_PROOF);
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
async fn claim_verifies_on_www_when_the_bare_url_redirects() -> Result<()> {
    let analytics = Analytics::open().await?;
    let args = json!({"domain":"example.com"});
    let pending = analytics.call("claim", args.clone()).await?;
    let token = pending["token"].as_str().unwrap();
    assert_eq!(pending["url"], PROOF);
    assert_eq!(pending["verified"], false);
    analytics
        .host
        .arrange("GET", PROOF, 301, b"Moved permanently".as_slice());
    analytics
        .host
        .arrange("GET", WWW_PROOF, 200, token.as_bytes());

    let verified = analytics.call("claim", args).await?;
    assert_eq!(
        verified,
        json!({"domain":"example.com","url":WWW_PROOF,"token":token,"verified":true})
    );
    let requests = analytics.host.fetched();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.url.as_str())
            .collect::<Vec<_>>(),
        [PROOF, WWW_PROOF]
    );
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
    analytics.host.set_now(1_001_000);
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
    analytics.host.set_now(1_012_000);
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
    analytics.host.set_now(1_006_000);
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
async fn stripe_sync_finishes_an_empty_account_in_one_call() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.connect_stripe().await?;
    let through = 56 * YEAR;
    analytics.host.set_now((through + 1) * 1000);
    let mut cursor = 0;
    for years in [1, 2, 4, 8, 16, 32] {
        let upper = (cursor + years * YEAR).min(through);
        let bounds = (cursor, upper);
        analytics.without_sessions_or_invoices(bounds);
        analytics.stripe("payment_intents", None, bounds, None, json!([]), false);
        cursor = upper;
    }

    let synced = analytics.call("sync_revenue", json!({})).await?;
    assert_eq!(
        synced,
        json!({
            "checkout_sessions":0,"payment_intents":0,"renewal_invoices":0,
            "synced_through":through,"has_more":false
        })
    );
    assert_eq!(analytics.host.fetched().len(), 18);
    Ok(())
}

#[tokio::test]
async fn stripe_sync_finishes_a_large_history_over_several_calls() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.connect_stripe().await?;
    const WINDOWS: u64 = 8;
    const PER_WINDOW: u64 = 131;
    analytics.host.set_now((WINDOWS * YEAR + 1) * 1000);
    for (lower, upper) in [(0, 1), (1, 3), (3, 7), (7, WINDOWS)] {
        analytics.without_sessions_or_invoices((lower * YEAR, upper * YEAR));
    }
    for window in 0..WINDOWS {
        let window_start = window * YEAR;
        let bounds = (window_start, window_start + YEAR);
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
async fn stripe_grows_windows_for_steady_sales_after_a_dense_period() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.connect_stripe().await?;
    let dense_through = 2 * DAY;
    analytics.host.set_now((dense_through + 1) * 1000);
    analytics.without_sessions_or_invoices((0, dense_through));
    let dense: Vec<_> = (1..=201)
        .rev()
        .map(|id| {
            payment(
                id,
                if id <= 100 {
                    DAY - 100 + id
                } else {
                    dense_through - 201 + id
                },
            )
        })
        .collect();
    analytics.payments((0, dense_through), &dense);
    analytics.payments((0, DAY), &dense[101..]);
    analytics.payments((DAY, dense_through), &dense[..101]);
    let synced = analytics.call("sync_revenue", json!({})).await?;
    assert_eq!(synced["payment_intents"], 201);
    assert_eq!(synced["synced_through"], dense_through);
    assert_eq!(synced["has_more"], false);

    let through = dense_through + YEAR;
    analytics.host.set_now((through + 1) * 1000);
    analytics.without_sessions_or_invoices((dense_through, through));
    let mut days = 0;
    for width in [1, 2, 4, 8, 16, 32, 64, 128, 110] {
        let rows: Vec<_> = (days + 1..=days + width)
            .rev()
            .map(|day| payment(201 + day, dense_through + day * DAY))
            .collect();
        analytics.payments(
            (
                dense_through + days * DAY,
                dense_through + (days + width) * DAY,
            ),
            &rows,
        );
        days += width;
    }

    let requests_before = analytics.host.fetched().len();
    let first = analytics.call("sync_revenue", json!({})).await?;
    assert_eq!(first["payment_intents"], 63);
    assert_eq!(first["synced_through"], dense_through + 63 * DAY);
    assert_eq!(first["has_more"], true);
    let last = analytics.call("sync_revenue", json!({})).await?;
    assert_eq!(last["payment_intents"], 302);
    assert_eq!(last["synced_through"], through);
    assert_eq!(last["has_more"], false);
    assert_eq!(analytics.host.fetched().len() - requests_before, 13);
    Ok(())
}

#[tokio::test]
async fn stripe_preserves_the_learned_width_across_a_short_final_window() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.connect_stripe().await?;
    let first_through = YEAR + 5;
    analytics.host.set_now((first_through + 1) * 1000);
    for bounds in [(0, YEAR), (YEAR, first_through)] {
        analytics.without_sessions_or_invoices(bounds);
        analytics.stripe("payment_intents", None, bounds, None, json!([]), false);
    }
    let first = analytics.call("sync_revenue", json!({})).await?;
    assert_eq!(first["synced_through"], first_through);
    assert_eq!(first["has_more"], false);

    let through = first_through + DAY;
    analytics.host.set_now((through + 1) * 1000);
    let bounds = (first_through, through);
    analytics.without_sessions_or_invoices(bounds);
    analytics.payments(bounds, &[payment(1, through)]);
    let requests_before = analytics.host.fetched().len();
    let next = analytics.call("sync_revenue", json!({})).await?;
    assert_eq!(next["payment_intents"], 1);
    assert_eq!(next["synced_through"], through);
    assert_eq!(next["has_more"], false);
    assert_eq!(analytics.host.fetched().len() - requests_before, 3);
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
    analytics.host.set_now(1_001_000);
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

#[tokio::test]
async fn funnel_until_is_exclusive_for_every_step_and_defaults_to_now() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.claim("alice", "example.com").await?;
    analytics.caller_file("alice")?.execute_batch(
        "INSERT INTO hits (domain,visitor,session,kind,path,channel,name,at) VALUES
         ('example.com','before','s','view','/start','direct',NULL,9),
         ('example.com','before','s','event',NULL,'direct','signup',10),
         ('example.com','inside','s','view','/start','direct',NULL,10),
         ('example.com','inside','s','event',NULL,'direct','signup',19),
         ('example.com','edge','s','view','/start','direct',NULL,11),
         ('example.com','edge','s','event',NULL,'direct','signup',20),
         ('example.com','after','s','view','/start','direct',NULL,20),
         ('example.com','after','s','event',NULL,'direct','signup',21);",
    )?;
    analytics.host.set_now(20_000);
    let args = json!({"steps":["/start","signup"],"since":10});
    let default = analytics.call("funnel", args.clone()).await?;
    assert_eq!(default["steps"][0]["visitors"], 2);
    assert_eq!(default["steps"][1]["visitors"], 1);
    let mut args = args;
    args["until"] = json!(20);
    assert_eq!(analytics.call("funnel", args.clone()).await?, default);
    args["until"] = json!(10);
    assert_eq!(
        analytics.call("funnel", args).await?["steps"],
        json!([{"step":"/start","visitors":0},{"step":"signup","visitors":0}])
    );
    Ok(())
}

#[tokio::test]
async fn channels_groups_touches_and_landing_within_the_range() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.claim("alice", "example.com").await?;
    analytics.caller_file("alice")?.execute_batch(
        "INSERT INTO hits (domain,visitor,session,kind,path,channel,source,medium,campaign,at) VALUES
         ('example.com','buyer','old','view','/before','old','old','old','old',9),
         ('example.com','buyer','first','view','/landing','search','engine','organic','spring',10),
         ('example.com','buyer','first','view','/docs','mail','newsletter','email','launch',10),
         ('example.com','buyer','later','view','/later','ads','ads','cpc','paid',20),
         ('example.com','browser','new','event','/event','direct',NULL,NULL,NULL,11),
         ('example.com','browser','new','view','/home','direct',NULL,NULL,NULL,12),
         ('example.com','browser','new','view','/outside','outside','outside','outside','outside',30);
         INSERT INTO customers (id,visitor) VALUES ('cus_buyer','buyer');
         INSERT INTO payments (id,customer,amount,currency,at) VALUES
         ('pi_in','cus_buyer',100,'usd',20),
         ('pi_out','cus_buyer',200,'eur',30);",
    )?;
    analytics.host.set_now(30_000);
    for (by, first, last, browser) in [
        ("channel", "search", "mail", "direct"),
        ("source", "engine", "newsletter", "unattributed"),
        ("medium", "organic", "email", "unattributed"),
        ("campaign", "spring", "launch", "unattributed"),
        ("landing", "/landing", "/landing", "/home"),
    ] {
        for (model, buyer) in [("first", first), ("last", last)] {
            let report = analytics
                .call(
                    "channels",
                    json!({"since":10,"until":30,"model":model,"by":by}),
                )
                .await?;
            let channels = report["channels"].as_array().unwrap();
            assert_eq!(channels.len(), 2, "{by} {model}");
            assert!(
                channels.contains(&json!({
                    "channel":buyer,"visitors":1,"paying_customers":1,"revenue":{"usd":100}
                })),
                "{by} {model}: {channels:?}"
            );
            assert!(
                channels.contains(&json!({
                    "channel":browser,"visitors":1,"paying_customers":0,"revenue":{}
                })),
                "{by} {model}: {channels:?}"
            );
            if by == "channel" {
                assert_eq!(
                    analytics
                        .call("channels", json!({"since":10,"model":model}))
                        .await?,
                    report
                );
            }
        }
    }
    assert_eq!(
        analytics
            .call(
                "channels",
                json!({"since":30,"until":30,"model":"last","by":"landing"})
            )
            .await?["channels"],
        json!([])
    );
    Ok(())
}

#[tokio::test]
async fn stats_compares_periods_and_reports_daily_and_weekly_activity() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.claim("alice", "example.com").await?;
    let since = 11 * DAY;
    let until = 19 * DAY;
    let previous = 3 * DAY;
    analytics.caller_file("alice")?.execute_batch(&format!(
        "INSERT INTO hits (domain,visitor,session,kind,path,channel,at) VALUES
         ('example.com','outside','s','view','/','direct',{before}),
         ('example.com','prior','s','view','/','direct',{previous}),
         ('example.com','carry','s','view','/','direct',{earlier}),
         ('example.com','a','s','view','/','direct',{since}),
         ('example.com','carry','s','view','/next','direct',{since}+1),
         ('example.com','b','s','view','/','direct',{since}+2),
         ('example.com','b','s','event',NULL,'direct',{since}+3),
         ('example.com','a','s','view','/next','direct',{since}+{DAY}),
         ('example.com','c','s','event',NULL,'direct',{until}-{DAY}),
         ('example.com','out','s','view','/','direct',{until});
         INSERT INTO payments (id,customer,visitor,amount,currency,at) VALUES
         ('pi_before',NULL,'outside',99,'usd',{before}),
         ('pi_previous','cus_prior','prior',10,'usd',{previous}),
         ('pi_first','cus_a','a',100,'usd',{since}),
         ('pi_second','cus_a','a',200,'eur',{since}+{DAY}),
         ('pi_third','cus_a','a',50,'usd',{until}-{DAY}),
         ('pi_b',NULL,'b',40,'usd',{since}+4),
         ('pi_unknown',NULL,NULL,60,'eur',{until}-{DAY}),
         ('pi_out',NULL,'out',99,'usd',{until});",
        before = previous - 1,
        earlier = since - 1,
    ))?;
    analytics.host.set_now(until * 1000);
    let day = analytics.call("stats", json!({"since":since})).await?;
    assert_eq!(day["since"], since);
    assert_eq!(day["until"], until);
    assert_eq!(
        day["totals"],
        json!({
            "visitors":4,"paying_customers":3,"revenue":{"usd":190,"eur":260},
            "conversion_rate":0.75,"revenue_per_visitor":{"usd":47.5,"eur":65.0},"bounce_rate":0.5
        })
    );
    assert_eq!(
        day["previous"],
        json!({
            "since":previous,"until":since,"totals":{
                "visitors":2,"paying_customers":1,"revenue":{"usd":10},
                "conversion_rate":0.5,"revenue_per_visitor":{"usd":5.0},"bounce_rate":0.5
            }
        })
    );
    assert_eq!(
        day["series"],
        json!([
            {"since":since,"until":since+DAY,"totals":{
                "visitors":3,"paying_customers":2,"revenue":{"usd":140},
                "conversion_rate":2.0/3.0,"revenue_per_visitor":{"usd":140.0/3.0},"bounce_rate":0.5
            }},
            {"since":since+DAY,"until":since+2*DAY,"totals":{
                "visitors":1,"paying_customers":1,"revenue":{"eur":200},
                "conversion_rate":1.0,"revenue_per_visitor":{"eur":200.0}
            }},
            {"since":until-DAY,"until":until,"totals":{
                "visitors":1,"paying_customers":2,"revenue":{"usd":50,"eur":60},
                "conversion_rate":2.0,"revenue_per_visitor":{"usd":50.0,"eur":60.0}
            }}
        ])
    );

    let week = analytics
        .call(
            "stats",
            json!({"since":since,"until":until,"interval":"week"}),
        )
        .await?;
    assert_eq!(week["totals"], day["totals"]);
    assert_eq!(week["previous"], day["previous"]);
    assert_eq!(week["series"].as_array().unwrap().len(), 2);
    assert_eq!(
        week["series"][0],
        json!({
            "since":since,"until":since+7*DAY,"totals":{
                "visitors":3,"paying_customers":2,"revenue":{"usd":140,"eur":200},
                "conversion_rate":2.0/3.0,"revenue_per_visitor":{"usd":140.0/3.0,"eur":200.0/3.0},
                "bounce_rate":0.5
            }
        })
    );
    assert_eq!(week["series"][1], day["series"][2]);

    let empty = analytics
        .call("stats", json!({"since":until,"until":until}))
        .await?;
    assert_eq!(empty["series"], json!([]));
    assert_eq!(
        empty["totals"],
        json!({
            "visitors":0,"paying_customers":0,"revenue":{},"revenue_per_visitor":{}
        })
    );
    assert_eq!(empty["previous"]["totals"], empty["totals"]);
    let revenue_only = analytics
        .call("stats", json!({"since":since+4,"until":since+5}))
        .await?;
    assert_eq!(
        revenue_only["totals"],
        json!({
            "visitors":0,"paying_customers":1,"revenue":{"usd":40},"revenue_per_visitor":{}
        })
    );
    assert_eq!(
        revenue_only["series"],
        json!([{
            "since":since+4,"until":since+5,"totals":revenue_only["totals"]
        }])
    );
    Ok(())
}

#[tokio::test]
async fn journey_follows_customer_and_visitor_ids_in_time_order() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.claim("alice", "example.com").await?;
    analytics.caller_file("alice")?.execute_batch(&format!(
        "INSERT INTO hits (domain,visitor,session,kind,path,channel,at) VALUES
         ('example.com','old','s','event','/ignored','direct',10),
         ('example.com','old','s','view','/entry','search',11),
         ('example.com','old','s','view','/pricing','direct',11),
         ('example.com','new','s','view','/return','newsletter',20);
         INSERT INTO customers (id,visitor) VALUES ('cus_a','new'), ('cus_no_visits','missing');
         INSERT INTO payments (id,customer,visitor,session,amount,currency,at) VALUES
         ('pi_first','cus_a','old','s',100,'usd',11+{DAY}),
         ('pi_renewal','cus_a',NULL,NULL,200,'eur',11+2*{DAY}),
         ('pi_other','cus_other','old',NULL,99,'usd',11+3*{DAY}),
         ('pi_no_visits','cus_no_visits',NULL,NULL,50,'eur',11+{DAY});"
    ))?;
    let report = analytics
        .call("journey", json!({"customer":"cus_a"}))
        .await?;
    assert_eq!(
        report["visits"],
        json!([
            {"visitor":"old","domain":"example.com","session":"s","at":11,
             "landing":"/entry","channel":"search","pages":["/entry","/pricing"],"pages_truncated":false},
            {"visitor":"new","domain":"example.com","session":"s","at":20,
             "landing":"/return","channel":"newsletter","pages":["/return"],"pages_truncated":false}
        ])
    );
    assert_eq!(report["visits_truncated"], false);
    assert_eq!(
        report["payments"],
        json!([
            {"id":"pi_first","customer":"cus_a","visitor":"old","session":"s",
             "amount":100,"currency":"usd","at":11+DAY},
            {"id":"pi_renewal","customer":"cus_a","visitor":"new","amount":200,"currency":"eur","at":11+2*DAY}
        ])
    );
    assert_eq!(report["days_to_first_payment"], 1.0);
    let visitor = analytics.call("journey", json!({"customer":"old"})).await?;
    assert_eq!(visitor["visits"], json!([report["visits"][0]]));
    assert_eq!(visitor["payments"].as_array().unwrap().len(), 2);
    assert_eq!(visitor["payments"][1]["id"], "pi_other");

    let no_visits = analytics
        .call("journey", json!({"customer":"cus_no_visits"}))
        .await?;
    assert_eq!(no_visits["visits"], json!([]));
    assert_eq!(no_visits["payments"][0]["id"], "pi_no_visits");
    assert!(no_visits.get("days_to_first_payment").is_none());
    Ok(())
}

#[tokio::test]
async fn journey_reports_when_visits_pages_or_payments_are_truncated() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.claim("alice", "example.com").await?;
    analytics.caller_file("alice")?.execute_batch(
        "WITH RECURSIVE x(n) AS (VALUES(1) UNION ALL SELECT n + 1 FROM x WHERE n < 51)
         INSERT INTO hits (domain,visitor,session,kind,path,channel,at)
         SELECT 'example.com','visitor',CASE WHEN n <= 26 THEN 's' ELSE 's' || n END,
                'view','/' || n,'direct',n FROM x;
         WITH RECURSIVE x(n) AS (VALUES(1) UNION ALL SELECT n + 1 FROM x WHERE n < 101)
         INSERT INTO payments (id,visitor,amount,currency,at)
         SELECT 'pi_' || n,'visitor',1,'usd',n + 100 FROM x;",
    )?;
    let report = analytics
        .call("journey", json!({"customer":"visitor"}))
        .await?;
    let visits = report["visits"].as_array().unwrap();
    assert_eq!(visits.len(), 25);
    assert_eq!(report["visits_truncated"], true);
    assert_eq!(visits[0]["landing"], "/1");
    assert_eq!(visits[0]["pages"].as_array().unwrap().len(), 25);
    assert_eq!(visits[0]["pages_truncated"], true);
    assert_eq!(visits[24]["session"], "s50");
    assert_eq!(report["payments"].as_array().unwrap().len(), 100);
    assert_eq!(report["payments_truncated"], true);

    analytics.caller_file("alice")?.execute_batch(
        "WITH RECURSIVE x(n) AS (VALUES(0) UNION ALL SELECT n + 1 FROM x WHERE n < 99)
         INSERT INTO hits (domain,visitor,session,kind,path,channel,at)
         SELECT 'example.com','long-pages','s' || (n / 25), 'view',
                '/' || replace(hex(zeroblob(1023)), '0', char(1)), 'direct',n FROM x;",
    )?;
    let long = analytics
        .call("journey", json!({"customer":"long-pages"}))
        .await?;
    assert_eq!(long["visits_truncated"], true);
    assert_eq!(long["visits"].as_array().unwrap().len(), 2);
    Ok(())
}

#[tokio::test]
async fn reports_fit_their_budgets_with_fifty_thousand_hits() -> Result<()> {
    let analytics = Analytics::open().await?;
    analytics.claim("alice", "example.com").await?;
    analytics.connect_stripe().await?;
    let since = 1_700_000_000;
    let until = since + 60 * DAY;
    let cursor = since - DAY;
    let stripe_window = (cursor, until - 1);
    analytics.host.set_now(until * 1000);
    analytics.without_sessions_or_invoices(stripe_window);
    let new_payments: Vec<_> = (0..200)
        .map(|n| {
            let mut row = payment(1000 + n, until - 1);
            row["metadata"] = json!({"crabtalk_visitor_id":format!("visitor-{}", n + 1015)});
            row
        })
        .collect();
    analytics.payments(stripe_window, &new_payments);

    let caller = analytics.caller_file("alice")?;
    let shared = analytics.shared()?;
    caller.execute_batch(&format!(
        "INSERT INTO cursors (kind,created) VALUES
         ('checkout/sessions',{cursor}), ('payment_intents',{cursor}), ('invoices',{cursor});
         WITH RECURSIVE x(n) AS (VALUES(0) UNION ALL SELECT n + 1 FROM x WHERE n < 44999)
         INSERT INTO hits (domain,visitor,session,kind,path,channel,source,medium,campaign,at)
         SELECT 'example.com',
                CASE WHEN n < 90 THEN 'returning' ELSE 'visitor-' || (n / 6) END,
                'session-' || (n / 3), 'view',
                CASE n % 3 WHEN 0 THEN '/entry/' || (n / 3 % 25) ELSE '/docs/' || (n % 100) END,
                'source-' || (n / 3 % 20),
                'source-' || (n / 3 % 20), 'referral', 'campaign-' || (n / 3 % 30),
                {since} + (n / 3 % 59) * {DAY} + (n % 3) * 60
         FROM x;
         WITH RECURSIVE x(n) AS (VALUES(0) UNION ALL SELECT n + 1 FROM x WHERE n < 999)
         INSERT INTO payments (id,customer,visitor,amount,currency,at)
         SELECT 'pi_' || n, 'cus_' || n,
                CASE WHEN n = 0 THEN 'returning' ELSE 'visitor-' || (n + 15) END,
                100, CASE WHEN n % 3 = 0 THEN 'eur' ELSE 'usd' END, {until}-1
         FROM x;"
    ))?;

    for (tool, args) in [
        (
            "funnel",
            json!({"since":since,"until":until,"steps":[
                "/entry/*","/docs/*","/docs/*","/entry/*","/docs/*","/docs/*","/entry/*","/docs/*"
            ]}),
        ),
        (
            "channels",
            json!({"since":since,"until":until,"model":"last","by":"landing"}),
        ),
        (
            "stats",
            json!({"since":since,"until":until,"interval":"day"}),
        ),
        ("journey", json!({"customer":"cus_0"})),
    ] {
        shared.execute_batch(&format!(
            "WITH RECURSIVE x(n) AS (VALUES(0) UNION ALL SELECT n + 1 FROM x WHERE n < 4999)
             INSERT INTO buffer (domain,visitor,session,kind,path,source,medium,campaign,at)
             SELECT 'example.com', 'visitor-' || (7500 + n / 6), 'buffer-' || (n / 3),
                    'view', CASE WHEN n % 3 = 0 THEN '/entry/newsletter' ELSE '/docs/intro' END,
                    'newsletter', 'email', 'summer', {until} - {DAY} + n FROM x;"
        ))?;
        let result = analytics.call(tool, args).await;
        let meter = analytics
            .host
            .host
            .meters(SERVICE)
            .await?
            .into_iter()
            .find(|meter| meter.tool == tool)
            .unwrap();
        eprintln!(
            "{tool}: cpu={}us sql_cpu={}us read={} written={} memory={}B",
            meter.cpu, meter.sql_cpu, meter.read, meter.written, meter.memory,
        );
        let report = result.with_context(|| format!("reporting with {tool}"))?;
        match tool {
            "funnel" => assert_eq!(report["steps"][1]["visitors"], 8320),
            "channels" => {
                let visitors: u64 = report["channels"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|channel| channel["visitors"].as_u64().unwrap())
                    .sum();
                assert_eq!(visitors, 8320);
                assert_eq!(report["revenue_fully_synced"], true);
            }
            "stats" => {
                assert_eq!(report["totals"]["visitors"], 8320);
                assert_eq!(report["totals"]["paying_customers"], 1200);
            }
            "journey" => assert_eq!(report["visits_truncated"], true),
            _ => unreachable!(),
        }
        let budget = analytics.service.budget(tool).unwrap();
        assert!(meter.cpu <= budget.cpu, "{tool} exceeded its CPU budget");
        assert!(meter.read <= budget.read, "{tool} exceeded its read budget");
        assert!(
            meter.written <= budget.written,
            "{tool} exceeded its write budget"
        );
        assert!(
            meter.memory <= budget.memory,
            "{tool} exceeded its memory budget"
        );
        let count: i64 = caller.query_row("SELECT count(*) FROM hits", [], |row| row.get(0))?;
        assert_eq!(count, 50_000);
        caller.execute("DELETE FROM hits WHERE buffer_id IS NOT NULL", [])?;
    }
    Ok(())
}
