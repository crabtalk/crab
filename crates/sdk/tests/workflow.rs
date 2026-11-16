use crabtalk_sdk::services::workflow::{Settlement, Spend, Terms};
use serde_json::{Value, json};

#[test]
fn accepted_terms_keep_price_quote_and_cumulative_cap_separate() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/workflow-terms-v1.json")).unwrap();
    let terms: Terms = serde_json::from_value(fixture.clone()).unwrap();
    assert_eq!(u64::from(terms.task_price.unwrap()), 50_000);
    assert_eq!(terms.direct_cost, 30_000);
    assert_eq!(terms.max_cost, 40_000);
    assert_eq!(serde_json::to_value(&terms).unwrap(), fixture);
    let mut future = fixture;
    future["version"] = json!(2);
    assert!(serde_json::from_value::<Terms>(future).is_err());
}

#[test]
fn unknown_reservations_and_settlement_identities_survive_round_trip() {
    let spend = json!({"run":"run-1","incurred":"400","reserved":"1000"});
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<Spend>(spend.clone()).unwrap()).unwrap(),
        spend
    );
    for value in [
        json!({"state":"owner"}),
        json!({"state":"held","reservation":"hold-1"}),
        json!({"state":"released"}),
        json!({"state":"settled","transaction":"sale-1"}),
    ] {
        let settlement: Settlement = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(settlement).unwrap(), value);
    }
}
