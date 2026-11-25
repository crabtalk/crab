use crabtalk_sdk::account::billing::{Credit, CreditHistory, PublicPlan, Verification};

#[test]
fn billing_responses_without_compute_credit_fields_remain_readable() {
    let credit: Credit =
        serde_json::from_str(r#"{"micros":200,"refill_cap_micros":0,"refill_micros":0}"#).unwrap();
    assert_eq!(credit.micros, 200);
    assert!(credit.compute.is_none());
    assert!(credit.recovery_until.is_none());
    let history: CreditHistory = serde_json::from_str(r#"{"months":[],"grants":[]}"#).unwrap();
    assert!(history.compute_months.is_empty());
    assert!(history.compute_grants.is_empty());
    let verification: Verification =
        serde_json::from_str(r#"{"verified_at":1,"reason":null,"github":true}"#).unwrap();
    assert!(verification.trial.is_none());
    let plan: PublicPlan = serde_json::from_str(
        r#"{"slug":"hobby","accounts":["user","org"],"credit_micros":0,"price_micros":null,"included":{},"memory":134217728,"rates":{"runs":100000000,"overage":20000,"read":100000,"written":100000000,"held":10}}"#,
    )
    .unwrap();
    assert!(plan.trial.is_none());
}
