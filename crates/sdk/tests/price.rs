//! Prices: USD micros, at least $0.0001, shown as USD.

use crabtalk_sdk::services::price::Price;

#[test]
fn below_the_smallest_price_is_refused() {
    assert!(
        Price::try_from(99)
            .unwrap_err()
            .contains("at least $0.0001")
    );
    assert_eq!(u64::from(Price::try_from(100).unwrap()), 100);
}

#[test]
fn a_price_shows_to_the_micro_and_at_least_the_cent() {
    let shown = |micros: u64| Price::try_from(micros).unwrap().to_string();
    assert_eq!(shown(2_000), "$0.002");
    assert_eq!(shown(100), "$0.0001");
    assert_eq!(shown(1_000_000), "$1.00");
    assert_eq!(shown(1_500_000), "$1.50");
    assert_eq!(shown(123_456), "$0.123456");
}

#[test]
fn the_wire_carries_micros_and_refuses_below_the_smallest() {
    let price: Price = serde_json::from_str("2000").unwrap();
    assert_eq!(serde_json::to_string(&price).unwrap(), "2000");
    assert!(serde_json::from_str::<Price>("99").is_err());
}
