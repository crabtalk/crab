use crabtalk_sdk::account::billing::{Credit, PublicPlan, RefillRequest, Unit, Verification};

#[test]
fn responses_from_the_previous_cloud_remain_readable() {
    let credit: Credit = serde_json::from_str(
        r#"{"micros":200,"compute":{"micros":1,"granted_micros":1,"reserved_micros":0,"expires_at":1},"refill_cap_micros":0,"refill_micros":0}"#,
    )
    .unwrap();
    assert_eq!(credit.micros, 200);
    assert_eq!(credit.expiring_micros, 0);
    assert_eq!(credit.refill_below_micros, RefillRequest::below());
    let verification: Verification = serde_json::from_str(
        r#"{"verified_at":1,"reason":null,"github":true,"trial":{"amount_micros":1,"lifetime_days":30,"retention_days":30}}"#,
    )
    .unwrap();
    assert_eq!(verification.verified_at, Some(1));
    let plan: PublicPlan = serde_json::from_str(
        r#"{"slug":"go","accounts":["user","org"],"credit_micros":5000000,"price_micros":5000000,"included":{},"memory":536870912,"rates":{"runs":100000000,"overage":20000,"read":100000,"written":100000000,"held":10}}"#,
    )
    .unwrap();
    assert_eq!(plan.credit_micros, 5_000_000);
}

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
