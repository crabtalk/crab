//! What a manifest is read from: an image's `.berm.abi`, `.berm.service` and
//! `.berm.kits` sections, and what it refuses.

use berm_abi::{ABI_SECTION, ABI_VERSION, KITS_SECTION, SERVICE_SECTION};
use berm_api::{
    Access, Credential, CredentialSecret, Deps, Image, Kit, Manifest, ServiceRecord, host,
};
use serde_json::Value;
use std::collections::BTreeMap;

/// A wasm module holding one custom section, `padding`, empty.
const EMPTY: &[u8] = b"\0asm\x01\0\0\0\0\x08\x07padding";

fn tool(name: &str, krate: &str) -> String {
    format!(
        r#"{{"tool":{{"name":"{name}","description":"d","parameters":{{"type":"object"}}}},"abi_version":{ABI_VERSION},"crate":"{krate}"}}"#
    )
}

fn cron(name: &str) -> String {
    format!(
        r#"{{"tool":{{"name":"{name}","description":"d","parameters":{{"type":"object"}},"cron":true,"access":"internal"}},"abi_version":{ABI_VERSION},"crate":"demo"}}"#
    )
}

fn service() -> Vec<u8> {
    service_record()
}

fn service_record() -> Vec<u8> {
    serde_json::to_vec(&ServiceRecord {
        deploy: false,
        public: false,
        lang: String::new(),
        name: "demo".to_owned(),
        version: "0.1.0".to_owned(),
        description: String::new(),
        homepage: String::new(),
        repository: String::new(),
        readme: String::new(),
        logo: None,
        buffer: 1024,
        deps: Deps {
            secrets: vec!["own".to_owned()],
            ..Deps::default()
        },
        credential_hosts: BTreeMap::new(),
        database: None,
        database_size: host::DATABASE_SIZE,
        workflows: BTreeMap::new(),
        crons: BTreeMap::new(),
    })
    .unwrap()
}

fn kit(name: &str) -> Kit {
    Kit {
        name: name.to_owned(),
        id: None,
        version: "0.1.0".to_owned(),
        deps: Deps {
            secrets: vec![format!("{name}-secret")],
            ..Deps::default()
        },
        database: None,
    }
}

/// Kit `name`, declaring credential `id` holding the secret `secret`.
fn credentialed(name: &str, id: &str, secret: &str) -> Kit {
    let mut kit = kit(name);
    kit.deps.secrets.push(secret.to_owned());
    kit.deps.credentials.insert(
        id.to_owned(),
        Credential {
            title: id.to_owned(),
            guide: String::new(),
            copy: Default::default(),
            hosts: vec!["api.example.com".into()],
            optional: false,
            secrets: vec![CredentialSecret {
                name: secret.to_owned(),
                label: String::new(),
                masked: true,
                pattern: None,
            }],
        },
    );
    kit
}

fn image(abi: &str, service: Option<&[u8]>, kits: &[Kit]) -> Vec<u8> {
    let mut image = Image(EMPTY)
        .with_section(ABI_SECTION, abi.as_bytes())
        .unwrap();
    if let Some(service) = service {
        image = Image(&image)
            .with_section(SERVICE_SECTION, service)
            .unwrap();
    }
    Image(&image)
        .with_section(KITS_SECTION, &serde_json::to_vec(kits).unwrap())
        .unwrap()
}

#[test]
fn a_section_reads_back_as_appended() {
    let image = Image(EMPTY).with_section(".a", b"payload").unwrap();
    let image = Image(&image).with_section(".b", b"").unwrap();
    assert_eq!(Image(&image).section(".a").unwrap(), Some(&b"payload"[..]));
    assert_eq!(Image(&image).section(".b").unwrap(), Some(&b""[..]));
    assert_eq!(Image(&image).section(".c").unwrap(), None);
}

#[test]
fn a_section_is_appended_once() {
    let image = Image(EMPTY).with_section(".a", b"x").unwrap();
    assert!(Image(&image).with_section(".a", b"y").is_err());
}

#[test]
fn the_manifest_joins_the_three_sections() {
    let abi = tool("b", "demo") + &tool("a", "demo");
    let manifest =
        Manifest::try_from(&*image(&abi, Some(&service()), &[kit("k1"), kit("k2")])).unwrap();
    assert_eq!(manifest.name, "demo");
    assert_eq!(manifest.buffer, 1024);
    let tools: Vec<&str> = manifest
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect();
    assert_eq!(tools, ["a", "b"]);
    assert_eq!(manifest.deps.secrets, ["own", "k1-secret", "k2-secret"]);
    assert_eq!(manifest.kits.len(), 2);
}

#[test]
fn the_manifest_reads_back_from_its_json() {
    let manifest = Manifest::try_from(&*image(&tool("a", "demo"), Some(&service()), &[])).unwrap();
    let json = serde_json::to_string(&manifest).unwrap();
    let read: Manifest = json.parse().unwrap();
    assert_eq!(read.tools.len(), 1);
    assert_eq!(read.buffer, 1024);
}

#[test]
fn an_image_without_its_service_section_is_refused() {
    let error = Manifest::try_from(&*image(&tool("a", "demo"), None, &[])).unwrap_err();
    assert!(error.to_string().contains(SERVICE_SECTION), "{error}");
}

#[test]
fn a_tool_compiled_in_another_crate_is_refused() {
    let abi = tool("a", "demo") + &tool("b", "some-kit");
    let error = Manifest::try_from(&*image(&abi, Some(&service()), &[])).unwrap_err();
    assert!(error.to_string().contains("some-kit"), "{error}");
}

#[test]
fn a_kit_named_twice_is_refused() {
    let error = Manifest::try_from(&*image(
        &tool("a", "demo"),
        Some(&service()),
        &[kit("k"), kit("k")],
    ))
    .unwrap_err();
    assert!(error.to_string().contains("kit k twice"), "{error}");
}

#[test]
fn an_image_of_another_abi_is_refused() {
    let old = r#"{"abi_version":4,"tools":[]}"#;
    let error = Manifest::try_from(&*image(old, Some(&service()), &[])).unwrap_err();
    assert!(error.to_string().contains("ABI version 4"), "{error}");
}

#[test]
fn an_image_with_no_tools_is_refused() {
    let error = Manifest::try_from(&*image("", Some(&service()), &[])).unwrap_err();
    assert!(error.to_string().contains("no tools"), "{error}");
}

#[test]
fn a_kits_credentials_reach_the_manifest() {
    let manifest = Manifest::try_from(&*image(
        &tool("a", "demo"),
        Some(&service()),
        &[credentialed("k", "github", "GITHUB_CLIENT_ID")],
    ))
    .unwrap();
    assert_eq!(
        manifest.deps.credential_of("GITHUB_CLIENT_ID"),
        Some("github")
    );
    assert!(
        manifest
            .deps
            .secrets
            .contains(&"GITHUB_CLIENT_ID".to_owned())
    );
}

#[test]
fn a_credential_declared_by_two_kits_is_refused() {
    let error = Manifest::try_from(&*image(
        &tool("a", "demo"),
        Some(&service()),
        &[
            credentialed("k1", "github", "A"),
            credentialed("k2", "github", "B"),
        ],
    ))
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("credential github is declared twice"),
        "{error:#}"
    );
}

#[test]
fn a_secret_in_two_credentials_is_refused() {
    let error = Manifest::try_from(&*image(
        &tool("a", "demo"),
        Some(&service()),
        &[
            credentialed("k1", "github", "A"),
            credentialed("k2", "google", "A"),
        ],
    ))
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("secret A is in two credentials"),
        "{error:#}"
    );
}

#[test]
fn abi_declarations_supply_access_price_title_catalog_and_budget() {
    let mut record: Value = serde_json::from_str(&tool("a", "demo")).unwrap();
    record["tool"]["access"] = "accounts".into();
    record["tool"]["title"] = "Search the web".into();
    record["tool"]["price"] = 2000.into();
    record["tool"]["examples"] = serde_json::json!([
        "Find recent Rust releases",
        "é".repeat(120),
        "Find Svelte updates"
    ]);
    record["tool"]["catalog"] = "web.search".into();
    record["tool"]["budget"] = serde_json::json!({"written":10});
    let abi = record.to_string() + &tool("b", "demo");
    let manifest = Manifest::try_from(&*image(&abi, Some(&service()), &[])).unwrap();
    let a = &manifest.tools[0];
    assert_eq!(a.access(), Some(Access::Accounts));
    assert_eq!(a.price, Some(2000));
    assert_eq!(
        a.examples,
        [
            "Find recent Rust releases".to_owned(),
            "é".repeat(120),
            "Find Svelte updates".to_owned()
        ]
    );
    assert!(manifest.tools[1].examples.is_empty());
    assert_eq!(a.title.as_deref(), Some("Search the web"));
    assert_eq!(a.catalog.as_deref(), Some("web.search"));
    assert_eq!(a.budget.unwrap().written, Some(10));
    assert_eq!(manifest.tools[1].access(), Some(Access::Public));
    assert_eq!(manifest.tools[1].budget, None);
}

#[test]
fn legacy_service_tool_maps_are_refused() {
    let mut service: Value = serde_json::from_slice(&service()).unwrap();
    service["tools"] = serde_json::json!({});
    assert!(
        Manifest::try_from(&*image(
            &tool("a", "demo"),
            Some(&serde_json::to_vec(&service).unwrap()),
            &[]
        ))
        .is_err()
    );
}

#[test]
fn a_cron_tool_given_public_access_is_refused() {
    let public_cron = cron("tick").replace("internal", "public");
    let error = Manifest::try_from(&*image(&public_cron, Some(&service()), &[])).unwrap_err();
    assert!(format!("{error:#}").contains("must be internal"), "{error}");
}

#[test]
fn invalid_compiled_declarations_are_refused() {
    for fields in [
        serde_json::json!({"title":"Ends in a period."}),
        serde_json::json!({"title":"x".repeat(61)}),
        serde_json::json!({"access":"internal","price":100}),
        serde_json::json!({"price":1}),
        serde_json::json!({"budget":{"cpu":0}}),
        serde_json::json!({"idempotency_ms":0}),
        serde_json::json!({"examples":["one", "two", "three", "four"]}),
        serde_json::json!({"examples":["x".repeat(121)]}),
        serde_json::json!({"examples":["   "]}),
    ] {
        let mut record: Value = serde_json::from_str(&tool("a", "demo")).unwrap();
        record["tool"]
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        assert!(Manifest::try_from(&*image(&record.to_string(), Some(&service()), &[])).is_err());
    }
}

#[test]
fn logo_validation_applies_to_images_and_manifest_json() {
    let mut record: Value = serde_json::from_slice(&service()).unwrap();
    record["logo"] = "https://example.com/logo.svg".into();
    let bytes = image(
        &tool("a", "demo"),
        Some(&serde_json::to_vec(&record).unwrap()),
        &[],
    );
    let manifest = Manifest::try_from(bytes.as_slice()).unwrap();
    assert_eq!(
        manifest.logo.as_ref().unwrap().as_str(),
        "https://example.com/logo.svg"
    );
    let mut json = serde_json::to_value(&manifest).unwrap();
    let roundtrip: Manifest = json.to_string().parse().unwrap();
    assert_eq!(roundtrip.logo, manifest.logo);

    record["logo"] = "http://example.com/logo.svg".into();
    let bytes = image(
        &tool("a", "demo"),
        Some(&serde_json::to_vec(&record).unwrap()),
        &[],
    );
    assert!(Manifest::try_from(bytes.as_slice()).is_err());
    json["logo"] = "file:///logo.svg".into();
    assert!(json.to_string().parse::<Manifest>().is_err());
}

#[test]
fn schedules_are_carried_by_the_image_and_must_name_cron_tools() {
    let mut record: Value = serde_json::from_slice(&service()).unwrap();
    record["crons"] = serde_json::json!({"tick":"*/5 * * * *"});
    let record = serde_json::to_vec(&record).unwrap();
    let bytes = image(&cron("tick"), Some(&record), &[]);
    let manifest = Manifest::try_from(bytes.as_slice()).unwrap();
    assert_eq!(manifest.crons["tick"], "*/5 * * * *");
    let bytes = image(&tool("tick", "demo"), Some(&record), &[]);
    assert!(Manifest::try_from(bytes.as_slice()).is_err());
}

#[test]
fn linked_http_destinations_merge_without_duplicates() {
    let mut record: ServiceRecord = serde_json::from_slice(&service()).unwrap();
    record.deps.http.fetch = vec!["own.example".into(), "shared.example".into()];
    let mut linked = kit("http");
    linked.deps.http.fetch = vec!["shared.example".into(), "kit.example".into()];
    linked.deps.http.proxy = vec!["proxy.example".into()];
    let image = image(
        &tool("a", "demo"),
        Some(&serde_json::to_vec(&record).unwrap()),
        &[linked],
    );
    let manifest = Manifest::try_from(image.as_slice()).unwrap();
    assert_eq!(
        manifest.deps.http.fetch,
        ["own.example", "shared.example", "kit.example"]
    );
    assert_eq!(manifest.deps.http.proxy, ["proxy.example"]);
}

#[test]
fn old_image_metadata_keeps_http_destinations_without_inventing_imports() {
    let mut record: Value = serde_json::from_slice(&service()).unwrap();
    record["deps"].as_object_mut().unwrap().remove("http");
    record["deps"]["syscalls"] = serde_json::json!({
        "berm.http.fetch": ["old.example"],
        "berm.sql": [],
        "berm.vouch": []
    });
    let mut linked = serde_json::to_value(kit("old")).unwrap();
    linked["deps"].as_object_mut().unwrap().remove("http");
    linked["deps"]["syscalls"] = serde_json::json!({"berm.http.proxy": ["proxy.example"]});
    let base = Image(EMPTY)
        .with_section(ABI_SECTION, tool("a", "demo").as_bytes())
        .unwrap();
    let base = Image(&base)
        .with_section(SERVICE_SECTION, &serde_json::to_vec(&record).unwrap())
        .unwrap();
    let base = Image(&base)
        .with_section(KITS_SECTION, &serde_json::to_vec(&[linked]).unwrap())
        .unwrap();
    let manifest = Manifest::try_from(base.as_slice()).unwrap();
    assert_eq!(manifest.deps.http.fetch, ["old.example"]);
    assert_eq!(manifest.deps.http.proxy, ["proxy.example"]);
    assert!(manifest.imports.is_empty());
    let mut saved = serde_json::to_value(&manifest).unwrap();
    saved["deps"].as_object_mut().unwrap().remove("http");
    saved["deps"]["syscalls"] = record["deps"]["syscalls"].clone();
    let restored: Manifest = serde_json::from_value(saved).unwrap();
    assert_eq!(restored.deps.http.fetch, ["old.example"]);
    let encoded = serde_json::to_value(restored).unwrap();
    assert!(encoded["deps"].get("syscalls").is_none());
}

#[test]
fn image_database_defaults_survive_manifest_storage_and_legacy_images() {
    let mut record: Value = serde_json::from_slice(&service()).unwrap();
    record["database_size"] = serde_json::json!(64 << 20);
    let bytes = serde_json::to_vec(&record).unwrap();
    let manifest = Manifest::try_from(&*image(&tool("a", "demo"), Some(&bytes), &[])).unwrap();
    assert_eq!(manifest.database_size, 64 << 20);
    let stored: Manifest = serde_json::to_string(&manifest).unwrap().parse().unwrap();
    assert_eq!(stored.database_size, manifest.database_size);
    record.as_object_mut().unwrap().remove("database_size");
    let bytes = serde_json::to_vec(&record).unwrap();
    let legacy = Manifest::try_from(&*image(&tool("a", "demo"), Some(&bytes), &[])).unwrap();
    assert_eq!(legacy.database_size, host::DATABASE_SIZE);
}

#[test]
fn invalid_image_database_defaults_are_refused() {
    let mut record: Value = serde_json::from_slice(&service()).unwrap();
    for size in [host::MIN_DATABASE_SIZE - 1, host::MAX_DATABASE_SIZE + 1] {
        record["database_size"] = serde_json::json!(size);
        let bytes = serde_json::to_vec(&record).unwrap();
        assert!(Manifest::try_from(&*image(&tool("a", "demo"), Some(&bytes), &[])).is_err());
    }
}

#[test]
fn visibility_is_read_from_the_service_image_and_defaults_private() {
    let mut record: Value = serde_json::from_slice(&service()).unwrap();
    for public in [true, false] {
        record["public"] = Value::Bool(public);
        let bytes = image(
            &tool("echo", "demo"),
            Some(&serde_json::to_vec(&record).unwrap()),
            &[],
        );
        let manifest = Manifest::try_from(bytes.as_slice()).unwrap();
        assert_eq!(manifest.public, public);
        let json = serde_json::to_string(&manifest).unwrap();
        assert_eq!(json.parse::<Manifest>().unwrap().public, public);
    }
    record.as_object_mut().unwrap().remove("public");
    let bytes = image(
        &tool("echo", "demo"),
        Some(&serde_json::to_vec(&record).unwrap()),
        &[],
    );
    assert!(!Manifest::try_from(bytes.as_slice()).unwrap().public);
}

#[test]
fn title_limits_count_characters_and_reject_final_periods() {
    assert!(berm_api::ToolSpec::check_title(&"é".repeat(60)).is_ok());
    for title in ["", "Send email.", "two\nlines", &"x".repeat(61)] {
        assert!(berm_api::ToolSpec::check_title(title).is_err());
    }
}

#[test]
fn hosts_only_bindings_resolve_kit_credentials_without_widening_secrets() {
    let mut kit = credentialed("supabase-kit", "supabase", "SUPABASE_KEY");
    kit.deps
        .credentials
        .get_mut("supabase")
        .unwrap()
        .hosts
        .clear();
    let mut record: ServiceRecord = serde_json::from_slice(&service()).unwrap();
    record
        .credential_hosts
        .insert("supabase".into(), vec!["project.supabase.co".into()]);
    let bytes = image(
        &tool("a", "demo"),
        Some(&serde_json::to_vec(&record).unwrap()),
        &[kit],
    );
    let manifest = Manifest::try_from(bytes.as_slice()).unwrap();
    assert_eq!(
        manifest.deps.secret_hosts("SUPABASE_KEY"),
        ["project.supabase.co"]
    );
    assert_eq!(manifest.deps.http.fetch, ["project.supabase.co"]);
    assert_eq!(manifest.deps.credentials["supabase"].title, "supabase");
    assert!(
        serde_json::to_value(&manifest)
            .unwrap()
            .get("credential_hosts")
            .is_none()
    );
}

#[test]
fn kit_bindings_refuse_missing_hosts_unknown_ids_and_fixed_host_overrides() {
    let mut hostless = credentialed("supabase-kit", "supabase", "SUPABASE_KEY");
    hostless
        .deps
        .credentials
        .get_mut("supabase")
        .unwrap()
        .hosts
        .clear();
    let error =
        Manifest::try_from(image(&tool("a", "demo"), Some(&service()), &[hostless]).as_slice())
            .unwrap_err();
    assert!(
        format!("{error:#}")
            .contains("credential `supabase` from kit `supabase-kit` needs `hosts`")
    );
    let mut record: ServiceRecord = serde_json::from_slice(&service()).unwrap();
    record
        .credential_hosts
        .insert("supabase".into(), vec!["project.supabase.co".into()]);
    let bytes = serde_json::to_vec(&record).unwrap();
    let error =
        Manifest::try_from(image(&tool("a", "demo"), Some(&bytes), &[]).as_slice()).unwrap_err();
    assert!(
        format!("{error:#}").contains(
            "credential `supabase` has no title or secrets, and no linked kit declares it"
        )
    );
    let error = Manifest::try_from(
        image(
            &tool("a", "demo"),
            Some(&bytes),
            &[credentialed("supabase-kit", "supabase", "SUPABASE_KEY")],
        )
        .as_slice(),
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("credential supabase is declared twice"));
}

#[test]
fn full_service_declarations_cannot_override_kit_metadata() {
    let kit = credentialed("supabase-kit", "supabase", "SUPABASE_KEY");
    let mut record: ServiceRecord = serde_json::from_slice(&service()).unwrap();
    record
        .deps
        .credentials
        .insert("supabase".into(), kit.deps.credentials["supabase"].clone());
    let bytes = serde_json::to_vec(&record).unwrap();
    let error =
        Manifest::try_from(image(&tool("a", "demo"), Some(&bytes), &[kit]).as_slice()).unwrap_err();
    assert!(
        format!("{error:#}")
            .contains("credential `supabase` is declared by kit `supabase-kit`; set only `hosts`")
    );
}

#[test]
fn signing_credentials_follow_the_root_services_tool_marks() {
    let abi = r#"{"tool":{"name":"receive","description":"Receives a webhook","parameters":{"type":"object"},"webhook":"standard","access":"internal"},"abi_version":ABI,"crate":"demo"}
{"tool":{"name":"stripe","description":"Receives Stripe","parameters":{"type":"object"},"webhook":"stripe","access":"internal"},"abi_version":ABI,"crate":"demo"}"#;
    let abi = abi.replace("ABI", &ABI_VERSION.to_string());
    let manifest = Manifest::try_from(image(&abi, Some(&service()), &[]).as_slice()).unwrap();
    for (id, secret) in [
        ("receive_webhook", "RECEIVE_WEBHOOK_SECRET"),
        ("stripe_webhook", "STRIPE_WEBHOOK_SECRET"),
    ] {
        let credential = &manifest.deps.credentials[id];
        assert!(credential.hosts.is_empty());
        assert_eq!(credential.secrets[0].name, secret);
        assert!(!credential.title.is_empty() && !credential.guide.is_empty());
        assert!(manifest.deps.secrets.iter().any(|name| name == secret));
    }
    assert!(manifest.deps.http.is_empty());
    assert_eq!(manifest.deps.credentials.len(), 2);
    assert_eq!(
        manifest
            .deps
            .secrets
            .iter()
            .filter(|name| name.as_str() == "RECEIVE_WEBHOOK_SECRET")
            .count(),
        1
    );
    let ordinary =
        Manifest::try_from(image(&tool("ordinary", "demo"), Some(&service()), &[]).as_slice())
            .unwrap();
    assert!(ordinary.deps.credentials.is_empty());
}

#[test]
fn an_ordinary_services_credential_requires_hosts() {
    let mut record: ServiceRecord = serde_json::from_slice(&service()).unwrap();
    let mut credential = credentialed("unused", "key", "KEY")
        .deps
        .credentials
        .remove("key")
        .unwrap();
    credential.hosts.clear();
    record.deps.credentials.insert("key".into(), credential);
    let bytes = serde_json::to_vec(&record).unwrap();
    let error =
        Manifest::try_from(image(&tool("a", "demo"), Some(&bytes), &[]).as_slice()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "credential `key` needs `hosts`: the hosts its secrets are sent to."
    );
}

#[test]
fn webhook_credentials_refuse_config_overrides_and_long_secret_names() {
    let abi = tool("receive", "demo").replace(
        "\"parameters\":",
        "\"webhook\":\"standard\",\"access\":\"internal\",\"parameters\":",
    );
    for name in ["receive_webhook", "RECEIVE_WEBHOOK_SECRET"] {
        let mut record: ServiceRecord = serde_json::from_slice(&service()).unwrap();
        if name == "receive_webhook" {
            record
                .credential_hosts
                .insert(name.into(), vec!["api.example.com".into()]);
        } else {
            record.deps.secrets.push(name.into());
        }
        let error = Manifest::try_from(
            image(&abi, Some(&serde_json::to_vec(&record).unwrap()), &[]).as_slice(),
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "`{name}` is the signing secret of the webhook tool `receive`; remove it from crab.toml."
            )
        );
    }
    let boundary = abi.replace("receive", &"a".repeat(49));
    assert!(Manifest::try_from(image(&boundary, Some(&service()), &[]).as_slice()).is_ok());
    let long = abi.replace("receive", &"a".repeat(50));
    let error = Manifest::try_from(image(&long, Some(&service()), &[]).as_slice()).unwrap_err();
    assert!(
        error.to_string().contains("past 64 characters"),
        "{error:#}"
    );
}
