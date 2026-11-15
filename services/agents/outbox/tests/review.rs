use agent_outbox::{Draft, review};
use berm_lang::test;

#[test]
fn review_preserves_the_email_as_literal_content_without_sending() {
    test::forget();
    let draft = Draft {
        from: "hello@example.com".into(),
        to: vec!["reader@example.com".into()],
        subject: "Report".into(),
        text: "# Heading\n<script>test</script>\n```".into(),
    };
    let result = review(draft.clone()).unwrap();
    assert_eq!(result.input.text, draft.text);
    assert!(
        result
            .body
            .contains("    # Heading\n    <script>test</script>\n    ```")
    );
    assert_eq!(result.tool, "send");
    assert!(test::fetched().is_empty());
}

#[test]
fn unsupported_content_is_refused_before_a_request_is_created() {
    let mut draft = Draft {
        from: "hello@example.com".into(),
        to: vec!["reader@example.com".into()],
        subject: "Report".into(),
        text: "Ready".into(),
    };
    draft.to = vec!["reader@example.com".into(); 6];
    assert!(review(draft.clone()).is_err());
    draft.to.truncate(1);
    draft.text = "x".repeat(4001);
    assert!(review(draft.clone()).is_err());
    draft.text = "Ready".into();
    draft.subject = "Report\nBcc: other@example.com".into();
    assert!(review(draft).is_err());
}
