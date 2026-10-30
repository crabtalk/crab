//! What meters cost at the rates.

use berm_api::Meter;
use crabtalk_sdk::system::meta::Rates;

fn rates() -> Rates {
    Rates {
        runs: 150_000_000,
        overage: 20_000,
        read: 1_000_000,
        written: 1_000_000_000,
        held: 12,
        proxied: None,
    }
}

#[test]
fn a_call_costs_each_unit_at_its_rate() {
    let meter = Meter {
        runs: 1,
        read: 10,
        written: 2,
        held: 1_000,
        ..Meter::default()
    };
    // In billionths of a micro: 150,000,000 for the run, 1,000,000 for 50µs
    // of overage, 10,000,000 read, 2,000,000,000 written, 12,000 held.
    assert_eq!(rates().cost_decimal(&meter, 50), "2.161012");
    assert_eq!(rates().call_cost(&meter, 50), 2);
}

#[test]
fn a_whole_cost_has_no_fraction() {
    let meter = Meter {
        written: 3,
        ..Meter::default()
    };
    assert_eq!(rates().cost_decimal(&meter, 0), "3");
}

#[test]
fn proxied_bytes_are_free_while_unrated() {
    let meter = Meter {
        proxied: 1 << 30,
        ..Meter::default()
    };
    assert_eq!(rates().cost_decimal(&meter, 0), "0");
    let rated = Rates {
        proxied: Some(1_000_000_000),
        ..rates()
    };
    assert_eq!(rated.cost_decimal(&meter, 0), (1u64 << 30).to_string());
}
