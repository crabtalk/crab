use berm_lang::{
    Schema,
    schema::{self, Piece},
    serde_json::{self, Value, json},
};

#[derive(Schema)]
struct Connection {
    url: String,
    credential: Option<String>,
}

#[derive(Schema)]
enum Platform {
    Local,
    Grok(Connection),
    Custom {
        endpoint: String,
        label: Option<String>,
    },
}

berm_lang::declare_service!(variants);

#[test]
fn payload_variants_render_and_round_trip_the_same_wire_shape() {
    const PIECES: &[Piece] = &[Piece::Shape(&Platform::SHAPE)];
    const OUT: [u8; schema::len(PIECES)] = schema::render(PIECES);
    let schema: Value = serde_json::from_slice(&OUT).unwrap();
    assert_eq!(
        schema["oneOf"][0],
        json!({"type":"string","enum":["local"]})
    );
    assert_eq!(schema["oneOf"][1]["required"], json!(["grok"]));
    assert_eq!(schema["oneOf"][1]["additionalProperties"], false);
    assert_eq!(
        schema["oneOf"][1]["properties"]["grok"]["required"],
        json!(["url"])
    );
    for wire in [
        json!("local"),
        json!({"grok":{"url":"https://example.com"}}),
        json!({"custom":{"endpoint":"https://example.com"}}),
    ] {
        let platform: Platform = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(platform).unwrap(), wire);
    }
    assert!(
        serde_json::from_value::<Platform>(json!({"grok":{"url":"a"},"custom":{"endpoint":"b"}}))
            .is_err()
    );
    assert!(serde_json::from_value::<Platform>(json!({"grok":{}})).is_err());
}

#[test]
fn generated_clients_keep_payload_variants_typed() {
    let platform = variants::types::Platform::Grok(variants::types::Connection {
        url: "https://example.com".into(),
        credential: None,
    });
    let wire = serde_json::to_value(platform).unwrap();
    assert_eq!(wire, json!({"grok":{"url":"https://example.com"}}));
    let restored: variants::types::Platform = serde_json::from_value(wire).unwrap();
    assert!(matches!(restored, variants::types::Platform::Grok(_)));
    let named = variants::types::Platform::Custom {
        endpoint: "a".into(),
        label: None,
    };
    assert_eq!(
        serde_json::to_value(named).unwrap(),
        json!({"custom":{"endpoint":"a"}})
    );
}
