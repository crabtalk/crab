//! The catalog's list: the shipped file loads, a platform listed twice is
//! refused, and a job is checked against it.

use crabtalk_sdk::catalog::Catalog;

#[test]
fn the_list_loads_in_the_order_written() {
    let catalog = Catalog::listed();
    let ids: Vec<&str> = catalog.categories.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), 13);
    assert_eq!(ids[0], "web");
    assert_eq!(ids[12], "accounts");
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
