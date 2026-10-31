//! GitHub OAuth belongs to the provider library; app sessions do not.
use berm_lang::test;
const TOKEN: &str = "https://github.com/login/oauth/access_token";
const USER: &str = "https://api.github.com/user";

#[test]
fn oauth_uses_app_credentials_and_the_exchanged_token_instead_of_the_repository_token() {
    test::respond(
        "POST",
        TOKEN,
        200,
        br#"{"access_token":"oauth-user-token"}"#,
    );
    test::respond(
        "GET",
        USER,
        200,
        br#"{"id":42,"login":"octo","name":null,"email":null}"#,
    );
    let user =
        github::oauth::user("code&scope=admin", "https://app.example/callback?a=1&b=2").unwrap();
    assert_eq!(user.id, 42);
    assert_eq!(user.login, "octo");
    let requests = test::fetched();
    let body = String::from_utf8(requests[0].body.clone()).unwrap();
    assert!(body.starts_with("code=code%26scope%3Dadmin&redirect_uri=https%3A%2F%2Fapp.example%2Fcallback%3Fa%3D1%26b%3D2&"),"{body}");
    assert!(body.ends_with(
        "client_id={secret:GITHUB_CLIENT_ID}&client_secret={secret:GITHUB_CLIENT_SECRET}"
    ));
    assert_eq!(
        requests[1].header("Authorization"),
        Some("Bearer oauth-user-token")
    );
    assert_eq!(
        requests[1].header("X-GitHub-Api-Version"),
        Some("2026-03-10")
    );
    assert!(!body.contains("GITHUB_TOKEN"));
}

#[test]
fn an_unsuccessful_token_response_never_reads_the_user_even_if_it_contains_a_token() {
    test::respond(
        "POST",
        TOKEN,
        400,
        br#"{"access_token":"invalid","error":"bad_verification_code"}"#,
    );
    let error = github::oauth::user("code", "https://app.example/callback").unwrap_err();
    assert!(error.message.contains("bad_verification_code"));
    assert_eq!(test::fetched().len(), 1);
}

#[test]
fn an_unreadable_token_response_reports_the_provider_status() {
    test::respond("POST", TOKEN, 502, b"bad gateway");
    let error = github::oauth::user("code", "https://app.example/callback").unwrap_err();
    assert!(error.message.contains("502"));
    assert_eq!(test::fetched().len(), 1);
}
