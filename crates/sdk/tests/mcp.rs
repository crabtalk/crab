use berm_api::ToolSpec;
use crabtalk_sdk::mcp::{Action, Found, Tool};
use serde_json::{self, json};

#[test]
fn discovery_does_not_return_schemas_and_query_refuses_writes() {
    let found = Found {
        service: "owner/reader".into(),
        description: "Read pages".into(),
        tools: vec![Tool {
            name: "read".into(),
            title: Some("Read a page".into()),
        }],
    };
    assert_eq!(
        serde_json::to_value(found).unwrap(),
        json!({"service":"owner/reader", "description":"Read pages", "tools":[{"name":"read", "title":"Read a page"}]})
    );
    let mut tool: ToolSpec = serde_json::from_value(
        json!({"name":"write", "description":"Write", "parameters":{"type":"object"}}),
    )
    .unwrap();
    assert!(
        Action::Query
            .check_tool(&tool)
            .unwrap_err()
            .contains("use call")
    );
    assert!(Action::Call.check_tool(&tool).is_ok());
    tool.query = true;
    assert!(Action::Query.check_tool(&tool).is_ok());
}
