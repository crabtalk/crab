use berm_api::{DEFAULT_BUFFER, project::Project};
use std::fs;
fn read(text: &str) -> anyhow::Result<Project> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crab.toml");
    fs::write(&path, text).unwrap();
    Project::read(&path)
}
const GITHUB: &str = r#"
[credentials.github]
title = "GitHub OAuth app"
guide = "Create one."
copy = { callback = "Authorization callback URL" }
hosts = ["api.github.com"]
secrets = [
    { name = "GITHUB_CLIENT_ID", label = "Client ID", masked = false, pattern = '^Ov23li' },
    { name = "GITHUB_CLIENT_SECRET", label = "Client secret" },
]
"#;
#[test]
fn credential_hosts_and_secrets_join_dependencies() {
    let deps = read(GITHUB).unwrap().deps();
    assert_eq!(deps.secrets, ["GITHUB_CLIENT_ID", "GITHUB_CLIENT_SECRET"]);
    assert_eq!(deps.http.fetch, ["api.github.com"]);
    let credential = &deps.credentials["github"];
    assert_eq!(credential.copy["callback"], "Authorization callback URL");
    assert!(!credential.secrets[0].masked);
    assert!(credential.secrets[1].masked);
    assert_eq!(credential.secrets[0].pattern.as_deref(), Some("^Ov23li"));
}
#[test]
fn duplicate_secrets_and_invalid_patterns_are_refused() {
    assert!(read(&GITHUB.replace("GITHUB_CLIENT_SECRET", "GITHUB_CLIENT_ID")).is_err());
    assert!(read(&GITHUB.replace("masked = false", "masekd = false")).is_err());
    assert!(read(&GITHUB.replace("^Ov23li", "^(Ov23li")).is_err());
}
#[test]
fn credential_fetch_duplicates_compare_exact_strings() {
    let error = read(&format!("{GITHUB}\n[http]\nfetch = [\"api.github.com\"]")).unwrap_err();
    assert!(
        format!("{error:#}").contains("`api.github.com` is already allowed by credential `github`")
    );
    let project = read(&format!(
        "{GITHUB}\n[http]\nfetch = [\"*.github.com\"]\nproxy = [\"api.github.com\"]"
    ))
    .unwrap();
    assert_eq!(
        project.deps().http.fetch,
        ["*.github.com", "api.github.com"]
    );
    assert_eq!(project.deps().http.proxy, ["api.github.com"]);
}
#[test]
fn retired_project_keys_are_refused_without_a_compatibility_path() {
    for text in [
        "image = \"fixtures/app\"",
        "logo = \"https://example.com/logo.svg\"",
        "public = true",
        "template = true",
        "deploy = false",
        "formerly = []",
        "secrets = []",
        "[crons]",
        "[workflows.flow.schedule]\ncron = \"0 8 * * *\"\ninput = {}",
        "[tools.search]\naccess = \"public\"",
        "name = \"old\"",
        "calls = []",
        "[syscalls]",
        "[http]\nftech = []",
    ] {
        assert!(read(text).is_err(), "accepted {text}");
    }
}
#[test]
fn image_publication_and_logos_use_one_table() {
    assert!(read("").unwrap().image.is_none());
    let private = read("[image]\nname = \"fixtures/app\"").unwrap();
    let image = private.image.unwrap();
    assert!(!image.public && !image.template);
    for (public, template) in [(true, false), (false, true)] {
        let image = read(&format!(
            "[image]\nname = \"fixtures/app\"\npublic = {public}\ntemplate = {template}"
        ))
        .unwrap()
        .image
        .unwrap();
        assert_eq!((image.public, image.template), (public, template));
    }
    assert!(
        read("[image]\nname = \"fixtures/app\"\npublic = true\ntemplate = true")
            .unwrap_err()
            .to_string()
            .contains("can't be set on a template")
    );
    let logo = read("[image]\nname = \"fixtures/app\"\nlogo = \"https://example.com/logo.svg\"")
        .unwrap()
        .image
        .unwrap()
        .logo
        .unwrap();
    assert_eq!(logo.as_str(), "https://example.com/logo.svg");
    for url in [
        "http://example.com/logo.svg",
        "logo.svg",
        "//example.com/logo.svg",
        "https://",
        "data:image/png;base64,AA==",
        "",
    ] {
        assert!(read(&format!("[image]\nname = \"fixtures/app\"\nlogo = {url:?}")).is_err());
    }
    assert!(
        private_image()
            .check_kit()
            .unwrap_err()
            .to_string()
            .contains("image")
    );
}
fn private_image() -> Project {
    read("[image]\nname = \"fixtures/app\"").unwrap()
}
#[test]
fn migration_identity_and_instance_names_are_stable() {
    let service=read("id = \"732e70b4fd594e91a94da0f60f50a948\"\n[database]\nmigrations = \"migrations\"\n[image]\nname = \"fixtures/watch\"").unwrap();
    assert_eq!(service.instance("package"), "watch");
    service.check_dependency().unwrap();
    assert!(read("id = \"invalid\"").unwrap().check_service().is_err());
    assert!(
        read("[database]\nmigrations = \"migrations\"")
            .unwrap()
            .check_dependency()
            .is_err()
    );
    assert_eq!(read("").unwrap().instance("package"), "package");
}
#[test]
fn smaller_buffers_and_explicit_credential_hosts_remain_valid() {
    let project=read("buffer = 1024\n[credentials.manual]\ntitle = \"API token\"\nhosts = [\"api.example.com\"]\nsecrets = [{name = \"API_KEY\"}]").unwrap();
    project.check_credentials(&[]).unwrap();
    assert_eq!(project.deps().http.fetch, ["api.example.com"]);
    assert_eq!(project.buffer(), 1024);
    assert_eq!(read("").unwrap().buffer(), DEFAULT_BUFFER);
    assert_eq!(DEFAULT_BUFFER, 1 << 20);
}
#[test]
fn schedules_parse_both_forms_and_reject_unknown_keys_and_invalid_crons() {
    let project = read(
        "[schedules]\ntick = \"*/5 * * * *\"\nflow = {cron = \"0 8 * * *\", input = {x = 1}} ",
    )
    .unwrap();
    assert_eq!(project.schedules["tick"].cron(), "*/5 * * * *");
    assert_eq!(project.schedules["flow"].input().unwrap()["x"], 1);
    for text in [
        "[schedules]\ntick = \"bad\"",
        "[schedules]\ntick = {cron = \"* * * * *\", typo = true}",
    ] {
        assert!(read(text).is_err());
    }
}
#[test]
fn auth_credential_hosts_are_callable_without_changing_secret_destinations() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../services/kits/auth/crab.toml");
    let project = Project::read(&path).unwrap();
    project.check_kit().unwrap();
    let deps = project.deps();
    assert_eq!(deps.credential_of("GOOGLE_CLIENT_SECRET"), Some("google"));
    assert!(deps.http.fetch.contains(&"github.com".into()));
    assert_eq!(
        deps.secret_hosts("GOOGLE_CLIENT_SECRET"),
        ["oauth2.googleapis.com"]
    );
}

#[test]
fn every_explicit_metadata_field_counts_as_an_override_even_at_its_default() {
    use berm_api::Kit;
    let kit_project =
        read("[credentials.supabase]\ntitle = \"Supabase\"\nsecrets = [{name = \"SUPABASE_KEY\"}]")
            .unwrap();
    let kit = Kit {
        name: "supabase-kit".into(),
        id: None,
        version: "1.0.0".into(),
        deps: kit_project.deps(),
        database: None,
    };
    let binding = "[credentials.supabase]\nhosts = [\"project.supabase.co\"]\n";
    read(binding)
        .unwrap()
        .check_credentials(std::slice::from_ref(&kit))
        .unwrap();
    for field in [
        "title = \"\"",
        "guide = \"\"",
        "copy = {}",
        "optional = false",
        "secrets = []",
    ] {
        let project = read(&format!("{binding}{field}\n")).unwrap();
        let error = project
            .check_credentials(std::slice::from_ref(&kit))
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "credential `supabase` is declared by kit `supabase-kit`; set only `hosts`."
        );
    }
    let error = read(binding).unwrap().check_credentials(&[]).unwrap_err();
    assert!(error.to_string().contains("no linked kit declares it"));
    assert!(
        read(&format!(
            "{binding}\n[http]\nfetch = [\"project.supabase.co\"]"
        ))
        .is_err()
    );
}
