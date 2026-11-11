use berm_api::host::{Broken, Checked, LastUse, Plan, Source};
use std::collections::BTreeMap;

#[test]
fn deployment_sets_preserve_recorded_callers_and_use_name_order() {
    let broken = Broken {
        broken: "tool old: removed".into(),
        by: vec![LastUse {
            source: Source::Service,
            caller: "alice/dashboard".into(),
            last: "2026-11-09".into(),
        }],
    };
    let checked = |breaks| Checked {
        version: "1.0.0".into(),
        migrations: Vec::new(),
        breaks,
        secrets: Vec::new(),
        required: Vec::new(),
    };
    let plan = Plan::new(&BTreeMap::from([
        ("dashboard".into(), checked(Vec::new())),
        ("weather".into(), checked(vec![broken])),
    ]));
    assert_eq!(plan.order, ["dashboard", "weather"]);
    assert_eq!(plan.breaks["weather"][0].by[0].caller, "alice/dashboard");
}
