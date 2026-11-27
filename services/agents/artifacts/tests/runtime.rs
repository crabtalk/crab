use anyhow::{Result, bail};
use berm_api::{Manifest, Output, host::Caller};
use berm_build::fixture;
use berm_host::test::TestHost;
use berm_lang::serde_json::{Value, json};

const PACKAGE: &str = "crab-agent-artifacts";
const SERVICE: &str = "artifacts";

struct Artifacts(TestHost);

impl Artifacts {
    async fn open() -> Result<Self> {
        let host = TestHost::open(1000).await?;
        host.deploy(SERVICE, &fixture::image(PACKAGE)?).await?;
        Ok(Self(host))
    }

    async fn call(&self, tool: &str, input: Value) -> Result<Value> {
        self.as_account("account-1", None, None, tool, input).await
    }

    async fn as_account(
        &self,
        id: &str,
        account: Option<&str>,
        key: Option<&str>,
        tool: &str,
        input: Value,
    ) -> Result<Value> {
        let caller = Caller {
            id: id.to_owned(),
            account: account.map(str::to_owned),
            key: key.map(str::to_owned),
            app: None,
            user: None,
            client: None,
            agent: None,
        };
        let (_, output) = self
            .0
            .host
            .call(
                SERVICE,
                Some(&caller),
                tool,
                input.to_string().into_bytes(),
                false,
            )
            .await?;
        match output {
            Output::Done(value) => Ok(berm_lang::serde_json::from_str(&value)?),
            Output::Failed(error) => bail!("{error}"),
        }
    }
}

#[test]
fn manifest_is_public_caller_only_and_free() -> Result<()> {
    let bytes = fixture::image(PACKAGE)?;
    let manifest = Manifest::try_from(bytes.as_slice())?;
    assert!(manifest.public);
    assert!(manifest.database.is_none());
    assert!(manifest.caller_storage.is_some());
    assert_eq!(manifest.tools.len(), 10);
    for tool in &manifest.tools {
        assert!(tool.price.is_none(), "{}", tool.name);
        assert!(
            tool.catalog
                .as_deref()
                .is_some_and(|job| job.starts_with("artifacts."))
        );
        assert!(
            tool.parameters
                .get("properties")
                .and_then(|p| p.get("user"))
                .is_none()
        );
    }
    Ok(())
}

#[tokio::test]
async fn projects_and_numbers_are_scoped_by_account_and_project() -> Result<()> {
    let artifacts = Artifacts::open().await?;
    assert_eq!(artifacts.call("project_list", json!({})).await?, json!([]));
    assert!(
        artifacts
            .call("project_create", json!({"name":"bad/name"}))
            .await
            .is_err()
    );
    for project in ["notes", "other"] {
        artifacts
            .call("project_create", json!({"name":project}))
            .await?;
        assert_eq!(
            artifacts
                .call(
                    "article_write",
                    json!({"project":project,"title":"first","markdown":project})
                )
                .await?["number"],
            1
        );
    }
    assert!(
        artifacts
            .call("project_create", json!({"name":"notes"}))
            .await
            .is_err()
    );
    assert_eq!(
        artifacts.call("project_list", json!({})).await?,
        json!([{"name":"notes"},{"name":"other"}])
    );
    assert!(
        artifacts
            .as_account(
                "account-2",
                None,
                None,
                "read",
                json!({"project":"notes","entry":"1"})
            )
            .await
            .is_err()
    );
    artifacts
        .as_account(
            "member-1",
            Some("org-1"),
            Some("key-1"),
            "project_create",
            json!({"name":"notes"}),
        )
        .await?;
    assert_eq!(
        artifacts
            .as_account(
                "member-2",
                Some("org-1"),
                Some("key-2"),
                "project_list",
                json!({})
            )
            .await?,
        json!([{"name":"notes"}]),
    );
    assert_eq!(
        artifacts
            .as_account("account-2", None, None, "project_list", json!({}))
            .await?,
        json!([])
    );
    Ok(())
}

#[tokio::test]
async fn articles_search_read_and_carry_comments_through_edits() -> Result<()> {
    let artifacts = Artifacts::open().await?;
    artifacts
        .call("project_create", json!({"name":"notes"}))
        .await?;
    artifacts
        .call("project_create", json!({"name":"other"}))
        .await?;
    artifacts
        .call(
            "article_write",
            json!({"project":"other","title":"Guide","markdown":"Rollout"}),
        )
        .await?;
    let body = "# Guide\n\n## Setup\nChoose a rollout.\n\n## Finish\ncafé";
    let article = artifacts
        .call(
            "article_write",
            json!({"project":"notes","title":"Guide","markdown":body}),
        )
        .await?;
    assert_eq!(article["reference"], "#1");

    assert_eq!(
        artifacts
            .call("read", json!({"project":"notes","entry":"1:7"}))
            .await?["markdown"],
        "café"
    );
    let section = artifacts
        .call("read", json!({"project":"notes","entry":"1#setup"}))
        .await?;
    assert_eq!(section["markdown"], "## Setup\nChoose a rollout.\n\n");
    assert_eq!(section["lines"], "3-5");
    let found = artifacts
        .call("search", json!({"project":"notes","query":"rollout"}))
        .await?;
    assert_eq!(found.as_array().unwrap().len(), 1);
    assert_eq!(found[0]["reference"], "#1:4");
    assert_eq!(found[0]["line"], 4);
    let comment = artifacts
        .call(
            "comment",
            json!({"project":"notes","entry":"1#setup","quote":"rollout","note":"Add an example."}),
        )
        .await?;
    assert_eq!(comment["comment"]["quote"], "rollout");

    artifacts.call("article_edit", json!({"project":"notes","entry":"1:4","markdown":"First choose a rollout.\nThen verify it.\n"})).await?;
    let edited = artifacts
        .call("read", json!({"project":"notes","entry":"#1"}))
        .await?;
    assert_eq!(
        edited["markdown"],
        "# Guide\n\n## Setup\nFirst choose a rollout.\nThen verify it.\n\n## Finish\ncafé"
    );
    let note = &edited["comments"]["comment"][0];
    let start = note["start"].as_u64().unwrap() as usize;
    let end = note["end"].as_u64().unwrap() as usize;
    assert_eq!(&edited["markdown"].as_str().unwrap()[start..end], "rollout");
    assert_eq!(
        artifacts
            .call("search", json!({"project":"notes","query":"verify"}))
            .await?[0]["reference"],
        "#1:5"
    );
    assert_eq!(
        artifacts
            .call("search", json!({"project":"notes","query":"otherword"}))
            .await?,
        json!([])
    );
    assert!(
        artifacts
            .call(
                "read",
                json!({"project":"notes","entry":"another-account/notes/1"})
            )
            .await
            .is_err()
    );
    Ok(())
}

#[tokio::test]
async fn board_numbers_cards_and_reciprocal_blockers_survive_changes() -> Result<()> {
    let artifacts = Artifacts::open().await?;
    for name in ["notes", "other"] {
        artifacts
            .call("project_create", json!({"name":name}))
            .await?;
    }
    artifacts
        .call(
            "article_write",
            json!({"project":"notes","title":"Guide","markdown":"Getting started"}),
        )
        .await?;
    assert!(
        artifacts
            .call(
                "card_add",
                json!({"project":"notes","board":"TEMP","column":" ","text":"No lane"})
            )
            .await
            .is_err()
    );
    let one = artifacts
        .call(
            "card_add",
            json!({"project":"notes","board":"DEV","column":"todo","text":"Ship the playbook"}),
        )
        .await?;
    assert_eq!(one["reference"], "DEV-1");
    assert_eq!(one["board"], 2);
    assert_eq!(one["column"], "TODO");
    let second = artifacts
        .call(
            "card_add",
            json!({"project":"notes","board":"dev","column":"todo","text":"Check exports"}),
        )
        .await?;
    assert_eq!(second["reference"], "DEV-2");
    artifacts
        .call(
            "card_add",
            json!({"project":"other","board":"OPS","column":"todo","text":"Prepare rollout"}),
        )
        .await?;
    let moved = artifacts
        .call(
            "card_move",
            json!({"project":"notes","card":"dev-1","column":"doing"}),
        )
        .await?;
    assert_eq!(moved["column"], "DOING");
    assert_eq!(moved["card"]["id"], one["card"]["id"]);
    assert_eq!(
        artifacts
            .call("read", json!({"project":"notes","entry":"2"}))
            .await?["board"]["key"],
        "DEV"
    );
    assert_eq!(
        artifacts
            .call("search", json!({"project":"notes","query":"playbook"}))
            .await?[0]["reference"],
        "DEV-1"
    );

    let waiting = artifacts
        .call(
            "card_blockers",
            json!({"project":"notes","card":"DEV-1","blockers":["dev-2","DEV-2","other#OPS-1"]}),
        )
        .await?;
    assert_eq!(
        waiting["card"]["blocked_by"],
        json!(["DEV-2", "other#OPS-1"])
    );
    assert_eq!(
        artifacts
            .call("read", json!({"project":"notes","entry":"DEV-2"}))
            .await?["card"]["blocks"],
        json!(["DEV-1"])
    );
    assert_eq!(
        artifacts
            .call("read", json!({"project":"other","entry":"OPS-1"}))
            .await?["card"]["blocks"],
        json!(["notes#DEV-1"])
    );
    assert!(
        artifacts
            .call(
                "card_blockers",
                json!({"project":"notes","card":"DEV-1","blockers":["DEV-99"]})
            )
            .await
            .is_err()
    );
    assert_eq!(
        artifacts
            .call("read", json!({"project":"notes","entry":"DEV-1"}))
            .await?["card"]["blocked_by"],
        json!(["DEV-2", "other#OPS-1"])
    );
    artifacts
        .call(
            "card_blockers",
            json!({"project":"notes","card":"DEV-1","blockers":[]}),
        )
        .await?;
    assert!(
        artifacts
            .call("read", json!({"project":"other","entry":"OPS-1"}))
            .await?["card"]
            .get("blocks")
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn concurrent_creates_get_distinct_numbers() -> Result<()> {
    let artifacts = Artifacts::open().await?;
    artifacts
        .call("project_create", json!({"name":"notes"}))
        .await?;
    let body = |title| json!({"project":"notes","title":title,"markdown":title});
    let (a, b) = tokio::join!(
        artifacts.call("article_write", body("first")),
        artifacts.call("article_write", body("second")),
    );
    let mut numbers = [
        a?["number"].as_u64().unwrap(),
        b?["number"].as_u64().unwrap(),
    ];
    numbers.sort_unstable();
    assert_eq!(numbers, [1, 2]);
    Ok(())
}
