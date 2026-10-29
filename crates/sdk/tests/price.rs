//! `[prices]` values: USD decimal strings read into micros, at least $0.0001
//! and to at most six places.

use crabtalk_sdk::services::price::Price;

fn micros(s: &str) -> u64 {
    s.parse::<Price>().unwrap().into()
}

fn refused(s: &str) -> String {
    s.parse::<Price>().unwrap_err()
}

#[test]
fn a_decimal_reads_as_micros() {
    assert_eq!(micros("0.002"), 2_000);
    assert_eq!(micros("0.0001"), 100);
    assert_eq!(micros("0.000150"), 150);
    assert_eq!(micros("1"), 1_000_000);
    assert_eq!(micros("12.5"), 12_500_000);
}

#[test]
fn below_the_smallest_price_is_refused() {
    assert!(refused("0").contains("smallest price"));
    assert!(refused("0.0").contains("smallest price"));
    assert!(refused("0.00009").contains("smallest price"));
}

#[test]
fn more_than_six_places_is_refused() {
    assert!(refused("0.0000015").contains("more than 6 decimal places"));
}

#[test]
fn a_negative_price_is_refused() {
    assert!(refused("-0.002").contains("negative"));
}

#[test]
fn a_malformed_price_is_refused() {
    for malformed in ["", ".5", "1.", "$1", "+1", "1e-3", "0,002", " 1", "1.2.3"] {
        assert!(refused(malformed).contains("not USD"), "{malformed:?}");
    }
    assert!(refused("99999999999999999999").contains("not USD"));
}

#[test]
fn a_price_shows_to_the_micro_and_at_least_the_cent() {
    let shown = |s: &str| s.parse::<Price>().unwrap().to_string();
    assert_eq!(shown("0.002"), "$0.002");
    assert_eq!(shown("0.0001"), "$0.0001");
    assert_eq!(shown("1"), "$1.00");
    assert_eq!(shown("1.5"), "$1.50");
    assert_eq!(shown("0.123456"), "$0.123456");
}

#[test]
fn the_wire_carries_micros_and_refuses_below_the_smallest() {
    let price: Price = serde_json::from_str("2000").unwrap();
    assert_eq!(serde_json::to_string(&price).unwrap(), "2000");
    assert!(serde_json::from_str::<Price>("99").is_err());
}
