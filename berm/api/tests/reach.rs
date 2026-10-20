//! How a manifest's declared hosts reach a request's host: by name, through
//! `*.domain`, through `*`, or not at all.

use berm_api::{Deps, Reach};

const FETCH: &str = "berm.http.fetch";

fn deps(hosts: &[&str]) -> Deps {
    Deps {
        syscalls: [(
            FETCH.to_owned(),
            hosts.iter().map(|h| h.to_string()).collect(),
        )]
        .into(),
        ..Deps::default()
    }
}

#[test]
fn a_name_matches_itself_whole_in_any_case() {
    let deps = deps(&["Example.com"]);
    assert_eq!(deps.reach(FETCH, "example.com"), Some(Reach::Named));
    assert_eq!(deps.reach(FETCH, "www.example.com"), None);
    assert_eq!(deps.reach(FETCH, "notexample.com"), None);
}

#[test]
fn a_subdomain_pattern_matches_subdomains_and_not_the_domain() {
    let deps = deps(&["*.Example.com"]);
    assert_eq!(deps.reach(FETCH, "www.example.com"), Some(Reach::Pattern));
    assert_eq!(deps.reach(FETCH, "a.b.EXAMPLE.com"), Some(Reach::Pattern));
    assert_eq!(deps.reach(FETCH, "example.com"), None);
    assert_eq!(deps.reach(FETCH, "notexample.com"), None);
}

#[test]
fn a_star_matches_every_host() {
    let deps = deps(&["*"]);
    assert_eq!(deps.reach(FETCH, "example.com"), Some(Reach::Pattern));
    assert_eq!(deps.reach(FETCH, "127.0.0.1"), Some(Reach::Pattern));
}

#[test]
fn a_star_inside_a_label_is_a_name() {
    let deps = deps(&["*example.com", "api.*.com"]);
    assert_eq!(deps.reach(FETCH, "notexample.com"), None);
    assert_eq!(deps.reach(FETCH, "api.example.com"), None);
}

#[test]
fn a_name_wins_over_a_pattern() {
    let deps = deps(&["*", "example.com"]);
    assert_eq!(deps.reach(FETCH, "example.com"), Some(Reach::Named));
    assert_eq!(deps.reach(FETCH, "other.com"), Some(Reach::Pattern));
}

#[test]
fn another_syscall_reaches_nothing() {
    let deps = deps(&["*"]);
    assert_eq!(deps.reach("berm.http.proxy", "example.com"), None);
}
