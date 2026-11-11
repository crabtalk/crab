use berm_abi::events::{Event, Page};

#[test]
fn checkpoints_and_page_positions_cannot_skip_unacknowledged_events() {
    let mut page = Page {
        events: Vec::new(),
        cursor: "0".into(),
        has_more: false,
    };
    assert!(page.check(None).is_ok());
    page.has_more = true;
    assert!(page.check(Some("0")).is_err());
    page.events.push(Event {
        id: "a".into(),
        cursor: "1".into(),
        at: 10,
        data: (),
    });
    assert!(page.check(Some("0")).is_err());
    page.cursor = "1".into();
    assert!(page.check(Some("0")).is_ok());
    assert!(page.check(None).is_err());
    assert!(page.check(Some("1")).is_err());
    page.events.push(Event {
        id: "a".into(),
        cursor: "2".into(),
        at: 10,
        data: (),
    });
    page.cursor = "2".into();
    assert!(page.check(Some("0")).is_err());
    page.events[1].id = "b".into();
    assert!(page.check(Some("0")).is_ok());
    page.events[1].cursor = "1".into();
    assert!(page.check(Some("0")).is_err());
}
