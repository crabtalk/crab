use berm_api::host::{Broken, Checked, LastUse, Plan, Source};
use std::collections::BTreeMap;

fn checked(calls: &[&str], breaks: Vec<Broken>) -> Checked {
    Checked {
        version: "0.1.0".to_owned(),
        calls: calls.iter().map(|call| (*call).to_owned()).collect(),
        migrations: Vec::new(),
        breaks,
    }
}

fn broken(by: &[(Source, &str)]) -> Broken {
    Broken {
        broken: "tool old: removed".to_owned(),
        by: by
            .iter()
            .map(|(source, caller)| LastUse {
                source: *source,
                caller: (*caller).to_owned(),
                last: "2026-10-27".to_owned(),
            })
            .collect(),
    }
}

#[test]
fn a_service_switches_after_those_it_calls() {
    let set = BTreeMap::from([
        ("api".to_owned(), checked(&["store"], Vec::new())),
        ("store".to_owned(), checked(&[], Vec::new())),
    ]);
    assert_eq!(Plan::new(&set).order, ["store", "api"]);
}

#[test]
fn a_caller_that_stops_calling_is_exempt_and_switches_first() {
    let set = BTreeMap::from([
        (
            "a".to_owned(),
            checked(&[], vec![broken(&[(Source::Service, "b")])]),
        ),
        ("b".to_owned(), checked(&[], Vec::new())),
    ]);
    let plan = Plan::new(&set);
    assert!(plan.breaks["a"].is_empty());
    assert_eq!(plan.order, ["b", "a"]);
}

#[test]
fn a_caller_that_still_calls_is_counted() {
    let set = BTreeMap::from([
        (
            "a".to_owned(),
            checked(&[], vec![broken(&[(Source::Service, "b")])]),
        ),
        ("b".to_owned(), checked(&["a"], Vec::new())),
    ]);
    assert_eq!(Plan::new(&set).breaks["a"].len(), 1);
}

#[test]
fn a_caller_outside_the_set_is_counted() {
    let set = BTreeMap::from([
        (
            "a".to_owned(),
            checked(
                &[],
                vec![broken(&[(Source::Service, "b"), (Source::Account, "b")])],
            ),
        ),
        ("b".to_owned(), checked(&[], Vec::new())),
    ]);
    let plan = Plan::new(&set);
    assert_eq!(plan.breaks["a"].len(), 1);
    assert_eq!(plan.breaks["a"][0].by.len(), 1);
    assert_eq!(plan.breaks["a"][0].by[0].source, Source::Account);
}
