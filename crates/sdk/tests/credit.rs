use crabtalk_sdk::account::billing::{RefillRequest, Unit};

#[test]
fn an_owed_warning_keeps_the_unit_released_clients_know() {
    assert_eq!(serde_json::to_string(&Unit::Owed).unwrap(), r#""credit""#);
    assert_eq!(
        serde_json::from_str::<Unit>(r#""credit""#).unwrap(),
        Unit::Owed
    );
}

#[test]
fn a_refill_without_a_threshold_refills_under_five_dollars() {
    let request: RefillRequest =
        serde_json::from_str(r#"{"cap_micros":100000000,"amount_micros":20000000}"#).unwrap();
    assert_eq!(request.below_micros, 5_000_000);
}
