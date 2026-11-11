use berm_api::host::{DatabaseUsage, Live, Metered};
use serde_json::json;

#[test]
fn old_nodes_do_not_report_zero_database_usage() {
    let live: Live = serde_json::from_value(json!({
        "running": 0, "memory": 0, "stored": 1500000,
        "cpu": {"total": 0, "per_second": 0.0},
        "read": {"total": 0, "per_second": 0.0},
        "written": {"total": 0, "per_second": 0.0},
        "proxied": {"total": 0, "per_second": 0.0},
        "logged": {"total": 0, "per_second": 0.0}
    }))
    .unwrap();
    assert!(live.database.is_none());
    let mut day = Metered::default();
    day.database = Some(DatabaseUsage {
        used: 1024,
        limit: 2048,
        at: 100,
    });
    let roundtrip: Metered =
        serde_json::from_value(serde_json::to_value(day.clone()).unwrap()).unwrap();
    assert_eq!(roundtrip.database, day.database);
}
