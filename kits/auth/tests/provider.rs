//! `Identity::github` and `Identity::google` natively: the requests they send
//! and how they read what GitHub and Google answer.

use berm_lang::test;
use crab_auth::Identity;

const GITHUB_TOKEN: &str = "https://github.com/login/oauth/access_token";
const GITHUB_USER: &str = "https://api.github.com/user";
const GOOGLE_TOKEN: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USER: &str = "https://openidconnect.googleapis.com/v1/userinfo";
const REDIRECT: &str = "https://app.example.com/callback";

#[test]
fn github_exchanges_the_code_and_reads_the_user() {
    test::respond("POST", GITHUB_TOKEN, 200, br#"{"access_token":"gho_1"}"#);
    test::respond(
        "GET",
        GITHUB_USER,
        200,
        br#"{"id":42,"login":"octo","name":null,"email":null,"avatar_url":"https://avatars.example.com/octo"}"#,
    );
    let identity = Identity::github("c0de", REDIRECT).expect("a GitHub identity");
    assert_eq!(identity.provider, "github");
    assert_eq!(identity.subject, "42");
    assert_eq!(identity.name.as_deref(), Some("octo"));
    assert_eq!(identity.email, None);
    assert_eq!(
        identity.avatar.as_deref(),
        Some("https://avatars.example.com/octo")
    );

    let fetched = test::fetched();
    assert_eq!(
        String::from_utf8(fetched[0].body.clone()).expect("a form"),
        "code=c0de&redirect_uri=https%3A%2F%2Fapp.example.com%2Fcallback&client_id={secret:GITHUB_CLIENT_ID}&client_secret={secret:GITHUB_CLIENT_SECRET}"
    );
    assert_eq!(fetched[1].header("Authorization"), Some("Bearer gho_1"));
    assert_eq!(fetched[1].header("User-Agent"), Some("crab-auth"));
}

#[test]
fn google_sends_the_verifier_and_drops_an_unverified_email() {
    test::respond("POST", GOOGLE_TOKEN, 200, br#"{"access_token":"ya29.1"}"#);
    test::respond(
        "GET",
        GOOGLE_USER,
        200,
        br#"{"sub":"g-1","name":"Ada","email":"ada@example.com","email_verified":false}"#,
    );
    let identity = Identity::google("c0de", REDIRECT, Some("v3rifier")).expect("a Google identity");
    assert_eq!(identity.provider, "google");
    assert_eq!(identity.subject, "g-1");
    assert_eq!(identity.email, None);

    let form = String::from_utf8(test::fetched()[0].body.clone()).expect("a form");
    assert!(
        form.starts_with("grant_type=authorization_code&code=c0de&"),
        "{form}"
    );
    assert!(form.contains("&code_verifier=v3rifier&"), "{form}");
}

#[test]
fn google_keeps_a_verified_email() {
    test::respond("POST", GOOGLE_TOKEN, 200, br#"{"access_token":"ya29.1"}"#);
    test::respond(
        "GET",
        GOOGLE_USER,
        200,
        br#"{"sub":"g-1","email":"ada@example.com","email_verified":true}"#,
    );
    let identity = Identity::google("c0de", REDIRECT, None).expect("a Google identity");
    assert_eq!(identity.email.as_deref(), Some("ada@example.com"));
    let form = String::from_utf8(test::fetched()[0].body.clone()).expect("a form");
    assert!(!form.contains("code_verifier"), "{form}");
}

#[test]
fn a_refused_code_fails_with_the_providers_description() {
    test::respond(
        "POST",
        GITHUB_TOKEN,
        200,
        br#"{"error":"bad_verification_code","error_description":"The code passed is incorrect or expired."}"#,
    );
    let failure = Identity::github("c0de", REDIRECT).err().expect("a refusal");
    assert_eq!(
        failure.message,
        "GitHub refused the code: The code passed is incorrect or expired."
    );
}

#[test]
fn a_refused_user_fails_with_its_status() {
    test::respond("POST", GITHUB_TOKEN, 200, br#"{"access_token":"gho_1"}"#);
    test::respond("GET", GITHUB_USER, 401, br#"{"message":"Bad credentials"}"#);
    let failure = Identity::github("c0de", REDIRECT).err().expect("a refusal");
    assert_eq!(failure.message, "GitHub answered 401 for the user");
}
