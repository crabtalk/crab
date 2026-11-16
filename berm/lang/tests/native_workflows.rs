#![deny(warnings)]

use berm_lang::{
    serde_json::{self, json},
    test,
};

mod service {
    use berm_lang::{Result, tool, workflow};

    /// Echo a string
    #[tool]
    fn echo(value: String) -> Result<String> {
        Ok(value)
    }

    /// Accept a string
    #[tool]
    fn sink(value: String) -> Result<()> {
        let _ = value;
        Ok(())
    }

    struct Text;
    /// Text
    #[workflow]
    impl Text {
        #[step]
        fn prepare(input: &str) -> Option<EchoCall> {
            (!input.is_empty()).then(|| EchoCall {
                value: input.into(),
            })
        }

        #[output]
        fn done(prepare: Option<&str>) -> Option<String> {
            prepare.map(str::to_owned)
        }
    }

    struct Unit;
    /// Unit
    #[workflow]
    impl Unit {
        #[step]
        fn write(input: &str) -> SinkCall {
            SinkCall {
                value: input.into(),
            }
        }

        #[output]
        fn done(write: &()) {
            let _ = write;
        }
    }

    struct Uncalled;
    /// Uncalled
    #[workflow]
    impl Uncalled {
        #[step]
        fn prepare(input: &str) -> EchoCall {
            EchoCall {
                value: input.into(),
            }
        }

        #[output]
        fn done(prepare: &str) -> String {
            prepare.into()
        }
    }
}

#[test]
fn private_exports_accept_borrowed_strings_and_implicit_unit_outputs() {
    let cases: [(extern "C" fn(), _, _, _); 5] = [
        (
            service::berm_workflow_text_prepare,
            json!("hello"),
            json!({}),
            json!({"kind":"calls","inputs":[{"value":"hello"}]}),
        ),
        (
            service::berm_workflow_text_prepare,
            json!(""),
            json!({}),
            json!({"kind":"skipped"}),
        ),
        (
            service::berm_workflow_text_done,
            json!("hello"),
            json!({"prepare":{"state":"present","value":"hello"}}),
            json!({"kind":"output","value":"hello"}),
        ),
        (
            service::berm_workflow_text_done,
            json!(""),
            json!({"prepare":{"state":"skipped"}}),
            json!({"kind":"output","value":null}),
        ),
        (
            service::berm_workflow_unit_done,
            json!("hello"),
            json!({"write":{"state":"present","value":null}}),
            json!({"kind":"output","value":null}),
        ),
    ];
    for (export, input, nodes, expected) in cases {
        let args = serde_json::to_vec(&json!({"input":input,"nodes":nodes})).unwrap();
        let bytes = test::call(export, &args).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            expected
        );
    }
}
