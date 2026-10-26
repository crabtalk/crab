use berm_api::host::{Share, Spread};
use std::collections::BTreeMap;

#[test]
fn a_value_falls_below_its_share_bound() {
    for used in [1, 2, 3, 99, 100, 101, 5_000, 1_000_000] {
        let index = Spread::index(used);
        assert!(
            used < Spread::bound(index),
            "{used} under {}",
            Spread::bound(index)
        );
        assert!(
            used >= Spread::bound(index - 1),
            "{used} past {}",
            Spread::bound(index - 1)
        );
    }
    assert_eq!(Spread::index(0), 0);
    assert_eq!(Spread::bound(0), 1);
}

#[test]
fn percentiles_are_the_bounds_their_calls_reach() {
    let mut counted = BTreeMap::new();
    for used in std::iter::repeat_n(100, 90).chain(std::iter::repeat_n(10_000, 10)) {
        *counted.entry(Spread::index(used)).or_default() += 1;
    }
    let spread = Spread::of(&counted, 10_000);
    assert_eq!(spread.p50, Spread::bound(Spread::index(100)));
    assert_eq!(spread.p95, 10_000);
    assert_eq!(spread.max, 10_000);
    assert_eq!(
        spread.shares,
        vec![
            Share {
                upto: Spread::bound(Spread::index(100)),
                calls: 90
            },
            Share {
                upto: Spread::bound(Spread::index(10_000)),
                calls: 10
            },
        ]
    );
}

#[test]
fn no_calls_spread_to_nothing() {
    assert_eq!(Spread::of(&BTreeMap::new(), 0), Spread::default());
}
