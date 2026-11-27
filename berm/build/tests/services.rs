//! Built service images and the kits they link.
use berm_api::{Manifest, host};
use berm_build::fixture;

fn manifest(package: &str) -> Manifest {
    Manifest::try_from(fixture::image(package).unwrap().as_slice()).unwrap()
}

#[test]
fn kits_link_their_credentials_and_hosts_without_exporting_tools() {
    assert_eq!(manifest("crab-agent-relay").database_size, 64 << 20);
    let linked = manifest("berm-linked-services");
    assert_eq!(
        linked
            .tools
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        ["inspect"]
    );
    assert!(linked.deps.secrets.iter().any(|s| s == "GITHUB_TOKEN"));
    assert!(linked.deps.vars.is_empty());
    assert_eq!(
        linked.deps.credentials["upstash_redis"].hosts,
        ["your-database.upstash.io"]
    );
    for kit in ["crabkit-upstash", "crabkit-github"] {
        assert!(linked.kits.iter().any(|linked| linked.name == kit), "{kit}");
    }
    assert!(linked.kits.iter().any(|kit| kit.name == "crabkit-auth"
        && kit.id.as_deref() == Some("115a99982fc0ce031c0a94fad9aa47a0")));
    assert!(
        linked
            .deps
            .secrets
            .iter()
            .any(|s| s == "UPSTASH_REDIS_REST_TOKEN")
    );
    assert_eq!(linked.buffer, berm_api::DEFAULT_BUFFER);
    assert!(linked.deps.credentials.contains_key("github"));
    assert!(linked.deps.credentials["github_api"].optional);
    assert!(linked.keeps_database());
    assert_eq!(linked.database_size, host::DATABASE_SIZE);
    let required = linked.deps.unset(&[]);
    assert!(required.iter().any(|name| name == "GITHUB_CLIENT_ID"));
    assert!(required.iter().any(|name| name == "GITHUB_CLIENT_SECRET"));
    assert!(!required.iter().any(|name| name == "GITHUB_TOKEN"));
    assert!(linked.deps.http.fetch.iter().any(|s| s == "api.github.com"));
}

#[test]
fn first_party_tools_have_registered_catalog_jobs_in_the_built_image() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../services");
    let catalog = crabtalk_sdk::catalog::Catalog::listed();
    for group in ["agents", "web"] {
        for entry in std::fs::read_dir(root.join(group)).unwrap() {
            let dir = entry.unwrap().path();
            if !dir.join("crab.toml").is_file() {
                continue;
            }
            let image = std::fs::read(berm_build::build(&dir, None).unwrap()).unwrap();
            let manifest = Manifest::try_from(image.as_slice()).unwrap();
            for tool in manifest.tools.iter().filter(|tool| tool.public) {
                let job = tool.catalog.as_deref().unwrap_or_else(|| {
                    panic!("{}: {} has no catalog job", manifest.name, tool.name)
                });
                catalog.check(job).unwrap();
            }
        }
    }
}
