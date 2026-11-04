//! The catalog's list: the shipped file loads, a platform listed twice is
//! refused, and a job is checked against it.

use crabtalk_sdk::catalog::{Catalog, Provider};
use std::{fs, path::Path};
use toml::Table;

#[test]
fn the_list_loads_in_the_order_written() {
    let catalog = Catalog::listed();
    let ids: Vec<&str> = catalog.categories.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), 15);
    assert_eq!(ids[0], "web");
    assert_eq!(ids.last(), Some(&"accounts"));
    assert_eq!(catalog.category("tiktok-shop").unwrap().title, "Commerce");
}

#[test]
fn a_platform_listed_twice_is_refused() {
    let twice =
        "[a]\ntitle = \"A\"\nplatforms = [\"x\"]\n\n[b]\ntitle = \"B\"\nplatforms = [\"x\"]\n";
    assert!(
        twice
            .parse::<Catalog>()
            .unwrap_err()
            .contains("x is listed twice")
    );
}

#[test]
fn a_job_starts_with_a_listed_platform() {
    let catalog = Catalog::listed();
    assert_eq!(catalog.check("tiktok.user.videos").unwrap().id, "social");
    assert_eq!(catalog.check("company.enrich").unwrap().id, "companies");
    assert!(
        catalog
            .check("myspace.friends")
            .unwrap_err()
            .contains("does not list")
    );
    for malformed in ["web", "web.", ".search"] {
        assert!(
            catalog
                .check(malformed)
                .unwrap_err()
                .contains("then the job"),
            "{malformed}"
        );
    }
}

fn provider(service: &str, title: Option<&str>, calls: u64) -> Provider {
    Provider {
        service: service.to_owned(),
        tool: "enrich".to_owned(),
        title: title.map(str::to_owned),
        description: String::new(),
        price: None,
        calls,
    }
}

#[test]
fn providers_group_into_jobs_under_their_platform() {
    let catalog = Catalog::listed();
    let categories = catalog.jobs(vec![
        ("company.enrich".to_owned(), provider("a/one", None, 3)),
        (
            "company.enrich".to_owned(),
            provider("b/two", Some("Enrich a company"), 9),
        ),
        ("company.search".to_owned(), provider("a/one", None, 20)),
        ("myspace.friends".to_owned(), provider("a/one", None, 1)),
    ]);
    assert_eq!(categories.len(), catalog.categories.len());
    let companies = categories.iter().find(|c| c.id == "companies").unwrap();
    let company = &companies.platforms[0];
    assert_eq!(company.id, "company");
    let ids: Vec<&str> = company.jobs.iter().map(|job| job.id.as_str()).collect();
    assert_eq!(ids, ["company.search", "company.enrich"]);
    let enrich = &company.jobs[1];
    assert_eq!(enrich.title, "Enrich a company");
    assert_eq!(enrich.providers[0].service, "b/two");
    let listed: usize = categories
        .iter()
        .flat_map(|c| &c.platforms)
        .map(|p| p.jobs.len())
        .sum();
    assert_eq!(listed, 2);
}

#[test]
fn first_party_tools_have_registered_catalog_jobs() {
    let catalog = Catalog::listed();
    let services = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../services");
    for group in ["agents", "web", "ops"] {
        for entry in fs::read_dir(services.join(group)).unwrap() {
            let path = entry.unwrap().path().join("crab.toml");
            if !path.is_file() {
                continue;
            }
            let manifest: Table = fs::read_to_string(&path).unwrap().parse().unwrap();
            for (name, tool) in manifest["tools"].as_table().unwrap() {
                if tool.get("access").is_none() {
                    continue;
                }
                let job = tool
                    .get("catalog")
                    .and_then(|job| job.as_str())
                    .unwrap_or_else(|| {
                        panic!("{}: tools.{name} has no catalog job", path.display())
                    });
                catalog
                    .check(job)
                    .unwrap_or_else(|error| panic!("{}: tools.{name}: {error}", path.display()));
            }
        }
    }
}
